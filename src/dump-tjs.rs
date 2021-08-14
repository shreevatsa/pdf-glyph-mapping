//! Parses a PDF file, and dumps the following:
//!
//! 1.  For each font, its ToUnicode mapping (if present), with the font's /BaseFont name.
//! 2.  For each font, the operands of each text-showing (`Tj` etc) operation that uses that font.

use anyhow::Result;
use clap::Clap;
use itertools::Itertools;
use lopdf::ObjectId;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs::File;
use std::io::Write;

fn basename_for_font(font_id: ObjectId, base_font_name: &str) -> String {
    format!("font-{}-{}-{}", font_id.0, font_id.1, base_font_name)
}
struct TjFiles {
    file: HashMap<lopdf::ObjectId, File>,
}
impl TjFiles {
    fn get_file(
        &mut self,
        maps_dir: &std::path::PathBuf,
        font_id: (String, ObjectId),
    ) -> &mut File {
        self.file.entry(font_id.1).or_insert_with(|| {
            let filename = std::path::Path::new(maps_dir)
                .join(basename_for_font(font_id.1, &font_id.0) + ".Tjs");
            println!("Creating file: {:?}", filename);
            std::fs::create_dir_all(maps_dir.clone()).unwrap();
            File::create(filename).unwrap()
        })
    }
}

fn real_font_id(font_reference_id: ObjectId, document: &lopdf::Document) -> (String, ObjectId) {
    /*
    For instance, given "15454 0", returns ("APZKLW+NotoSansDevanagari-Bold", "40531 0"), in this example:
    ...

    /F4 15454 0 R

    ...

    15454 0 obj
    <<
      /BaseFont /APZKLW+NotoSansDevanagari-Bold
      /DescendantFonts [ 40495 0 R ]
      /Encoding /Identity-H
      /Subtype /Type0
      /ToUnicode 40496 0 R
      /Type /Font
    >>
    endobj

    ...

    40495 0 obj
    <<
      /BaseFont /APZKLW+NotoSansDevanagari-Bold
      /CIDSystemInfo <<
        /Ordering (Identity)
        /Registry (Adobe)
        /Supplement 0
      >>
      /CIDToGIDMap /Identity
      /DW 0
      /FontDescriptor 40531 0 R
      /Subtype /CIDFontType2
      /Type /Font

    ...
    */
    let referenced_font = document
        .get_object(font_reference_id)
        .unwrap()
        .as_dict()
        .unwrap();
    let base_font_name = referenced_font
        .get(b"BaseFont")
        .unwrap()
        .as_name_str()
        .unwrap()
        .to_string();
    let descendant_fonts = referenced_font
        .get(b"DescendantFonts")
        .unwrap()
        .as_array()
        .unwrap();
    assert_eq!(descendant_fonts.len(), 1);
    let descendant_font = document
        .get_object(descendant_fonts[0].as_reference().unwrap())
        .unwrap()
        .as_dict()
        .unwrap();
    (
        base_font_name,
        descendant_font
            .get(b"FontDescriptor")
            .unwrap()
            .as_reference()
            .unwrap(),
    )
}

/// Writes the text operators from `content` into corresponding `files`.
fn process_text_operators_object(
    document: &mut lopdf::Document,
    object_id: ObjectId,
    fonts: &lopdf::Dictionary,
    xobjects_dict: &lopdf::Dictionary,
    maps_dir: &std::path::PathBuf,
    files: &mut TjFiles,
    font_glyph_mappings: &mut HashMap<ObjectId, HashMap<u16, String>>,
    phase: &Phase,
) {
    let mut content: lopdf::content::Content = {
        let mut content_u8 = Vec::new();
        if let Ok(content_stream) = document
            .get_object(object_id)
            .and_then(lopdf::Object::as_stream)
        {
            match content_stream.decompressed_content() {
                Ok(data) => content_u8.write_all(&data).unwrap(),
                Err(_) => content_u8.write_all(&content_stream.content).unwrap(),
            };
        }
        lopdf::content::Content::decode(&content_u8).unwrap()
    };
    // println!("Finding text operators in: {:?}", content);
    let mut current_font: (String, ObjectId) = ("".to_string(), (0, 0));
    let mut i = 0;
    while i < content.operations.len() {
        let op = &content.operations[i];
        let operator = &op.operator;
        if operator == "Tf" {
            let font_name = op.operands[0].as_name_str().unwrap();
            let font_id = fonts
                .get(font_name.as_bytes())
                .unwrap()
                .as_reference()
                .unwrap();
            // println!("Switching to font {}, which means {:?}", font_name, font_id);
            current_font = real_font_id(font_id, document);
        } else if ["Tj", "TJ", "'", "\""].contains(&operator.as_str()) {
            let content: &mut lopdf::content::Content = &mut content;
            let glyphs = glyphs_in_text_operation(content, i);
            match phase {
                // Phase 1: Write to file.
                Phase::Phase1Dump => {
                    dump_text_operation(&glyphs, &current_font, maps_dir, files);
                }
                Phase::Phase2Fix => {
                    // Phase 2: Wrap the operator in /ActualText.
                    i = wrap_text_operation(
                        content,
                        i,
                        current_font.clone(),
                        font_glyph_mappings,
                        glyphs,
                    )
                }
            };
            let obj = document.get_object_mut(object_id).unwrap();
            let stream = obj.as_stream_mut().unwrap();
            stream.set_content(content.encode().unwrap());
        } else if operator == "Do" {
            assert_eq!(op.operands.len(), 1);
            let name = &op.operands[0].as_name_str().unwrap();
            let (object_id, stream) = {
                let mut object = xobjects_dict.get(name.as_bytes()).unwrap();
                let mut id = (0, 0);
                while let Ok(ref_id) = object.as_reference() {
                    id = ref_id;
                    object = document.objects.get(&ref_id).unwrap();
                }
                (id, object.as_stream().unwrap().clone())
            };
            let empty_dict = lopdf::Dictionary::new();
            let (fonts, xobjects_dict) = match stream.dict.get(b"Resources") {
                Ok(value) => {
                    let resources_dict = value.as_dict().unwrap();
                    (
                        resources_dict.get(b"Font").unwrap().as_dict().unwrap(),
                        resources_dict.get(b"XObject").unwrap().as_dict().unwrap(),
                    )
                }
                Err(_) => (&empty_dict, &empty_dict),
            };
            process_text_operators_object(
                document,
                object_id,
                &fonts,
                xobjects_dict,
                maps_dir,
                files,
                font_glyph_mappings,
                phase,
            );
        } else {
            // println!(
            //     "Not a text-showing operator: {} {:?}",
            //     &operator, op.operands
            // );
        }
        i += 1;
    }
}

fn actual_text_for(
    glyphs: &[u16],
    current_font: (String, ObjectId),
    font_glyph_mappings: &mut HashMap<ObjectId, HashMap<u16, String>>,
) -> Result<String> {
    // println!("Looking up font {:?}", current_font);

    fn get_font_mapping(font_id: ObjectId, base_font_name: &str) -> HashMap<u16, String> {
        let mut ret = HashMap::<u16, std::string::String>::new();
        let filename = basename_for_font(font_id, base_font_name) + ".toml";
        println!("Trying to read from filename {}", filename);
        let s = std::fs::read_to_string(filename).unwrap();
        for (i, line) in s.lines().enumerate() {
            if i > 0 {
                let (glyph_id, meaning) = match regex::Regex::new(
                    // TODO: The "meaning" should be arbitrary Unicode string.
                    r"(?P<glyph_id>[[:xdigit:]]{4}) -> \{(?P<meaning>[[:xdigit:]]{4})}",
                )
                .unwrap()
                .captures(line)
                {
                    Some(captures) => (
                        captures.name("glyph_id").unwrap().as_str(),
                        captures.name("meaning").unwrap().as_str(),
                    ),
                    None => ("", ""),
                };
                // println!("Line #{}# maps #{}# to #{}#", line, glyph_id, meaning);
                ret.insert(
                    u16::from_str_radix(glyph_id, 16).unwrap(),
                    std::char::from_u32(u32::from_str_radix(meaning, 16).unwrap())
                        .unwrap()
                        .to_string(),
                );
            }
        }
        // let filename2 = "manual.toml";
        // let toml_string = std::fs::read_to_string(filename2).unwrap();
        // let config: toml::Value = toml::from_str(&toml_string).unwrap();
        // println!("{:#?}", config);
        ret
    }
    if !font_glyph_mappings.contains_key(&current_font.1) {
        let tmp = get_font_mapping(current_font.1, &current_font.0);
        font_glyph_mappings.insert(current_font.1, tmp);
    }
    let current_map = font_glyph_mappings.get(&current_font.1).unwrap();
    // TODO: Replace by something more sophisticated. :-)
    Ok(glyphs
        .iter()
        .map(|n| {
            match current_map.get(n) {
                Some(v) => v.to_string(),
                None => {
                    // println!("No mapping for {:04X}.", n);
                    format!("{{{:04X}}}", n)
                }
            }
            // if *n == 3 {
            //     format!(" ")
            // }
        })
        .join(""))
}

fn glyphs_in_text_operation(content: &mut lopdf::content::Content, i: usize) -> Vec<u16> {
    let op = &content.operations[i];
    let operator = &op.operator;
    let mut bytes: Vec<u8> = Vec::new();
    let text: &[u8] = if operator == "Tj" || operator == "'" {
        assert_eq!(op.operands.len(), 1);
        op.operands[0].as_str().unwrap()
    } else if operator == "\"" {
        assert_eq!(op.operands.len(), 3);
        op.operands[2].as_str().unwrap()
    } else if operator == "TJ" {
        assert_eq!(op.operands.len(), 1);
        let operands = op.operands[0].as_array().unwrap();
        for element in operands {
            if let Ok(s) = element.as_str() {
                bytes.extend(s);
            } else if let Ok(_) = element.as_f64() {
                //
            } else {
                assert!(false, "TJ operator wasn't str or number: {:#?}", element);
            }
        }
        bytes.as_slice()
    } else {
        unreachable!();
    };

    let glyphs: Vec<u16> = text
        .chunks(2)
        .map(|chunk| chunk[0] as u16 * 256 + chunk[1] as u16)
        .collect();
    glyphs
}

fn dump_text_operation(
    glyphs: &Vec<u16>,
    current_font: &(String, ObjectId),
    maps_dir: &std::path::PathBuf,
    files: &mut TjFiles,
) {
    let file = files.get_file(maps_dir, current_font.clone());
    let glyph_hexes: Vec<String> = glyphs.iter().map(|n| format!("{:04X} ", n)).collect();
    glyph_hexes
        .iter()
        .for_each(|g| file.write_all(g.as_bytes()).unwrap());
    file.write_all(b"\n").unwrap();
}

/// Before: content[i] = op.
/// After:
///         content[i] = BDC [/Span <</ActualText (...)>>]
///         content[i + 1] = op
///         content[i + 2] = EMC []
fn wrap_text_operation(
    content: &mut lopdf::content::Content,
    i: usize,
    current_font: (String, ObjectId),
    font_glyph_mappings: &mut HashMap<ObjectId, HashMap<u16, String>>,
    glyphs: Vec<u16>,
) -> usize {
    let mytext = actual_text_for(&glyphs, current_font, font_glyph_mappings);

    fn encode_for_pdf_silly_actualtext(mytext: &str) -> Vec<u8> {
        /*
        In the PDF 1.7 spec, see
        •   "7.3.4.2 Literal Strings" (p. 15, PDF page 23).
        •   "7.9.2 String Object Types" (p. 85, PDF page 93), especially Table 35 and Figure 7.
        •   "7.9.2.2 Text String Type" (immediately following the above).
         */

        let mut ret: Vec<u8> = vec![254, 255];
        for usv in mytext.encode_utf16() {
            let bytes = usv.to_be_bytes();
            assert_eq!(bytes.len(), 2);
            for byte in &bytes {
                ret.push(*byte);
            }
        }
        ret
    }

    // println!("Surrounding #{}#", mytext);
    let dict = lopdf::dictionary!("ActualText" =>
    lopdf::Object::String(encode_for_pdf_silly_actualtext(&mytext.unwrap()),
                          lopdf::StringFormat::Hexadecimal));
    // println!("…this became: {:?}", dict);
    content.operations.insert(
        i,
        lopdf::content::Operation::new(
            "BDC",
            vec![lopdf::Object::from("Span"), lopdf::Object::Dictionary(dict)],
        ),
    );
    content
        .operations
        .insert(i + 2, lopdf::content::Operation::new("EMC", vec![]));
    i + 2
}

fn print_text_operators_doc(
    document: &mut lopdf::Document,
    maps_dir: &std::path::PathBuf,
    files: &mut TjFiles,
    phase: &Phase,
    output_pdf_file: Option<std::path::PathBuf>,
) -> Result<()> {
    let mut font_glyph_mappings: HashMap<ObjectId, HashMap<u16, String>> = HashMap::new();
    let pages = document.get_pages();
    println!("{} pages in this document", pages.len());
    for (page_num, page_id) in pages {
        let (maybe_dict, resource_objects) = document.get_page_resources(page_id);
        println!(
            "Page number {} has page id {:?} and page resources: {:?} and {:?}",
            page_num, page_id, maybe_dict, resource_objects
        );
        let mut fonts = lopdf::Dictionary::new();
        if let Some(resource_dict) = maybe_dict {
            if let Ok(f) = resource_dict.get(b"Font") {
                fonts.extend(f.as_dict()?);
            }
        }
        for object_id in &resource_objects {
            let resource = document.get_object(*object_id).unwrap();
            let dict = resource.as_dict().unwrap();
            if let Ok(f) = dict.get(b"Font") {
                fonts.extend(f.as_dict().unwrap());
            }
        }
        // println!("Fonts: {:?}", fonts);
        let mut xobjects_dict = lopdf::Dictionary::new();
        if let Some(resource_dict) = maybe_dict {
            if let Ok(xd) = resource_dict.get(b"XObject") {
                xobjects_dict.extend(xd.as_dict().unwrap());
            }
        } else {
            assert_eq!(resource_objects.len(), 1);
        }
        for object_id in &resource_objects {
            let resource = document.get_object(*object_id).unwrap();
            let resource_dict = resource.as_dict().unwrap();
            if let Ok(xd) = resource_dict.get(b"XObject") {
                xobjects_dict.extend(xd.as_dict().unwrap());
            }
        }
        // println!("xobjects_dict: {:?}", xobjects_dict);

        let content_streams = document.get_page_contents(page_id);
        for object_id in content_streams {
            process_text_operators_object(
                document,
                object_id,
                &fonts,
                &xobjects_dict,
                maps_dir,
                files,
                &mut font_glyph_mappings,
                phase,
            );
        }
    }
    match output_pdf_file {
        Some(new_filename) => {
            println!("Creating file: {:?}", new_filename);
            document.save(new_filename)?;
            Ok(())
        }
        None => todo!(),
    }
}

fn from_two_bytes(bytes: &[u8]) -> u16 {
    assert_eq!(bytes.len(), 2);
    (bytes[0] as u16) * 256 + (bytes[1] as u16)
}

/// For each font N in `document`, dump its ToUnicode map to file maps_dir/font-N.toml
fn dump_tounicode_mappings(document: &lopdf::Document, maps_dir: std::path::PathBuf) -> Result<()> {
    for (object_id, object) in &document.objects {
        if let Ok(dict) = object.as_dict() {
            if let Ok(stream_object) = dict.get_deref(b"ToUnicode", &document) {
                let (base_font_name, font_id) = real_font_id(*object_id, &document);
                // map from glyph id (as 4-digit hex string) to set of codepoints.
                // The latter is a set because our PDF assigns the same mapping multiple times, for some reason.
                let mut mapped: HashMap<String, HashSet<u16>> = HashMap::new();
                let stream = stream_object.as_stream()?;
                // TODO: The lopdf library seems to have some difficulty when the stream is an actual CMap file (with comments etc).
                if let Ok(content) = stream.decode_content() {
                    for op in content.operations {
                        let operator = op.operator;
                        if operator == "endbfchar" {
                            for src_and_dst in op.operands.chunks(2) {
                                assert_eq!(src_and_dst.len(), 2);
                                let src = from_two_bytes(src_and_dst[0].as_str()?);
                                let dst = from_two_bytes(src_and_dst[1].as_str()?);
                                if dst != 0 {
                                    mapped
                                        .entry(format!("{:04X}", src))
                                        .or_default()
                                        .insert(dst);
                                }
                            }
                        } else if operator == "endbfrange" {
                            for begin_end_offset in op.operands.chunks(3) {
                                assert_eq!(begin_end_offset.len(), 3);
                                let begin = from_two_bytes(begin_end_offset[0].as_str().unwrap());
                                let end = from_two_bytes(begin_end_offset[1].as_str().unwrap());
                                let offset = from_two_bytes(begin_end_offset[2].as_str().unwrap());
                                for src in begin..=end {
                                    let dst = src - begin + offset;
                                    if dst != 0 {
                                        mapped
                                            .entry(format!("{:04X}", src))
                                            .or_default()
                                            .insert(dst);
                                    }
                                }
                            }
                        }
                    }
                }
                if mapped.len() > 0 {
                    std::fs::create_dir_all(maps_dir.clone())?;
                    let filename =
                        maps_dir.join(basename_for_font(font_id, &base_font_name) + ".toml");
                    println!(
                        "Creating file {:?} for Font {:?} ({}) with {} mappings",
                        filename,
                        font_id,
                        base_font_name,
                        mapped.len()
                    );
                    let toml_string = toml::to_string(&mapped)?;
                    std::fs::write(filename, toml_string)?;
                } else {
                    println!("Font {:?} ({}) ToUnicode empty? (Or maybe it's a CMAP file this library can't handle)", font_id, base_font_name);
                }
            }
        }
    }
    Ok(())
}

enum Phase {
    Phase1Dump,
    Phase2Fix,
}

impl std::str::FromStr for Phase {
    type Err = std::num::ParseIntError;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        if s == "phase2" {
            Ok(Phase::Phase2Fix)
        } else {
            Ok(Phase::Phase1Dump)
        }
    }
}

fn main() -> Result<()> {
    /// Parse a PDF file either to dump text operations (Tj etc) in it,
    /// or to "fix" all text by surrounding them with /ActualText.
    #[derive(Clap)]
    struct Opts {
        // #[clap(parse(from_os_str), value_hint = clap::ValueHint::AnyPath)]
        input_pdf_file: std::path::PathBuf,
        /// Whether to dump (phase 1) or fix (phase 2).
        #[clap(long)]
        phase: Phase,
        /// The directory for either the maps to dump (Phase 1), or maps to read (Phase 2).
        maps_dir: std::path::PathBuf,
        output_pdf_file: Option<std::path::PathBuf>,
    }

    let opts = Opts::parse();
    let filename = opts.input_pdf_file;

    let mut files = TjFiles {
        file: HashMap::new(),
    };

    let start = std::time::Instant::now();
    println!("Loading {:?}", filename);
    let mut document = lopdf::Document::load(&filename).expect("could not load PDF file");
    let end = std::time::Instant::now();
    println!("Loaded {:?} in {:?}", &filename, end.duration_since(start));

    match opts.phase {
        Phase::Phase1Dump => {
            dump_tounicode_mappings(&document, opts.maps_dir.clone())?;
            println!("Done dumping ToUnicode mappings (if any).");
        }
        Phase::Phase2Fix => {}
    }

    let guard = pprof::ProfilerGuard::new(100)?;
    print_text_operators_doc(
        &mut document,
        &opts.maps_dir,
        &mut files,
        &opts.phase,
        opts.output_pdf_file,
    )?;
    if let Ok(report) = guard.report().build() {
        let file = File::create("flamegraph.svg")?;
        report.flamegraph(file)?;
    };
    Ok(())
}

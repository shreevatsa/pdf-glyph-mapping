//! Parses a PDF file, and dumps the following:
//!
//! 1.  For each font, its ToUnicode mapping (if present), with the font's /BaseFont name.
//! 2.  For each font, the operands of each text-showing (`Tj` etc) operation that uses that font.

use anyhow::Result;
use clap::Clap;
use itertools::Itertools;
use lopdf::ObjectId;
use serde_derive::Deserialize;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

macro_rules! indent {
    ($depth:ident) => {
        print!(
            "{}",
            // https://stackoverflow.com/questions/35280798/printing-a-character-a-variable-number-of-times-with-println
            std::iter::repeat("    ").take($depth).collect::<String>()
        );
    };
}

enum Phase {
    Phase1Dump,
    Phase2Fix,
}

/// Parse a PDF file either to dump text operations (Tj etc) in it,
/// or to "fix" all text by surrounding them with /ActualText.
fn main() -> Result<()> {
    #[derive(Clap)]
    struct Opts {
        // #[clap(parse(from_os_str), value_hint = clap::ValueHint::AnyPath)]
        input_pdf_file: std::path::PathBuf,
        /// Whether to dump (phase 1) or fix (phase 2).
        #[clap(long)]
        phase: Phase,
        /// Operate on just a single page (should this take ranges?)
        #[clap(long)]
        page: Option<u32>,
        /// verbose output
        #[clap(long)]
        debug: bool,
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
            /// For each font N in `document`, dump its ToUnicode map to file maps_dir/font-N.toml
            fn _dump_tounicode_mappings(
                document: &lopdf::Document,
                maps_dir: std::path::PathBuf,
            ) -> Result<()> {
                for (object_id, object) in &document.objects {
                    if let Ok(dict) = object.as_dict() {
                        if let Ok(stream_object) = dict.get_deref(b"ToUnicode", &document) {
                            let (base_font_name, font_id) = real_font_id(*object_id, &document)?;
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
                                            let begin =
                                                from_two_bytes(begin_end_offset[0].as_str()?);
                                            let end = from_two_bytes(begin_end_offset[1].as_str()?);
                                            let offset =
                                                from_two_bytes(begin_end_offset[2].as_str()?);
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
                                let filename = maps_dir
                                    .join(basename_for_font(font_id, &base_font_name) + ".toml");
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
            _dump_tounicode_mappings(&document, opts.maps_dir.clone())?;
            println!("Done dumping ToUnicode mappings (if any).");
        }
        Phase::Phase2Fix => {}
    }

    // let guard = pprof::ProfilerGuard::new(100)?;
    /// For each object in `document`, call `process_textops_in_object`.
    /// The main job is to find, for each page, its resource objects and content streams.
    fn _process_textops_in_doc(
        document: &mut lopdf::Document,
        maps_dir: &std::path::PathBuf,
        files: &mut TjFiles,
        phase: &Phase,
        output_pdf_file: Option<std::path::PathBuf>,
        chosen_page_number: Option<u32>,
        debug_depth: usize,
    ) -> Result<()> {
        let mut font_glyph_mappings: HashMap<ObjectId, HashMap<u16, String>> = HashMap::new();
        let pages = document.get_pages();
        println!("{} pages in this document", pages.len());
        let mut seen_ops = linked_hash_map::LinkedHashMap::new();
        for (page_num, page_id) in pages {
            if let Some(p) = chosen_page_number {
                if page_num != p {
                    continue;
                };
            }
            let (maybe_dict, resource_objects) = document.get_page_resources(page_id);
            if page_num % 10 == 0 || debug_depth > 0 {
                println!(
                    "Page number {} has page id {:?} and page resources: {:?} and {:?}",
                    page_num, page_id, maybe_dict, resource_objects
                );
            }
            let mut fonts = lopdf::Dictionary::new();
            if let Some(resource_dict) = maybe_dict {
                if let Ok(f) = resource_dict.get(b"Font") {
                    fonts.extend(f.as_dict()?);
                }
            }
            for object_id in &resource_objects {
                let resource = document.get_object(*object_id)?;
                let dict = resource.as_dict()?;
                if let Ok(f) = dict.get(b"Font") {
                    fonts.extend(f.as_dict()?);
                }
            }
            let mut xobjects_dict = lopdf::Dictionary::new();
            if let Some(resource_dict) = maybe_dict {
                if let Ok(xd) = resource_dict.get(b"XObject") {
                    xobjects_dict.extend(xd.as_dict()?);
                }
            } else {
                assert_eq!(resource_objects.len(), 1);
            }
            for object_id in &resource_objects {
                let resource = document.get_object(*object_id)?;
                let resource_dict = resource.as_dict()?;
                if let Ok(xd) = resource_dict.get(b"XObject") {
                    xobjects_dict.extend(xd.as_dict()?);
                }
            }
            // println!("xobjects_dict: {:?}", xobjects_dict);

            let content_streams = document.get_page_contents(page_id);
            for object_id in content_streams {
                process_textops_in_object(
                    object_id,
                    document,
                    &fonts,
                    &xobjects_dict,
                    maps_dir,
                    files,
                    &mut font_glyph_mappings,
                    phase,
                    debug_depth + (debug_depth > 0) as usize,
                    &mut seen_ops,
                )?;
            }
        }
        println!("Seen the following operators: {:?}", seen_ops);
        if let Phase::Phase2Fix = phase {
            for (k, v) in font_glyph_mappings {
                let map_filename = format!("map-{}-{}.toml", k.0, k.1);
                println!("Creating file: {:?}", map_filename);
                let mut map_for_toml: HashMap<String, String> = HashMap::new();
                for (glyph_id, text) in v {
                    map_for_toml.insert(format!("{:04X}", glyph_id), text);
                }
                let _ = std::fs::write(map_filename, toml::to_vec(&map_for_toml)?);
            }
            if let Some(output_pdf_filename) = output_pdf_file {
                println!("Creating file: {:?}", output_pdf_filename);
                document.save(output_pdf_filename)?;
            } else {
                todo!()
            }
        }
        Ok(())
    }
    _process_textops_in_doc(
        &mut document,
        &opts.maps_dir,
        &mut files,
        &opts.phase,
        opts.output_pdf_file,
        opts.page,
        opts.debug as usize,
    )?;
    // if let Ok(report) = guard.report().build() {
    //     let file = File::create("flamegraph.svg")?;
    //     report.flamegraph(file)?;
    // };
    Ok(())
}

/// For each text operator inside `object_id`, calls either `dump_text_operation` or `wrap_text_operation`.
/// This can call itself, because of the "Do" operator.
fn process_textops_in_object(
    content_stream_object_id: ObjectId,
    document: &mut lopdf::Document,
    fonts: &lopdf::Dictionary,
    xobjects_dict: &lopdf::Dictionary,
    maps_dir: &std::path::PathBuf,
    files: &mut TjFiles,
    font_glyph_mappings: &mut HashMap<ObjectId, HashMap<u16, String>>,
    phase: &Phase,
    debug_depth: usize,
    seen_ops: &mut linked_hash_map::LinkedHashMap<String, u32>,
) -> Result<()> {
    let mut content: lopdf::content::Content = {
        let content_stream = document.get_object(content_stream_object_id)?.as_stream()?;
        let data_to_decode = content_stream
            .decompressed_content()
            .unwrap_or(content_stream.content.clone());
        lopdf::content::Content::decode(&data_to_decode)?
    };
    if debug_depth > 0 {
        indent!(debug_depth);
        // println!("Finding text operators in: {:?}", content);
        println!("Finding text ops among {} ops.", content.operations.len(),);
    }
    let mut current_font: (String, ObjectId) = ("".to_string(), (0, 0));
    let mut current_tm_c = 0.0; // Hack: Keeping track of the current Tm matrix, just its third component will do for now.
    let mut i = 0;
    while i < content.operations.len() {
        let op = &content.operations[i];
        let operator = &op.operator;
        if debug_depth > 0 {
            indent!(debug_depth);
            println!("Operator: {}", operator);
        }
        *seen_ops.entry(operator.clone()).or_insert(0) += 1;
        match operator.as_str() {
            // Setting a new font.
            "Tf" => {
                let font_name = op.operands[0].as_name_str()?;
                let font_id = fonts.get(font_name.as_bytes())?.as_reference()?;
                // println!("Switching to font {}, which means {:?}", font_name, font_id);
                current_font = real_font_id(font_id, document)?;
            }
            // Setting font matrix.
            "Tm" => {
                assert_eq!(op.operands.len(), 6);
                current_tm_c = match op.operands[2].as_f64() {
                    Ok(n) => n,
                    Err(_) => op.operands[2].as_i64()? as f64,
                };
                if debug_depth > 0 {
                    indent!(debug_depth);
                    println!("slant is now: {} from {:?}", current_tm_c, op.operands);
                }
            }
            // An actual text-showing operator.
            "Tj" | "TJ" | "'" | "\"" => {
                let content: &mut lopdf::content::Content = &mut content;
                // TODO: This assumes glyph ids are 16-bit, which is true for "composite" fonts that have a CMAP,
                // but for "simple" fonts, glyph ids are just 8-bit. See 9.4.3 (p. 251) of PDF32000_2008.pdf.
                let glyph_ids = glyph_ids_in_text_operation(content, i)?;
                match phase {
                    // Phase 1: Write to file.
                    Phase::Phase1Dump => {
                        fn _dump_text_operation(
                            glyph_ids: &Vec<u16>,
                            current_font: &(String, ObjectId),
                            maps_dir: &std::path::PathBuf,
                            files: &mut TjFiles,
                        ) -> Result<(), std::io::Error> {
                            let file = files.get_file(maps_dir, current_font.clone());
                            let glyph_hexes: Vec<String> =
                                glyph_ids.iter().map(|n| format!("{:04X} ", n)).collect();
                            glyph_hexes
                                .iter()
                                .for_each(|g| file.write_all(g.as_bytes()).unwrap());
                            file.write_all(b"\n")
                        }
                        _dump_text_operation(&glyph_ids, &current_font, maps_dir, files)?;
                    }
                    Phase::Phase2Fix => {
                        // Phase 2: Wrap the operator in /ActualText.

                        /// Before: content[i] = op.
                        /// After:
                        ///         content[i] = BDC [/Span <</ActualText (...)>>]
                        ///         content[i + 1] = op
                        ///         content[i + 2] = EMC []
                        fn _wrap_text_operation(
                            content: &mut lopdf::content::Content,
                            i: usize,
                            current_font: (String, ObjectId),
                            current_tm_c: f64,
                            font_glyph_mappings: &mut HashMap<ObjectId, HashMap<u16, String>>,
                            glyph_ids: Vec<u16>,
                            maps_dir: &std::path::PathBuf,
                        ) -> Result<usize> {
                            let mytext = actual_text_for(
                                &glyph_ids,
                                current_font,
                                current_tm_c,
                                font_glyph_mappings,
                                maps_dir,
                            )?;
                            // Encode `mytext` into the encoding PDF expects for ActualText.
                            let mytext_encoded_for_actualtext = {
                                /*
                                In the PDF 1.7 spec, see
                                •   "7.3.4.2 Literal Strings" (p. 15, PDF page 23).
                                •   "7.9.2 String Object Types" (p. 85, PDF page 93), especially Table 35 and Figure 7.
                                •   "7.9.2.2 Text String Type" (immediately following the above).
                                 */
                                let mut bytes: Vec<u8> = vec![254, 255];
                                for usv in mytext.encode_utf16() {
                                    let two_bytes = usv.to_be_bytes();
                                    assert_eq!(two_bytes.len(), 2);
                                    for byte in &two_bytes {
                                        bytes.push(*byte);
                                    }
                                }
                                bytes
                            };
                            let dict = lopdf::dictionary!(
                                "ActualText" => lopdf::Object::String(
                                    mytext_encoded_for_actualtext,
                                    lopdf::StringFormat::Hexadecimal));
                            content.operations.insert(
                                i,
                                lopdf::content::Operation::new(
                                    "BDC",
                                    vec![
                                        lopdf::Object::from("Span"),
                                        lopdf::Object::Dictionary(dict),
                                    ],
                                ),
                            );
                            content
                                .operations
                                .insert(i + 2, lopdf::content::Operation::new("EMC", vec![]));
                            return Ok(i + 2);

                            /// The string that be encoded into /ActualText surrounding those glyphs.
                            fn actual_text_for(
                                glyph_ids: &[u16],
                                current_font: (String, ObjectId),
                                current_tm_c: f64,
                                font_glyph_mappings: &mut HashMap<ObjectId, HashMap<u16, String>>,
                                maps_dir: &std::path::PathBuf,
                            ) -> Result<String> {
                                // println!("Looking up font {:?}", current_font);
                                if !font_glyph_mappings.contains_key(&current_font.1) {
                                    let tmp = get_font_mapping(
                                        &current_font.0,
                                        current_font.1,
                                        maps_dir,
                                    )?;
                                    font_glyph_mappings.insert(current_font.1, tmp);
                                }
                                let current_map =
                                    font_glyph_mappings.get_mut(&current_font.1).unwrap();

                                let actual_text_string = glyph_ids
                                    .iter()
                                    .map(|glyph_id| {
                                        if let Some(v) = current_map.get(glyph_id) {
                                            v.to_string()
                                        } else {
                                            println!(
                                                "No mapping found for glyph {:04X} in font {}!",
                                                glyph_id, current_font.0
                                            );
                                            println!("Nevermind, enter replacement text now:");
                                            let replacement: String = text_io::read!("{}\n");
                                            println!("Thanks, using replacement #{}#", replacement);
                                            current_map.insert(*glyph_id, replacement.clone());
                                            replacement
                                        }
                                    })
                                    .join("");
                                // Hack: Surround the ActualText with the font name. Better would be to do this in the equivalent of `pdftotext`.
                                let actual_text_string = format!(
                                    "[{}]{}[/{}]",
                                    current_font.0, actual_text_string, current_font.0
                                );
                                return if current_tm_c > 0.0 {
                                    Ok("[sl]".to_owned() + &actual_text_string + "[/sl]")
                                } else {
                                    Ok(actual_text_string)
                                };

                                // let re1 = regex::Regex::new(r"ि<CCsucc>(([क-ह]्)*[क-ह])").unwrap();
                                // let actual_text_string = re1.replace_all(&actual_text_string, r"\1ि");
                                // let re2 = regex::Regex::new(r"(([क-ह]्)*[क-ह][^क-ह]*)र्<CCprec>").unwrap();
                                // let actual_text_string = re2.replace_all(&actual_text_string, r"र्\1");
                                // // if actual_text_string.contains("<CC") {
                                // //     println!("Some leftovers in #{}#", actual_text_string);
                                // // }
                                // return Ok(actual_text_string.to_string());

                                fn get_font_mapping(
                                    base_font_name: &str,
                                    font_id: ObjectId,
                                    maps_dir: &std::path::PathBuf,
                                ) -> Result<HashMap<u16, String>> {
                                    // let filename = maps_dir.join(format!(
                                    //     "{}.toml",
                                    //     basename_for_font(font_id, base_font_name)
                                    // ));
                                    let glob_pattern = format!(
                                        "{}/*{}.toml",
                                        maps_dir.to_string_lossy(),
                                        base_font_name
                                    );
                                    println!("For font {:?} = {}, looking for map files matching pattern #{}#", font_id, base_font_name, glob_pattern);
                                    let mut filename = PathBuf::new();
                                    for entry in glob::glob(&glob_pattern)
                                        .expect("Failed to read glob pattern")
                                    {
                                        match entry {
                                            Ok(path) => filename = path,
                                            Err(e) => println!(
                                                "While trying to match {}: {:?}",
                                                glob_pattern, e
                                            ),
                                        }
                                    }
                                    println!("Trying to read from filename {:?}", filename);

                                    #[derive(Deserialize)]
                                    struct Replacements {
                                        replacement_text: String,
                                        replacement_codes: Vec<i32>,
                                        replacement_desc: Vec<String>,
                                    }

                                    let m: HashMap<String, Replacements> =
                                        toml::from_slice(&std::fs::read(filename)?)?;
                                    let mut ret = HashMap::<u16, String>::new();
                                    for (glyph_id_str, replacements) in m {
                                        ret.insert(
                                            u16::from_str_radix(&glyph_id_str, 16)?,
                                            replacements.replacement_text,
                                        );
                                    }
                                    Ok(ret)
                                }
                            }
                        }

                        i = _wrap_text_operation(
                            content,
                            i,
                            current_font.clone(),
                            current_tm_c,
                            font_glyph_mappings,
                            glyph_ids,
                            maps_dir,
                        )?
                    }
                };
                let obj = document.get_object_mut(content_stream_object_id)?;
                let stream = obj.as_stream_mut()?;
                stream.set_content(content.encode()?);
            }
            // "Invoke named XObject" (chase the reference; "do" it).
            "Do" => {
                assert_eq!(op.operands.len(), 1);
                let name = &op.operands[0].as_name_str()?;
                let (object_id, stream) = {
                    let mut object = xobjects_dict.get(name.as_bytes())?;
                    let mut id = (0, 0);
                    while let Ok(ref_id) = object.as_reference() {
                        id = ref_id;
                        object = document.objects.get(&ref_id).unwrap();
                    }
                    (id, object.as_stream()?.clone())
                };
                // TODO: Use an Option<Dictionary> instead of allocating a new one.
                let empty_dict = lopdf::Dictionary::new();
                let (fonts, xobjects_dict) = match stream.dict.get(b"Resources") {
                    Ok(value) => {
                        let resources_dict = value.as_dict()?;
                        (
                            resources_dict.get(b"Font")?.as_dict()?,
                            match resources_dict.get(b"XObject") {
                                Ok(rd) => rd.as_dict()?,
                                Err(_) => &empty_dict,
                            },
                        )
                    }
                    Err(_) => (&empty_dict, &empty_dict),
                };
                process_textops_in_object(
                    object_id,
                    document,
                    &fonts,
                    xobjects_dict,
                    maps_dir,
                    files,
                    font_glyph_mappings,
                    phase,
                    debug_depth + (debug_depth > 0) as usize,
                    seen_ops,
                )?;
            }
            // None of the cases we care about.
            _ => {
                // println!(
                //     "Not a text-showing operator: {} {:?}",
                //     &operator, op.operands
                // );
            }
        }
        i += 1;
    }
    return Ok(());

    fn glyph_ids_in_text_operation(
        content: &mut lopdf::content::Content,
        i: usize,
    ) -> Result<Vec<u16>> {
        let op = &content.operations[i];
        let operator = &op.operator;
        let mut bytes: Vec<u8> = Vec::new();
        let text: &[u8] = if operator == "Tj" || operator == "'" {
            assert_eq!(op.operands.len(), 1);
            op.operands[0].as_str()?
        } else if operator == "\"" {
            assert_eq!(op.operands.len(), 3);
            op.operands[2].as_str()?
        } else if operator == "TJ" {
            assert_eq!(op.operands.len(), 1);
            let operands = op.operands[0].as_array()?;
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

        let glyph_ids: Vec<u16> = text
            .chunks(2)
            .map(|chunk| chunk[0] as u16 * 256 + chunk[1] as u16)
            .collect();
        Ok(glyph_ids)
    }
}

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

fn real_font_id(
    font_reference_id: ObjectId,
    document: &lopdf::Document,
) -> Result<(String, ObjectId)> {
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
    let referenced_font = document.get_object(font_reference_id)?.as_dict()?;
    let base_font_name = referenced_font.get(b"BaseFont")?.as_name_str()?.to_string();
    if let Ok(descendant_fonts_object) = referenced_font.get(b"DescendantFonts") {
        let descendant_fonts = descendant_fonts_object.as_array()?;
        assert_eq!(descendant_fonts.len(), 1);
        let descendant_font = document
            .get_object(descendant_fonts[0].as_reference()?)?
            .as_dict()?;
        Ok((
            base_font_name,
            descendant_font.get(b"FontDescriptor")?.as_reference()?,
        ))
    } else {
        Ok((
            base_font_name,
            referenced_font.get(b"FontDescriptor")?.as_reference()?,
        ))
    }
}

fn from_two_bytes(bytes: &[u8]) -> u16 {
    assert_eq!(bytes.len(), 2);
    (bytes[0] as u16) * 256 + (bytes[1] as u16)
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

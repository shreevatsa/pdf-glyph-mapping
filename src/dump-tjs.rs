//! Parses a PDF file, and dumps the following:
//!
//! 1.  For each font, its ToUnicode mapping (if present), with the font's /BaseFont name.
//! 2.  For each font, the operands of each text-showing (`Tj` etc) operation that uses that font.

use clap::Clap;
use itertools::Itertools;
use lopdf::ObjectId;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs::File;
use std::io::Write;

fn filename_for_font(font_id: ObjectId) -> String {
    format!("Tjs-{:04}-{}", font_id.0, font_id.1)
}

struct TjFiles {
    file: HashMap<lopdf::ObjectId, File>,
}
impl TjFiles {
    fn get_file(&mut self, font_id: lopdf::ObjectId) -> &mut File {
        self.file.entry(font_id).or_insert_with(|| {
            let filename = filename_for_font(font_id);
            println!("Creating file: {}", filename);
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
fn print_text_operators_content(
    content: &lopdf::content::Content,
    fonts: &lopdf::Dictionary,
    xobjects_dict: &lopdf::Dictionary,
    document: &lopdf::Document,
    files: &mut TjFiles,
) {
    // println!("Finding text operators in: {:?}", content);
    let mut current_font: ObjectId = (0, 0);
    for op in &content.operations {
        let s = &op.operator;
        if s == "Tf" {
            let font_name = op.operands[0].as_name_str().unwrap();
            let font_id = fonts
                .get(font_name.as_bytes())
                .unwrap()
                .as_reference()
                .unwrap();
            // println!("Switching to font {}, which means {:?}", font_name, font_id);
            current_font = real_font_id(font_id, document).1;
        } else if ["Tj", "TJ", "'", "\""].contains(&s.as_str()) {
            for operand in &op.operands {
                let bytes = operand.as_str().unwrap();
                let glyphs: Vec<u16> = bytes
                    .chunks(2)
                    .map(|chunk| chunk[0] as u16 * 256 + chunk[1] as u16)
                    .collect();
                // Write these to a file. Actually we could write directly from `bytes`....
                let file = files.get_file(current_font);
                let glyph_hexes: Vec<String> =
                    glyphs.iter().map(|n| format!("{:04X} ", n)).collect();
                glyph_hexes
                    .iter()
                    .for_each(|g| file.write_all(g.as_bytes()).unwrap());
                file.write_all(b"\n").unwrap();
            }
        } else if s == "Do" {
            assert_eq!(op.operands.len(), 1);
            let name = &op.operands[0].as_name_str().unwrap();
            let actual_xobject = xobjects_dict.get_deref(name.as_bytes(), &document).unwrap();
            let stream = actual_xobject.as_stream().unwrap();
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
            println!("Fonts and XObjects? {:?} and {:?}", fonts, xobjects_dict);
            let content = stream.decode_content().unwrap();
            print_text_operators_content(&content, fonts, xobjects_dict, document, files);
        } else {
            // println!("Not a text-showing operator: {} {:?}", &s, op.operands);
        }
    }
}

fn print_text_operators_doc(document: &lopdf::Document, files: &mut TjFiles) {
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
                fonts.extend(f.as_dict().unwrap());
            }
        }
        for object_id in &resource_objects {
            let resource = document.get_object(*object_id).unwrap();
            let dict = resource.as_dict().unwrap();
            if let Ok(f) = dict.get(b"Font") {
                fonts.extend(f.as_dict().unwrap());
            }
        }
        println!("Fonts: {:?}", fonts);

        // I couldn't find an easier way to look up an object by name, than looping through potentially multiple resource objects.
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
        println!("xobjects_dict: {:?}", xobjects_dict);

        let content = document.get_and_decode_page_content(page_id).unwrap();
        print_text_operators_content(&content, &fonts, &xobjects_dict, &document, files);
    }
}

fn from_two_bytes(bytes: &[u8]) -> u16 {
    assert_eq!(bytes.len(), 2);
    (bytes[0] as u16) * 256 + (bytes[1] as u16)
}

fn dump_tounicode_mappings(document: &lopdf::Document) {
    for (object_id, object) in &document.objects {
        if let Ok(dict) = object.as_dict() {
            if let Ok(s) = dict.get_deref(b"ToUnicode", &document) {
                let (base_font_name, font_id) = real_font_id(*object_id, &document);
                // Our PDF assigns the same mapping multiple times, for some reason.
                let mut mapped: HashMap<u16, HashSet<u16>> = HashMap::new();
                let ss = s.as_stream().unwrap();
                if let Ok(content) = ss.decode_content() {
                    for op in content.operations {
                        let operator = op.operator;
                        if operator == "endbfchar" {
                            for src_and_dst in op.operands.chunks(2) {
                                assert_eq!(src_and_dst.len(), 2);
                                let src = from_two_bytes(src_and_dst[0].as_str().unwrap());
                                let dst = from_two_bytes(src_and_dst[1].as_str().unwrap());
                                if dst != 0 {
                                    // println!("{:04X} -> {:04X}", src, dst);
                                    mapped.entry(src).or_default().insert(dst);
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
                                        // println!("{:04X} -> {:04X}", src, dst);
                                        mapped.entry(src).or_default().insert(dst);
                                    }
                                }
                            }
                        }
                    }
                }
                if mapped.len() > 0 {
                    let filename = filename_for_font(font_id) + ".map";
                    println!("Creating file: {}", filename);
                    let file = File::create(filename).unwrap();
                    let mut writer = std::io::BufWriter::new(&file);
                    writeln!(&mut writer, "{}", base_font_name).unwrap();
                    for k in mapped.keys().sorted() {
                        writeln!(&mut writer, "{:04X} -> {:04X?}", k, mapped[k]).unwrap();
                    }
                }
            }
        }
    }
}

fn main() {
    #[derive(clap::Clap, Debug)]
    #[clap(name = "dump-tjs")]
    struct Opt {
        #[clap(parse(from_os_str), value_hint = clap::ValueHint::AnyPath)]
        pdf_file: std::path::PathBuf,
    }
    let opt = Opt::parse();
    let filename = opt.pdf_file;

    let mut files = TjFiles {
        file: HashMap::new(),
    };

    let start = std::time::Instant::now();
    println!("Loading {:?}", filename);
    let document = lopdf::Document::load(&filename).expect("could not load PDF file");
    let end = std::time::Instant::now();
    println!("Loaded {:?} in {:?}", &filename, end.duration_since(start));

    dump_tounicode_mappings(&document);
    print_text_operators_doc(&document, &mut files);
}

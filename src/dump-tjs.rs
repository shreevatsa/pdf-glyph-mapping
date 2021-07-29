use clap::Clap;
use clap::ValueHint;
use lopdf;
use lopdf::Dictionary;
use lopdf::ObjectId;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

use std::sync::atomic::{AtomicUsize, Ordering};
static DEPTH: AtomicUsize = AtomicUsize::new(0);
macro_rules! dprintln {
    () => ();
    ($fmt:expr) => ({print!("{}", "       ".repeat(DEPTH.load(Ordering::Relaxed))); println!($fmt)});
    ($fmt:expr, $($arg:tt)*) => ({print!("{}", "       ".repeat(DEPTH.load(Ordering::Relaxed))); println!($fmt, $($arg)*)});
}

fn filename_for_font(font_id: ObjectId) -> String {
    format!("Tjs-{:04}-{}", font_id.0, font_id.1)
}

struct TjFiles {
    file: HashMap<lopdf::ObjectId, File>,
}

impl TjFiles {
    fn get_file(&mut self, font_id: lopdf::ObjectId) -> &mut File {
        return self.file.entry(font_id).or_insert_with(|| {
            let filename = filename_for_font(font_id);
            dprintln!("Creating file: {}", filename);
            File::create(filename).unwrap()
        });
    }
}

fn real_font_id(font_reference_id: ObjectId, document: &lopdf::Document) -> (String, ObjectId) {
    /*
    Consider this example:

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

    Here we want to get from "15454 0" to "40531 0".
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

fn print_text_operators_content(
    content: &lopdf::content::Content,
    fonts: &lopdf::Dictionary,
    xobjects_dict: &lopdf::Dictionary,
    document: &lopdf::Document,
    files: &mut TjFiles,
) {
    // dprintln!("Finding text operators in: {:?}", content);
    let mut current_font: ObjectId = (0, 0);
    for op in &content.operations {
        let s = &op.operator;
        if s == "Tf" {
            // dprintln!("Top-level Tf!");
            let font_name = op.operands[0].as_name_str().unwrap();
            let font_id = fonts
                .get(font_name.as_bytes())
                .unwrap()
                .as_reference()
                .unwrap();
            // dprintln!("Setting font {}, which means {:?}", font_name, font_id);
            current_font = real_font_id(font_id, document).1;
        } else if ["Tj", "TJ", "'", "\""].iter().any(|t| t == &s) {
            for operand in &op.operands {
                let bytes = operand.as_str().unwrap();
                let glyphs: Vec<u16> = bytes
                    .chunks(2)
                    .map(|chunk| chunk[0] as u16 * 256 + chunk[1] as u16)
                    .collect();
                // dprintln!(
                //     "We found a {} for font {}-{} with {} glyphs",
                //     op.operator,
                //     current_font.0,
                //     current_font.1,
                //     glyphs.len()
                // );
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
            dprintln!("{}: {:?} => name {:?}", &s, op.operands, name);
            dprintln!("xobjects_dict: {:?}", xobjects_dict);
            let actual_xobject = xobjects_dict.get_deref(name.as_bytes(), &document).unwrap();
            dprintln!("xobject {} is {:?}", name, actual_xobject);
            let stream = actual_xobject.as_stream().unwrap();
            DEPTH.fetch_add(1, Ordering::Relaxed);
            print_text_operators_stream(&stream, document, files);
            DEPTH.fetch_sub(1, Ordering::Relaxed);
        } else {
            // dprintln!("Not a text-showing operator: {} {:?}", &s, op.operands);
        }
    }
}

fn print_text_operators_stream(
    stream: &lopdf::Stream,
    document: &lopdf::Document,
    files: &mut TjFiles,
) {
    // dprintln!("Stream: {:.80?}", stream);
    dprintln!("Stream...");
    let dict = &stream.dict;
    let empty_dict = lopdf::Dictionary::new();
    let fonts = match dict.get(b"Resources") {
        Ok(value) => {
            let resources_dict = value.as_dict().unwrap();
            resources_dict.get(b"Font").unwrap().as_dict().unwrap()
        }
        Err(_) => &empty_dict,
    };
    dprintln!("Fonts? {:?}", fonts);
    let xobjects_dict = match dict.get(b"Resources") {
        Ok(value) => {
            let resources_dict = value.as_dict().unwrap();
            resources_dict.get(b"XObject").unwrap().as_dict().unwrap()
        }
        Err(_) => &empty_dict,
    };
    dprintln!("Xobjects? {:?}", xobjects_dict);

    let content = stream.decode_content().unwrap();
    DEPTH.fetch_add(1, Ordering::Relaxed);
    print_text_operators_content(&content, fonts, xobjects_dict, document, files);
    DEPTH.fetch_sub(1, Ordering::Relaxed);
}

fn print_text_operators_doc(document: &lopdf::Document, files: &mut TjFiles) {
    let pages = document.get_pages();
    dprintln!("{} pages in this document", pages.len());
    for (page_num, page_id) in pages {
        let (maybe_dict, resource_objects) = document.get_page_resources(page_id);
        dprintln!(
            "Page number {} has page id {:?} and get_page_resources: {:?} and {:?}",
            page_num,
            page_id,
            maybe_dict,
            resource_objects
        );
        let mut fonts = lopdf::Dictionary::new();
        if let Some(resource_dict) = maybe_dict {
            if let Ok(f) = resource_dict.get(b"Font") {
                fonts.extend(f.as_dict().unwrap());
            }
            // dprintln!("Fonts? {:?}", fonts);
        }
        for object_id in &resource_objects {
            let resource = document.get_object(*object_id).unwrap();
            let dict = resource.as_dict().unwrap();
            // dprintln!("Dict? {:?}", dict);
            if let Ok(f) = dict.get(b"Font") {
                fonts.extend(f.as_dict().unwrap());
            }
            // dprintln!("Fonts? {:?}", fonts);
        }
        dprintln!("Fonts: {:?}", fonts);

        // I couldn't find an easier way to look up an object by name, than looping through potentially multiple resource objects.
        let mut xobjects_dict: Dictionary = Dictionary::new();
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
        dprintln!("xobjects_dict: {:?}", xobjects_dict);

        let content = document.get_and_decode_page_content(page_id).unwrap();
        print_text_operators_content(&content, &fonts, &xobjects_dict, &document, files);
    }
}

#[derive(Clap, Debug)]
#[clap(name = "program", about = "This is a program, ok?")]
struct Opt {
    /// Files to process
    #[clap(parse(from_os_str), value_hint = ValueHint::AnyPath)]
    pdf_file: PathBuf,
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
                let mut mapped: HashMap<u16, HashSet<u16>> = HashMap::new();
                let ss = s.as_stream().unwrap();
                if let Ok(content) = ss.decode_content() {
                    for op in content.operations {
                        let operator = op.operator;
                        // println!("An op: {:?}", operator);
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
                            for begin_and_end_and_offset in op.operands.chunks(3) {
                                assert_eq!(begin_and_end_and_offset.len(), 3);
                                let begin =
                                    from_two_bytes(begin_and_end_and_offset[0].as_str().unwrap());
                                let end =
                                    from_two_bytes(begin_and_end_and_offset[1].as_str().unwrap());
                                let offset =
                                    from_two_bytes(begin_and_end_and_offset[2].as_str().unwrap());
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
                // println!("Maps for font {:?} = {:?}: ", object_id, font_id);
                if mapped.len() > 0 {
                    let filename = filename_for_font(font_id) + ".map";
                    dprintln!("Creating file: {}", filename);
                    let file = File::create(filename).unwrap();
                    let mut writer = std::io::BufWriter::new(&file);
                    writeln!(&mut writer, "{}", base_font_name).unwrap();
                    for kv in mapped {
                        // let v: Vec<&u16> = kv.1.iter().collect();
                        let v = kv.1;
                        writeln!(&mut writer, "{:04X} -> {:04X?}", kv.0, v).unwrap();
                    }
                }
            }
        }
    }
}

fn main() {
    println!("Hello, world!");
    let mut files: TjFiles = TjFiles {
        file: HashMap::new(),
    };
    let opt = Opt::parse(); // println!("{:#?}", opt);
    let filename = opt.pdf_file;

    let start = Instant::now();
    dprintln!("Loading {:?}", filename);
    let document = lopdf::Document::load(&filename).expect("could not open PDF file");
    let end = Instant::now();
    dprintln!("Loaded {:?} in {:?}", &filename, end.duration_since(start));

    dump_tounicode_mappings(&document);
    print_text_operators_doc(&document, &mut files);
}

/*

Also, about ToUnicode:

40496 0 obj
<<
  /Length 7117
>>
stream
/CIDInit /ProcSet findresource begin
12 dict begin
begincmap
/CIDSystemInfo
<<  /Registry (Adobe)
/Ordering (UCS)
/Supplement 0
>> def
/CMapName /Adobe-Identity-UCS def
/CMapType 2 def
1 begincodespacerange
<0000> <FFFF>
endcodespacerange
100 beginbfchar
<0003> <0020>
<0005> <0901>
...
<00D6> <0000>
<00D7> <0000>
endbfchar
100 beginbfchar
<00D8> <0000>
<00DB> <0000>
...
<0239> <0000>
<023A> <0000>
endbfchar
57 beginbfchar
<023B> <0000>
<023C> <0000>
...
<030C> <2018>
<030D> <2019>
endbfchar
100 beginbfrange
<0005> <0006> <0901>
<0005> <0007> <0901>
<0006> <0007> <0902>
<0009> <000C> <0905>
...
<0044> <0045> <0940>
<0044> <0047> <0940>
<0045> <0046> <0941>
endbfrange
47 beginbfrange
<0046> <0047> <0942>
<0047> <0048> <0943>
<004B> <004C> <0947>
...
<02F3> <02F4> <0033>
<030C> <030D> <2018>
<030E> <030F> <201C>
endbfrange
endcmap
CMapName currentdict /CMap defineresource pop
end
end
endstream
endobj


 */

use byteorder::{BigEndian, ByteOrder};
use lopdf::{Dictionary, Document, Object, ObjectId};
use serde_derive::{Deserialize, Serialize};
use serde_with::serde_as;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    str::FromStr,
};

macro_rules! indent {
    ($depth:ident) => {
        print!(
            "{}",
            // https://stackoverflow.com/questions/35280798/printing-a-character-a-variable-number-of-times-with-println
            std::iter::repeat("    ").take($depth).collect::<String>()
        );
    };
}

// `ok!(foo)` is like `foo?`, except that in case of error it is printed first. Maybe this is already in the libraries somewhere I don't know.
#[macro_export]
macro_rules! ok {
    ($result:expr) => {
        $result.map_err(|err| {
            println!("Error at line {} column {}: {}", line!(), column!(), err);
            err
        })?
    };
}

pub fn from_many_bytes(bytes: &[u8]) -> u64 {
    assert!(bytes.len() <= 8, format!("Wow, super-long: {:?}", bytes));
    let mut ret = 0;
    for byte in bytes {
        ret = ret * 256 + (*byte as u64)
    }
    ret
}

#[derive(Debug, Clone)]
pub struct Font {
    pub font_descriptor_id: Option<ObjectId>,
    pub base_font_name: Option<String>, // Example: "/BaseFont /APZKLW+NotoSansDevanagari-Bold"
    pub encoding: Option<String>,       // Example: "/Encoding /Identity-H"
    pub subtype: Option<FontSubtype>, // Example: "/Subtype /Type0", refined in /DescendantFonts to "/Subtype /CIDFontType2"
    /*
    See 9.10 Extraction of Text Content (page numbered 292 = PDF page 300 of PDF32000_2008.pdf):

    1. If the font dictionary has a "ToUnicode" entry (a CMap), use it.
    2. "If the font is a simple font that uses one of the predefined encodings MacRomanEncoding, MacExpertEncoding, or WinAnsiEncoding",
       or [all its characters are "known", basically], then (look it up)...
    3. If the font uses one of the predefined CMaps, ...
    4. "An ActualText entry [for a structure element or marked-content sequence]"
     */
    pub to_unicode: Option<()>,      //
    pub font_descriptor: Option<()>, //
}

// See Table 110 in PDF32000_2008.pdf.
#[derive(Debug, Clone)]
pub enum FontSubtype {
    Type0, // A composite font
    Type1,
    MMType1,
    Type3,
    TrueType,
    // CIDFontType0,
    // CIDFontType2
}
impl std::str::FromStr for FontSubtype {
    // TODO: Come up with a more useful kind of error.
    type Err = std::num::ParseIntError;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "Type0" => Ok(FontSubtype::Type0),
            "Type1" => Ok(FontSubtype::Type1),
            "MMType1" => Ok(FontSubtype::MMType1),
            "Type3" => Ok(FontSubtype::Type3),
            "TrueType" => Ok(FontSubtype::TrueType),
            // TODO: Figure out the right kind of error to put here.
            _ => todo!(),
        }
    }
}

// (Character code) ---[Encoding]---> (CIDs) ---[CIDToGIDMap]---> (Glyph IDs) ---[ToUnicode CMap]---> (Unicode scalar values)

// Mapping from character codes to "character selectors" aka CIDs.
#[serde_as]
#[derive(Deserialize, Serialize, Debug)]
pub struct ToUnicodeCMap {
    #[serde_as(as = "Vec<(_, _)>")]
    pub mapped: HashMap<Vec<u8>, HashSet<String>>,
}
impl ToUnicodeCMap {
    pub fn parse(stream_object: &Object) -> anyhow::Result<ToUnicodeCMap> {
        println!("Trying to parse a CMap out of: {:#?}", stream_object);
        // map from glyph id (as 4-digit hex string) to set of codepoints.
        // The latter is a set because our PDF assigns the same mapping multiple times, for some reason.
        let mut mapped: HashMap<Vec<u8>, HashSet<String>> = HashMap::new();
        let content_stream = stream_object.as_stream()?;
        // TODO: The lopdf library seems to have some difficulty when the stream is an actual CMap file (with comments etc).
        let content = {
            match content_stream.decompressed_content() {
                Ok(data) => lopdf::content::Content::decode(&data),
                Err(_) => lopdf::content::Content::decode(&content_stream.content),
            }?
        };
        for op in content.operations {
            println!("An op: {:#?}", op.operator);
            let operator = op.operator;
            if operator == "endbfchar" {
                for src_and_dst in op.operands.chunks(2) {
                    assert_eq!(src_and_dst.len(), 2);
                    println!(
                        "Mapping {:#?} to {:#?}",
                        src_and_dst[0].as_str()?,
                        src_and_dst[1].as_str()?
                    );
                    let dst: Vec<u16> = ok!(src_and_dst[1].as_str())
                        .chunks_exact(2)
                        .map(|chunk| (chunk[0] as u16) * 256 + (chunk[1] as u16))
                        .collect();
                    let dst = ok!(String::from_utf16(&dst));

                    mapped
                        .entry(src_and_dst[0].as_str()?.to_vec())
                        .or_default()
                        .insert(dst);
                }
            } else if operator == "endbfrange" {
                for begin_end_offset in op.operands.chunks(3) {
                    assert_eq!(begin_end_offset.len(), 3);
                    // TODO: Allow more general lengths of bytes.
                    let begin = from_many_bytes(begin_end_offset[0].as_str()?);
                    let end = from_many_bytes(begin_end_offset[1].as_str()?);
                    let offset = from_many_bytes(begin_end_offset[2].as_str()?);
                    for src in begin..=end {
                        let dst = src - begin + offset;
                        if dst != 0 {
                            let mut key = [0; 8];
                            BigEndian::write_u64(&mut key, src);
                            let mut value = [0; 8];
                            BigEndian::write_u64(&mut value, dst);
                            let value: Vec<u16> = value
                                .chunks_exact(2)
                                .map(|chunk| (chunk[0] as u16) * 256 + (chunk[1] as u16))
                                .collect();
                            let value = ok!(String::from_utf16(&value));
                            mapped.entry(key.to_vec()).or_default().insert(value);
                        }
                    }
                }
            }
        }
        Ok(ToUnicodeCMap { mapped })
    }
}

trait DocumentWithFontCache {
    fn get_font() {}
}

impl DocumentWithFontCache for lopdf::Document {}

pub trait OpVisitor {
    fn visit_op(
        &mut self,
        content: &mut lopdf::content::Content,
        i: &mut usize,
        get_font_from_name: &dyn Fn(&str) -> Font,
    );
}

// Copied from lopdf document.rs, and modified.
fn collect_fonts_from_resources<'a>(
    resources: &'a Dictionary,
    fonts: &mut BTreeMap<Vec<u8>, Font>,
    doc: &'a Document,
) {
    if let Ok(font_dict) = resources.get(b"Font").and_then(Object::as_dict) {
        /*
        The list of font resources, something like:
            /Font <<
                /C2_0 13 0 R
                /TT0 14 0 R
                /TT1 15 0 R
            >>
            where the key is the font's page-internal name, and the value is or points to a font dictionary.
        */
        for (name, value) in font_dict.iter() {
            let font = match *value {
                Object::Reference(id) => doc.get_dictionary(id).ok(),
                Object::Dictionary(ref dict) => Some(dict),
                _ => {
                    println!("What? Font /{:?} -> {:?}", name, *value);
                    None
                }
            };
            if !fonts.contains_key(name) {
                font.map(|font| fonts.insert(name.clone(), parse_font(font, doc).unwrap()));
            }
        }
    }
}
fn get_page_fonts(document: &Document, page_id: ObjectId) -> BTreeMap<Vec<u8>, Font> {
    let mut fonts = BTreeMap::new();
    let (resource_dict, resource_ids) = document.get_page_resources(page_id);
    if let Some(resources) = resource_dict {
        collect_fonts_from_resources(resources, &mut fonts, document);
    }
    for resource_id in resource_ids {
        if let Ok(resources) = document.get_dictionary(resource_id) {
            collect_fonts_from_resources(resources, &mut fonts, document);
        }
    }
    fonts
}

/// Go over each page in `document` and, for each operation in its content stream(s), call `visitor.visit_op`.
/// Handles bookkeeping of fonts and resources.
pub fn visit_page_content_stream_ops(
    document: &mut lopdf::Document,
    visitor: &mut dyn OpVisitor,
    chosen_page_number: Option<u32>,
    debug: bool,
) -> anyhow::Result<()> {
    let pages = document.get_pages();
    println!("{} pages in this document.", pages.len());
    let mut seen_ops = linked_hash_map::LinkedHashMap::new();
    // let mut seen_ops = std::collections::HashMap::new();
    for (page_num, page_id) in pages {
        if let Some(p) = chosen_page_number {
            if page_num != p {
                continue;
            };
        }
        let (resource_dict, resource_ids) = document.get_page_resources(page_id);
        if page_num % 10 == 0 || debug {
            println!(
                "Page number {} has page id {:?} and page resources: {:?} and {:?}",
                page_num, page_id, resource_dict, resource_ids
            );
        }
        // This line below is almost what we want, except that it borrows document so we'd end up double-borrowing document.
        // let fonts = document.get_page_fonts(page_id);
        let fonts = get_page_fonts(document, page_id);

        // TODO: Consider something similar to `get_page_fonts` above, if it turns out be necessary.
        let mut xobjects = lopdf::Dictionary::new();
        if let Some(resource_dict) = resource_dict {
            if let Ok(lopdf::Object::Dictionary(ref dict)) = resource_dict.get(b"XObject") {
                xobjects.extend(dict);
            }
        } else {
            // TODO: I've already forgotten why this is.
            assert_eq!(resource_ids.len(), 1);
        }
        for resource_id in &resource_ids {
            if let Ok(resource_dict) = document.get_dictionary(*resource_id) {
                if let Ok(lopdf::Object::Dictionary(ref dict)) = resource_dict.get(b"XObject") {
                    xobjects.extend(dict);
                }
            }
        }

        let content_streams = document.get_page_contents(page_id);
        for object_id in content_streams {
            visit_ops_in_object(
                object_id,
                document,
                Some(&fonts),
                Some(&xobjects),
                debug as usize,
                &mut seen_ops,
                visitor,
            )?;
        }
    }
    println!("Seen the following operators: {:?}", seen_ops);
    Ok(())
}

/// Helper function that actually calls `visitor.visit_op` for each operation inside `object_id`.
/// This can call itself, because of the "Do" operator.
fn visit_ops_in_object(
    content_stream_object_id: lopdf::ObjectId,
    document: &mut lopdf::Document,
    fonts: Option<&BTreeMap<Vec<u8>, Font>>,
    xobjects: Option<&lopdf::Dictionary>,
    debug_depth: usize,
    seen_ops: &mut linked_hash_map::LinkedHashMap<String, u32>,
    // seen_ops: &mut std::collections::HashMap<String, u32>,
    visitor: &mut dyn OpVisitor,
) -> anyhow::Result<()> {
    let mut content = {
        let content_stream = document.get_object(content_stream_object_id)?.as_stream()?;
        match content_stream.decompressed_content() {
            Ok(data) => lopdf::content::Content::decode(&data),
            Err(_) => lopdf::content::Content::decode(&content_stream.content),
        }?
    };
    if debug_depth > 0 {
        indent!(debug_depth);
        // println!("Finding text operators in: {:?}", content);
        println!("Will visit {} ops.", content.operations.len());
    }
    let mut i = 0;
    while i < content.operations.len() {
        let op = &content.operations[i];
        let operator = &op.operator;
        if debug_depth > 0 {
            indent!(debug_depth);
            println!("Operator: {}", operator);
        }

        // No great alternative for these 4 lines yet! https://stackoverflow.com/questions/51542024/how-do-i-use-the-entry-api-with-an-expensive-key-that-is-only-constructed-if-the
        if !seen_ops.contains_key(operator) {
            seen_ops.insert(operator.clone(), 0);
        }
        *seen_ops.get_mut(operator).unwrap() += 1;

        // The main reason for this function: operator "Do" is an instruction to invoke named XObject.
        if operator.as_str() == "Do" {
            assert_eq!(op.operands.len(), 1);
            let name = op.operands[0].as_name_str().unwrap();
            let (object_id, stream) = {
                let mut object = xobjects
                    .unwrap()
                    .get(name.as_bytes())
                    .unwrap_or_else(|_| panic!("XObject name {} not found in {:?}", name, op));
                let mut id = (0, 0);
                while let Ok(ref_id) = object.as_reference() {
                    id = ref_id;
                    object = document.objects.get(&ref_id).unwrap();
                }
                (id, object.as_stream()?.clone())
            };
            let mut fonts = BTreeMap::new();
            let (fonts, xobjects) = match stream.dict.get(b"Resources") {
                Ok(lopdf::Object::Dictionary(ref resources)) => (
                    {
                        collect_fonts_from_resources(resources, &mut fonts, &document);
                        Some(&fonts)
                    },
                    match resources.get(b"XObject") {
                        Ok(lopdf::Object::Dictionary(ref xobjects_dict)) => Some(xobjects_dict),
                        _ => None,
                    },
                ),
                _ => (None, None),
            };
            visit_ops_in_object(
                object_id,
                document,
                fonts,
                xobjects,
                debug_depth + (debug_depth > 0) as usize,
                seen_ops,
                visitor,
            )?;
        } else {
            // TODO: Change this interface. Maybe visit Tf right here, or pass in a map, or something.
            visitor.visit_op(&mut content, &mut i, &|font_name: &str| {
                let font = fonts.unwrap().get(font_name.as_bytes()).unwrap();
                println!("Switching to font {}, which means {:?}", font_name, font);
                font.clone()
            })
        }
        i += 1;
    }
    // The calls to `visit_op` may have changed `content`, so incorporate those changes.
    document
        .get_object_mut(content_stream_object_id)?
        .as_stream_mut()?
        .set_content(content.encode()?);
    Ok(())
}

/// For instance, given the dict for "15454 0", returns ("APZKLW+NotoSansDevanagari-Bold", "40531 0"), in this example:
/// ...
///
/// /F4 15454 0 R
///
/// ...
///
/// 15454 0 obj
/// <<
///   /BaseFont /APZKLW+NotoSansDevanagari-Bold
///   /DescendantFonts [ 40495 0 R ]
///   /Encoding /Identity-H
///   /Subtype /Type0
///   /ToUnicode 40496 0 R
///   /Type /Font
/// >>
/// endobj
///
/// ...
///
/// 40495 0 obj
/// <<
///   /BaseFont /APZKLW+NotoSansDevanagari-Bold
///   /CIDSystemInfo <<
///     /Ordering (Identity)
///     /Registry (Adobe)
///     /Supplement 0
///   >>
///   /CIDToGIDMap /Identity
///   /DW 0
///   /FontDescriptor 40531 0 R
///   /Subtype /CIDFontType2
///   /Type /Font
///
/// ...
pub fn parse_font(referenced_font: &Dictionary, document: &Document) -> anyhow::Result<Font> {
    let base_font_name = ok!(ok!(referenced_font.get(b"BaseFont")).as_name_str()).to_string();
    println!("Looking into referenced_font = {:#?}", referenced_font);

    let encoding = referenced_font.get(b"Encoding")?.as_name_str()?.to_owned();

    fn get_subtype(referenced_font: &lopdf::Dictionary) -> FontSubtype {
        let subtype = referenced_font.get(b"Subtype");
        // println!("It has subtype: {:?}", subtype);
        let subtype = subtype.unwrap().as_name();
        // println!("...which as name is: {:?}", subtype);
        let subtype = FontSubtype::from_str(std::str::from_utf8(subtype.unwrap()).unwrap());
        // println!("...which as FontSubtype is: {:?}", subtype);
        subtype.unwrap()
    }
    let font_subtype = get_subtype(referenced_font);
    let is_composite_font = matches!(font_subtype, FontSubtype::Type0);
    assert!(referenced_font.has(b"DescendantFonts") == is_composite_font);
    // Simple font.
    if !is_composite_font {
        return Ok(Font {
            base_font_name: Some(base_font_name),
            font_descriptor_id: Some(ok!(
                ok!(referenced_font.get(b"FontDescriptor")).as_reference()
            )),
            encoding: Some(encoding),
            subtype: Some(font_subtype),
            to_unicode: None,
            font_descriptor: None,
        });
    }

    // Otherwise, we have DescendantFonts, always a one-element array (see table 121 in PDF32000_2008.pdf). Follow it.
    let descendant_fonts_object = referenced_font.get(b"DescendantFonts").unwrap();
    // But in practice, I've encountered a reference... so follow that first.
    let descendant_fonts_object = match descendant_fonts_object {
        lopdf::Object::Reference(r) => document.get_object(*r).unwrap(),
        _ => descendant_fonts_object,
    };
    match descendant_fonts_object {
        lopdf::Object::Array(arr) => assert_eq!(arr.len(), 1),
        _ => assert!(
            false,
            "Expected a one-element array: Got /DescendantFonts -> #{:?}# in #{:?}#.",
            descendant_fonts_object, referenced_font
        ),
    }
    let descendant_font = ok!(follow_to_dict(document, descendant_fonts_object));
    fn follow_to_dict<'a>(
        document: &'a lopdf::Document,
        object: &'a lopdf::Object,
    ) -> anyhow::Result<&'a lopdf::Dictionary> {
        match object {
            lopdf::Object::Dictionary(d) => Ok(&d),
            lopdf::Object::Reference(r) => follow_to_dict(document, ok!(document.get_object(*r))),
            lopdf::Object::Array(arr) => {
                assert_eq!(arr.len(), 1);
                follow_to_dict(
                    document,
                    ok!(document.get_object(ok!(arr[0].as_reference()))),
                )
            }
            _ => unimplemented!(),
        }
    }

    Ok(Font {
        base_font_name: Some(base_font_name),
        font_descriptor_id: Some(ok!(
            ok!(descendant_font.get(b"FontDescriptor")).as_reference()
        )),
        encoding: Some(encoding),
        subtype: Some(font_subtype),
        to_unicode: None,
        font_descriptor: None,
    })
}

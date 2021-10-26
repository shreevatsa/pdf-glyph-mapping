use std::{collections::HashMap, fs::File, io::Write};

use itertools::Itertools;
use lopdf::ObjectId;
use serde_derive::Deserialize;

use crate::{ok, pdf_visit::ToUnicodeCMap};
use crate::{pdf_visit, Phase};

pub struct TextState {
    pub current_font: pdf_visit::Font,
    // Hack: Keeping track of the current Tm matrix, just its third component will do for now.
    pub current_tm_c: f64,
}

impl TextState {
    #[allow(non_snake_case)]
    fn visit_Tf<F>(&mut self, op: &lopdf::content::Operation, get_font_from_name: F)
    where
        F: FnOnce(&str) -> pdf_visit::Font,
    {
        assert_eq!(op.operator, "Tf");
        assert_eq!(op.operands.len(), 2); // font name (key in Font subdictionary of resource dictionary) and size.
        let font_name = op.operands[0].as_name_str().unwrap();
        self.current_font = get_font_from_name(font_name);
    }

    #[allow(non_snake_case)]
    fn visit_Tm(&mut self, op: &lopdf::content::Operation) {
        assert_eq!(op.operator, "Tm");
        assert_eq!(op.operands.len(), 6);
        self.current_tm_c = match op.operands[2].as_f64() {
            Ok(n) => n,
            Err(_) => op.operands[2].as_i64().unwrap() as f64,
        };
    }

    /// For the PDF text-showing operators (Tj ' " TJ), convert the operand into a vector (the glyph ids in the font).
    /// TODO: This assumes glyph ids are 16-bit, which is true for "composite" fonts that have a CMAP,
    /// but for "simple" fonts, glyph ids are just 8-bit. See 9.4.3 (p. 251) of PDF32000_2008.pdf.
    fn glyph_ids(
        op: &lopdf::content::Operation,
        font_subtype: &pdf_visit::FontSubtype,
    ) -> Vec<u16> {
        let operator = &op.operator;
        let mut bytes: Vec<u8> = Vec::new();
        let text: &[u8] = match operator.as_str() {
            // Tj "Show a text string."
            // '  "Move to the next line and show a text string."
            "Tj" | "'" => {
                assert_eq!(op.operands.len(), 1);
                op.operands[0].as_str().unwrap()
            }
            // "  "Move to the next line and show a text string..." op0 is the word spacing and op1 is the character spacing. op2 is the actual string.
            "\"" => {
                assert_eq!(op.operands.len(), 3);
                op.operands[2].as_str().unwrap()
            }
            // TJ "Show one or more text strings, allowing individual glyph positioning." (operand is an array)
            "TJ" => {
                assert_eq!(op.operands.len(), 1);
                for element in op.operands[0].as_array().unwrap() {
                    match element {
                        lopdf::Object::String(s, _) => bytes.extend(s),
                        // "If it is a number, the operator shall adjust the text position by that amount; that is, it shall translate the text matrix, Tm."
                        // We don't care about this right now.
                        lopdf::Object::Real(_) => {}
                        lopdf::Object::Integer(_) => {}
                        _ => assert!(false, "Unexpected per PDF spec: {:#?}", element),
                    }
                }
                &bytes
            }
            _ => unreachable!(),
        };
        match font_subtype {
            pdf_visit::FontSubtype::Type0 => {
                text.chunks(2).map(|chunk| from_two_bytes(chunk)).collect()
            }
            pdf_visit::FontSubtype::Type1
            | pdf_visit::FontSubtype::MMType1
            | pdf_visit::FontSubtype::Type3
            | pdf_visit::FontSubtype::TrueType => {
                text.chunks(1).map(|chunk| chunk[0] as u16).collect()
            }
        }
    }

    fn visit_text_showing_operator_dump(
        &self,
        glyph_ids: &[u16],
        maps_dir: &std::path::PathBuf,
        files: &mut TjFiles,
    ) {
        let glyph_hexes: Vec<String> = glyph_ids.iter().map(|n| format!("{:04X} ", n)).collect();
        let file = {
            files
                .file
                .entry(self.current_font.font_descriptor_id.unwrap())
                .or_insert_with(|| {
                    let filename = std::path::Path::new(maps_dir).join(
                        basename_for_font(
                            self.current_font.font_descriptor_id.unwrap(),
                            &self.current_font.base_font_name.as_ref().unwrap(),
                        ) + ".Tjs",
                    );
                    println!("Creating file: {:?}", filename);
                    std::fs::create_dir_all(maps_dir.clone()).unwrap();
                    std::fs::File::create(filename).unwrap()
                })
        };
        glyph_hexes
            .iter()
            .for_each(|g| file.write_all(g.as_bytes()).unwrap());
        file.write_all(b"\n").unwrap();
    }

    fn visit_text_showing_operator_wrap(
        &self,
        glyph_ids: &[u16],
        maps_dir: &std::path::PathBuf,
        font_glyph_mappings: &mut HashMap<lopdf::ObjectId, HashMap<u16, String>>,
    ) -> lopdf::Dictionary {
        // Phase 2: Wrap the operator in /ActualText.

        // Before: content[i] = op.
        // After:
        //         content[i] = BDC [/Span <</ActualText (...)>>]
        //         content[i + 1] = op
        //         content[i + 2] = EMC []
        //         i = i + 2
        let current_font = &self.current_font;

        // The string that be encoded into /ActualText surrounding those glyphs.
        let mytext = {
            // println!("Looking up font {:?}", current_font);
            if !font_glyph_mappings.contains_key(&current_font.font_descriptor_id.unwrap()) {
                let font_glyph_mapping = {
                    let base_font_name = &current_font.base_font_name.as_ref().unwrap();
                    let font_id = current_font.font_descriptor_id.unwrap();
                    let glob_pattern =
                        format!("{}/*{}.toml", maps_dir.to_string_lossy(), base_font_name);
                    println!(
                        "For font {:?} = {}, looking for map files matching pattern #{}#",
                        font_id, base_font_name, glob_pattern
                    );
                    let mut filename = std::path::PathBuf::new();
                    for entry in glob::glob(&glob_pattern).expect("Failed to read glob pattern") {
                        match entry {
                            Ok(path) => filename = path,
                            Err(e) => {
                                println!("While trying to match {}: {:?}", glob_pattern, e)
                            }
                        }
                    }
                    println!("Trying to read from filename {:?}", filename);

                    #[derive(Deserialize)]
                    struct Replacements {
                        replacement_text: String,
                        #[serde(rename = "replacement_codes")]
                        _replacement_codes: Vec<i32>,
                        #[serde(rename = "replacement_desc")]
                        _replacement_desc: Vec<String>,
                    }

                    let m: HashMap<String, Replacements> =
                        toml::from_slice(&std::fs::read(filename).unwrap()).unwrap();
                    let mut ret = HashMap::<u16, String>::new();
                    for (glyph_id_str, replacements) in m {
                        ret.insert(
                            u16::from_str_radix(&glyph_id_str, 16).unwrap(),
                            replacements.replacement_text,
                        );
                    }
                    ret
                };

                font_glyph_mappings
                    .insert(current_font.font_descriptor_id.unwrap(), font_glyph_mapping);
            }
            let current_map = font_glyph_mappings
                .get_mut(&current_font.font_descriptor_id.unwrap())
                .unwrap();

            let actual_text_string = glyph_ids
                .iter()
                .map(|glyph_id| {
                    if let Some(v) = current_map.get(glyph_id) {
                        v.to_string()
                    } else {
                        println!(
                            "No mapping found for glyph {:04X} in font {}!",
                            glyph_id,
                            current_font.base_font_name.as_ref().unwrap()
                        );
                        println!("Nevermind, enter replacement text now:");
                        let replacement: String = text_io::read!("{}\n"); // Quiet alternative: format!("[glyph{:04X}]", glyph_id);
                        println!("Thanks, using replacement #{}#", replacement);
                        current_map.insert(*glyph_id, replacement.clone());
                        replacement
                    }
                })
                .join("");
            // Hack: Surround the ActualText with the font name. Better would be to do this in the equivalent of `pdftotext`.
            let actual_text_string = format!(
                "[{}]{}[/{}]",
                current_font.base_font_name.as_ref().unwrap(),
                actual_text_string,
                current_font.base_font_name.as_ref().unwrap()
            );
            if self.current_tm_c > 0.0 {
                "[sl]".to_owned() + &actual_text_string + "[/sl]"
            } else {
                actual_text_string
            }
            // let re1 = regex::Regex::new(r"ि<CCsucc>(([क-ह]्)*[क-ह])").unwrap();
            // let actual_text_string = re1.replace_all(&actual_text_string, r"\1ि");
            // let re2 = regex::Regex::new(r"(([क-ह]्)*[क-ह][^क-ह]*)र्<CCprec>").unwrap();
            // let actual_text_string = re2.replace_all(&actual_text_string, r"र्\1");
            // // if actual_text_string.contains("<CC") {
            // //     println!("Some leftovers in #{}#", actual_text_string);
            // // }
            // return Ok(actual_text_string.to_string());
        };

        let dict = lopdf::dictionary!(
        "ActualText" => lopdf::Object::String(
            pdf_encode_unicode_text_string(&mytext),
            lopdf::StringFormat::Hexadecimal));
        dict
    }
}

pub struct MyOpVisitor {
    pub text_state: TextState,
    pub maps_dir: std::path::PathBuf,
    pub files: TjFiles,
    pub font_glyph_mappings: HashMap<ObjectId, HashMap<u16, String>>,
    pub phase: Phase,
}

impl MyOpVisitor {
    fn visit_text_showing_operator(
        &mut self,
        op: &lopdf::content::Operation,
        content: &mut lopdf::content::Content,
        i: &mut usize,
    ) {
        // First get the list of glyph_ids for this operator.
        let glyph_ids: Vec<u16> =
            TextState::glyph_ids(op, self.text_state.current_font.subtype.as_ref().unwrap());

        match self.phase {
            // Phase 1: Write to file.
            Phase::Phase1Dump => self.text_state.visit_text_showing_operator_dump(
                &glyph_ids,
                &self.maps_dir,
                &mut self.files,
            ),
            Phase::Phase2Fix => {
                let dict = self.text_state.visit_text_showing_operator_wrap(
                    &glyph_ids,
                    &self.maps_dir,
                    &mut self.font_glyph_mappings,
                );
                content.operations.insert(
                    *i,
                    lopdf::content::Operation::new(
                        "BDC",
                        vec![lopdf::Object::from("Span"), lopdf::Object::Dictionary(dict)],
                    ),
                );
                content
                    .operations
                    .insert(*i + 2, lopdf::content::Operation::new("EMC", vec![]));
                *i = *i + 2;
            }
        };
    }

    pub fn dump_font_glyph_mappings(&self) {
        for (k, v) in &self.font_glyph_mappings {
            let map_filename = format!("map-{}-{}.toml", k.0, k.1);
            println!("Creating file: {:?}", map_filename);
            let mut map_for_toml: HashMap<String, String> = HashMap::new();
            for (glyph_id, text) in v {
                map_for_toml.insert(format!("{:04X}", glyph_id), text.to_string());
            }
            let _ = std::fs::write(map_filename, toml::to_vec(&map_for_toml).unwrap());
        }
    }
}

impl pdf_visit::OpVisitor for MyOpVisitor {
    fn visit_op(
        &mut self,
        content: &mut lopdf::content::Content,
        i: &mut usize,
        get_font_from_name: &dyn Fn(&str) -> pdf_visit::Font,
    ) {
        let op = content.operations[*i].clone();
        match op.operator.as_str() {
            // Setting a new font.
            "Tf" => self.text_state.visit_Tf(&op, get_font_from_name),
            // Setting font matrix.
            "Tm" => self.text_state.visit_Tm(&op),
            // An actual text-showing operator.
            "Tj" | "TJ" | "'" | "\"" => {
                self.visit_text_showing_operator(&op, content, i);
            }
            // None of the cases we care about.
            _ => {
                // println!(
                //     "Not a text-showing operator: {} {:?}",
                //     &operator, op.operands
                // );
            }
        }
    }
}

pub fn from_two_bytes(bytes: &[u8]) -> u16 {
    assert_eq!(bytes.len(), 2);
    (bytes[0] as u16) * 256 + (bytes[1] as u16)
}

/// Used for dumping both Tj operands, and unicode mappings ("CMap"s).
fn basename_for_font(font_id: ObjectId, base_font_name: &str) -> String {
    format!("font-{}-{}-{}", font_id.0, font_id.1, base_font_name)
}
pub struct TjFiles {
    pub file: HashMap<lopdf::ObjectId, File>,
}

/// The PDF format expects a particular encoding for Unicode strings:  
/// -   Start with the first two bytes being 254 and 255 (FEFF).  
/// -   Follow up with the encoding of the string into UTF16-BE.  
/// You can see this in the PDF 1.7 spec:  
/// -   *"7.9.2 String Object Types"* (p. 85, PDF page 93), especially Table 35 and Figure 7.  
/// -   *"7.9.2.2 Text String Type"* (immediately following the above).  
/// -   ~~"7.3.4.2 Literal Strings" (p. 15, PDF page 23).~~
fn pdf_encode_unicode_text_string(mytext: &str) -> Vec<u8> {
    let mut bytes: Vec<u8> = vec![254, 255];
    for usv in mytext.encode_utf16() {
        let two_bytes = usv.to_be_bytes();
        assert_eq!(two_bytes.len(), 2);
        for byte in &two_bytes {
            bytes.push(*byte);
        }
    }
    bytes
}

/// For each font N in `document`, dump its ToUnicode map to file maps_dir/font-N.toml
pub fn dump_unicode_mappings(
    document: &mut lopdf::Document,
    maps_dir: std::path::PathBuf,
) -> anyhow::Result<()> {
    for (_object_id, object) in &document.objects {
        if let Ok(dict) = object.as_dict() {
            if let Ok(stream_object) = dict.get_deref(b"ToUnicode", &document) {
                let pdf_font = pdf_visit::parse_font(dict, &document)?;
                let base_font_name = pdf_font.base_font_name.unwrap();
                let font_descriptor_id = pdf_font.font_descriptor_id.unwrap();
                let cmap = ok!(ToUnicodeCMap::parse(stream_object));
                if cmap.mapped.len() > 0 {
                    std::fs::create_dir_all(maps_dir.clone())?;
                    let filename = maps_dir.join(
                        basename_for_font(font_descriptor_id, &base_font_name) + "-cmap.toml",
                    );
                    println!(
                        "Creating file {:?} for Font {:?} ({}) with {} mappings",
                        filename,
                        font_descriptor_id,
                        base_font_name,
                        cmap.mapped.len()
                    );

                    let toml_string = ok!(toml::to_string(&cmap));
                    ok!(std::fs::write(filename, toml_string));
                } else {
                    println!("Font {:?} ({}) ToUnicode empty? (Or maybe it's a CMAP file this library can't handle)", font_descriptor_id, base_font_name);
                }
            }
        }
    }
    println!("Done dumping ToUnicode mappings (if any).");
    Ok(())
}

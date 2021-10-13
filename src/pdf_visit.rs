macro_rules! indent {
    ($depth:ident) => {
        print!(
            "{}",
            // https://stackoverflow.com/questions/35280798/printing-a-character-a-variable-number-of-times-with-println
            std::iter::repeat("    ").take($depth).collect::<String>()
        );
    };
}

macro_rules! ok {
    ($result:expr) => {
        $result.map_err(|err| {
            println!("Error at line {} column {}: {}", line!(), column!(), err);
            err
        })?
    };
}

pub trait OpVisitor {
    fn visit_op(
        &mut self,
        content: &mut lopdf::content::Content,
        i: &mut usize,
        get_font_from_name: &dyn Fn(&str) -> (String, lopdf::ObjectId),
    );
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
        // TODO: See whether this next line will do, and if not, document why not.
        // let fonts = document.get_page_fonts(page_id);
        let mut fonts = lopdf::Dictionary::new();
        if let Some(resource_dict) = resource_dict {
            if let Ok(lopdf::Object::Dictionary(ref dict)) = resource_dict.get(b"Font") {
                fonts.extend(dict);
            }
        }
        for resource_id in &resource_ids {
            if let Ok(resource_dict) = document.get_dictionary(*resource_id) {
                if let Ok(lopdf::Object::Dictionary(ref dict)) = resource_dict.get(b"Font") {
                    fonts.extend(dict);
                }
            }
        }

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
    fonts: Option<&lopdf::Dictionary>,
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
            let (fonts, xobjects) = match stream.dict.get(b"Resources") {
                Ok(lopdf::Object::Dictionary(ref resources)) => (
                    match resources.get(b"Font") {
                        Ok(lopdf::Object::Dictionary(ref font_dict)) => Some(font_dict),
                        _ => None,
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
                let font_id = fonts
                    .unwrap()
                    .get(font_name.as_bytes())
                    .unwrap()
                    .as_reference()
                    .unwrap();
                println!("Switching to font {}, which means {:?}", font_name, font_id);

                font_descriptor_id(font_id, document).unwrap()
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

/// For instance, given "15454 0", returns ("APZKLW+NotoSansDevanagari-Bold", "40531 0"), in this example:
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
pub fn font_descriptor_id(
    font_reference_id: lopdf::ObjectId,
    document: &lopdf::Document,
) -> anyhow::Result<(String, lopdf::ObjectId)> {
    let referenced_font = ok!(ok!(document.get_object(font_reference_id)).as_dict());
    let base_font_name = ok!(ok!(referenced_font.get(b"BaseFont")).as_name_str()).to_string();

    // Simple case: no descendants.
    if !referenced_font.has(b"DescendantFonts") {
        return Ok((
            base_font_name,
            ok!(ok!(referenced_font.get(b"FontDescriptor")).as_reference()),
        ));
    }

    // Otherwise, we have descendant fonts. Follow them (it).
    let descendant_fonts_object = referenced_font.get(b"DescendantFonts").unwrap();
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

    Ok((
        base_font_name,
        ok!(ok!(descendant_font.get(b"FontDescriptor")).as_reference()),
    ))
}

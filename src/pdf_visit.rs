macro_rules! indent {
    ($depth:ident) => {
        print!(
            "{}",
            // https://stackoverflow.com/questions/35280798/printing-a-character-a-variable-number-of-times-with-println
            std::iter::repeat("    ").take($depth).collect::<String>()
        );
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

// For each object in `document`, call `visit_ops_in_object`.
// The main job is to find, for each page, its resource objects and content streams.
pub fn visit_page_content_stream_ops(
    document: &mut lopdf::Document,
    visitor: &mut dyn OpVisitor,
    chosen_page_number: Option<u32>,
    debug_depth: usize,
) -> anyhow::Result<()> {
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
            visit_ops_in_object(
                object_id,
                document,
                &fonts,
                &xobjects_dict,
                debug_depth + (debug_depth > 0) as usize,
                &mut seen_ops,
                visitor,
            )?;
        }
    }
    println!("Seen the following operators: {:?}", seen_ops);
    Ok(())
}

/// Call `visitor.visit_op` for each operation inside `object_id`.
/// This can call itself, because of the "Do" operator.
fn visit_ops_in_object(
    content_stream_object_id: lopdf::ObjectId,
    document: &mut lopdf::Document,
    fonts: &lopdf::Dictionary,
    xobjects_dict: &lopdf::Dictionary,
    debug_depth: usize,
    seen_ops: &mut linked_hash_map::LinkedHashMap<String, u32>,
    visitor: &mut dyn OpVisitor,
) -> anyhow::Result<()> {
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
    let mut i = 0;
    while i < content.operations.len() {
        let op = &content.operations[i];
        let operator = &op.operator;
        if debug_depth > 0 {
            indent!(debug_depth);
            println!("Operator: {}", operator);
        }
        *seen_ops.entry(operator.clone()).or_insert(0) += 1;
        if operator.as_str() == "Do" {
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
            visit_ops_in_object(
                object_id,
                document,
                &fonts,
                xobjects_dict,
                debug_depth + (debug_depth > 0) as usize,
                seen_ops,
                visitor,
            )?;
        } else {
            visitor.visit_op(&mut content, &mut i, &|font_name: &str| {
                let font_id = fonts
                    .get(font_name.as_bytes())
                    .unwrap()
                    .as_reference()
                    .unwrap();
                // println!("Switching to font {}, which means {:?}", font_name, font_id);
                crate::real_font_id(font_id, document).unwrap()
            })
        }
        i += 1;
    }
    document
        .get_object_mut(content_stream_object_id)?
        .as_stream_mut()?
        .set_content(content.encode()?);
    Ok(())
}

use ab_glyph;
// use ab_glyph::ScaleFont;
use image::{DynamicImage, Rgba};
// use rusttype;
use regex::Regex;
use std::convert::TryInto;
use std::io::Write;
// use std::fs::File;
// use std::io::Write;

// pub fn generate_fontdue(file_bytes: &[u8], glyph_index: u16, size: f32) {
//     // Read the font data.
//     let font = file_bytes;
//     // Setup the configuration for how the font will be parsed.
//     let settings = fontdue::FontSettings {
//         scale: size,
//         ..fontdue::FontSettings::default()
//     };
//     // Parse it into the font type.
//     let font = fontdue::Font::from_bytes(font, settings).unwrap();
//     // Rasterize and get the layout metrics for the character at a size.
//     let (metrics, bitmap) = font.rasterize_indexed(glyph_index, size);

//     // Output
//     let mut o = File::create("fontdue.pgm").unwrap();
//     let _ = o.write(format!("P5\n{} {}\n255\n", metrics.width, metrics.height).as_bytes());
//     let _ = o.write(&bitmap);
// }

// pub fn generate_rusttype(file_bytes: &[u8], _glyph_index: u16, size: f32) {
//     // This only succeeds if collection consists of one font
//     let font =
//         rusttype::Font::try_from_bytes(file_bytes as &[u8]).expect("Error constructing Font");
//     println!("{} glyphs in this font.", font.glyph_count());

//     // The font size to use
//     let scale = rusttype::Scale::uniform(size);

//     let colour = (255, 255, 255);

//     let v_metrics = font.v_metrics(scale);
//     let glyphs_height = (v_metrics.ascent - v_metrics.descent).ceil() as u32;

//     for g in 0..font.glyph_count() {
//         let gu: u16 = g.try_into().unwrap();
//         let glyph = font
//             .glyph(rusttype::GlyphId(gu))
//             .scaled(scale)
//             .positioned(rusttype::point(50.0, 30.0));

//         if let Some(bounding_box) = glyph.pixel_bounding_box() {
//             // println!("{:?} is the bounding box for {:?}", bounding_box, glyph);
//             let glyphs_width: u32 = (bounding_box.max.x - bounding_box.min.x) as u32;
//             let mut image =
//                 DynamicImage::new_rgba8(glyphs_width + 80, glyphs_height + 80).to_rgba8();
//             // Draw the glyph into the image per-pixel by using the draw closure
//             glyph.draw(|x, y, v| {
//                 image.put_pixel(
//                     // Offset the position by the glyph bounding box
//                     x + bounding_box.min.x as u32,
//                     y + bounding_box.min.y as u32,
//                     // Turn the coverage into an alpha value
//                     Rgba([colour.0, colour.1, colour.2, (v * 255.0) as u8]),
//                 )
//             });
//             // Save the image to a png file
//             let filename = format!("image_rusttype-{}.png", g);
//             image.save(&filename).unwrap();
//             println!("Generated: {}", filename);
//         } else {
//             println!("No bounding box for {:?}", glyph);
//         }
//     }
// }

// https://stackoverflow.com/a/59211505
fn catch_unwind_silent<F: FnOnce() -> R + std::panic::UnwindSafe, R>(
    f: F,
) -> std::thread::Result<R> {
    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let result = std::panic::catch_unwind(f);
    std::panic::set_hook(prev_hook);
    result
}

pub fn generate_ab_glyph(filename: PathBuf, size: f32) {
    use ab_glyph::Font;
    let file_bytes =
        &std::fs::read(&filename).unwrap_or_else(|_| panic!("Could not open file {:?}", filename));

    let re = Regex::new(r"font-(?P<font_id>[0-9]*).ttf$").unwrap();
    let font_id = match re.captures(filename.to_str().unwrap()) {
        Some(captures) => captures.name("font_id").unwrap().as_str(),
        None => "",
    };

    let font = ab_glyph::FontRef::try_from_slice(file_bytes).expect("Error constructing FontRef");
    println!("{} glyphs in this font.", font.glyph_count());

    /*
    What should we use for position?

    min.x, min.y

                    max.x, max.y

    Looks like output starts with (0, 0) as min.x, min.y, so we probably want to shift it.

    We need the width and height to be large enough that all of these hold:
            0 <= position.x + min.x <= position.x + max.x < w
            0 <= position.y + min.y <= position.y + max.y < h
    The former means that:
            position.x >= -min.x, over all glyphs. The smallest such value, namely max(-min.x), is position.x.
    The latter means that:
            w > position.x + max.x for this particular glyph.
     */
    // First pass: find good position and width and height.
    let mut x_min = 1000;
    let mut x_max = -1000;
    let mut y_min = 1000;
    let mut y_max = -1000;
    for g in 0..font.glyph_count() {
        let gu: u16 = g.try_into().unwrap();
        let glyph_id = ab_glyph::GlyphId(gu);
        let glyph = glyph_id.with_scale_and_position(size, ab_glyph::point(0.0, 0.0));
        if let Some(q) = font.outline_glyph(glyph.clone()) {
            x_min = std::cmp::min(x_min, q.px_bounds().min.x as i32);
            x_max = std::cmp::max(x_max, q.px_bounds().max.x as i32);
            y_min = std::cmp::min(y_min, q.px_bounds().min.y as i32);
            y_max = std::cmp::max(y_max, q.px_bounds().max.y as i32);
        }
        let filename = format!("font-{}-glyph-{:04X}.txt", font_id, gu);
        println!("Creating file: {}", filename);
        let file = std::fs::File::create(filename).unwrap();
        let mut writer = std::io::BufWriter::new(&file);
        let h_advance = match catch_unwind_silent(|| font.h_advance_unscaled(glyph_id)) {
            Ok(v) => v,
            Err(_) => -9999.0,
        };
        let v_advance = match catch_unwind_silent(|| font.v_advance_unscaled(glyph_id)) {
            Ok(v) => v,
            Err(_) => -9999.0,
        };
        let px_bounds = match font.outline_glyph(glyph.clone()) {
            Some(q) => q.px_bounds(),
            None => ab_glyph::Rect {
                min: ab_glyph::point(-42., -42.),
                max: ab_glyph::point(-42., -42.),
            },
        };
        write!(
            &mut writer,
            r#"
{:04X}: {:?}
{:?}  # px_bounds
{:?}  # glyph_bounds
{:?} and {:?} # advances.
"#,
            gu,
            glyph,
            px_bounds,
            font.glyph_bounds(&glyph),
            h_advance,
            v_advance,
        )
        .unwrap();
    }

    // Second pass: Draw it.
    let position = ab_glyph::point(-x_min as f32, -y_min as f32);
    // A common height because the images will be laid out side-by-side and we want their baselines to align.
    let height = position.y as i32 + y_max + 1;
    for g in 0..font.glyph_count() {
        let gu: u16 = g.try_into().unwrap();
        let glyph_id = ab_glyph::GlyphId(gu);

        let glyph = glyph_id.with_scale_and_position(size, position);
        let colour = (0, 0, 0);
        if let Some(q) = font.outline_glyph(glyph.clone()) {
            let mut image = DynamicImage::new_rgba8(
                (position.x + q.px_bounds().max.x + 1.0) as u32,
                height as u32,
            )
            .to_rgba8();
            println!(
                r#"For {:04X} which has px_bounds {:?}, creating an image {:?} by {:?}"#,
                g,
                q.px_bounds(),
                image.width(),
                image.height()
            );
            q.draw(|x, y, c| {
                /* draw pixel `(x, y)` with coverage: `c` */
                image.put_pixel(
                    // Offset the position by the glyph bounding box.
                    x + q.px_bounds().min.x as u32,
                    y + q.px_bounds().min.y as u32,
                    // Turn the coverage into an alpha value
                    Rgba([colour.0, colour.1, colour.2, (c * 255.0) as u8]),
                )
            });
            // Save the image to a png file
            let filename = format!("font-{}-glyph-{:04X}.png", font_id, gu);
            image.save(&filename).unwrap();
            println!("Generated: {}", filename);
        } else {
            // println!("No bounding box for GlyphId {:04X}", gu);
        }
    }
}
use clap::Clap;
use clap::ValueHint;
use std::path::PathBuf;

#[derive(Clap, Debug)]
#[clap(name = "program", about = "This is a program, ok?")]
struct Opt {
    /// Files to process
    #[clap(parse(from_os_str), value_hint = ValueHint::AnyPath)]
    font_file: PathBuf,
}

fn main() {
    let opt = Opt::parse();
    // println!("{:#?}", opt);

    // Load the font
    // let font_data = include_bytes!("../../simple-indic-tex/font-0012.ttf");
    // let font_data = include_bytes!("../../../noto/NotoSansDevanagari-Regular.ttf");
    // let font_data = include_bytes!("../../page-200/font-0020.ttf");

    // const SIZE: f32 = 200.0;
    // generate_fontdue(font_data, 561, SIZE);
    // std::process::exit(0);

    // let start1 = std::time::Instant::now();
    // generate_rusttype(font_data, 561, 32.0);
    // let duration1 = start1.elapsed();
    // let start2 = std::time::Instant::now();
    generate_ab_glyph(opt.font_file, 30.0);
    // let duration2 = start2.elapsed();
    // println!("Elapsed time: {:.2?} and {:.2?}", duration1, duration2);
}

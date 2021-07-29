use ab_glyph;
use image::{DynamicImage, Rgba};
use regex::Regex;
use std::convert::TryInto;

fn generate_ab_glyph(filename: PathBuf, size: f32) {
    use ab_glyph::Font;
    let file_bytes = &std::fs::read(&filename).unwrap();
    // TODO: take the font_id as input, instead of parsing from filename!
    let re = Regex::new(r"font-(?P<font_id>[0-9]*).ttf$").unwrap();
    let font_id = match re.captures(filename.to_str().unwrap()) {
        Some(captures) => captures.name("font_id").unwrap().as_str(),
        None => "",
    };

    let font = ab_glyph::FontRef::try_from_slice(file_bytes).expect("Error constructing FontRef");
    println!("{} glyphs in this font.", font.glyph_count());

    /*
    What should we use for position and width for each glyph?

    min.x, min.y

                    max.x, max.y

    Output starts with (0, 0) rather than (min.x, min.y), so we probably want to shift each point.
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
        if let Some(q) = font.outline_glyph(glyph) {
            x_min = std::cmp::min(x_min, q.px_bounds().min.x as i32);
            x_max = std::cmp::max(x_max, q.px_bounds().max.x as i32);
            y_min = std::cmp::min(y_min, q.px_bounds().min.y as i32);
            y_max = std::cmp::max(y_max, q.px_bounds().max.y as i32);
        }
    }

    // Second pass: Draw it.
    let position = ab_glyph::point(-x_min as f32, -y_min as f32);
    // A common height because the images will be laid out side-by-side and we want their baselines to align.
    let height = position.y as i32 + y_max + 1;
    for g in 0..font.glyph_count() {
        let glyph_id: u16 = g as u16;
        let glyph = ab_glyph::GlyphId(glyph_id).with_scale_and_position(size, position);
        let colour = (0, 0, 0);
        // We can generate images only for glyphs for which we have outlines.
        if let Some(q) = font.outline_glyph(glyph) {
            let mut image = DynamicImage::new_rgba8(
                (position.x + q.px_bounds().max.x + 1.0) as u32,
                height as u32,
            )
            .to_rgba8();
            q.draw(|x, y, c| {
                // draw pixel `(x, y)` with coverage `c` (=what fraction of the pixel the glyph covered).
                image.put_pixel(
                    // Offset the position so they appear properly; see the comment above "First pass".
                    x + q.px_bounds().min.x as u32,
                    y + q.px_bounds().min.y as u32,
                    // Using the "coverage" as the PNG image's "alpha value".
                    Rgba([colour.0, colour.1, colour.2, (c * 255.0) as u8]),
                )
            });
            let filename = format!("font-{}-glyph-{:04X}.png", font_id, glyph_id);
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
#[clap(name = "dump-glyphs")]
struct Opt {
    #[clap(parse(from_os_str), value_hint = ValueHint::AnyPath)]
    font_file: PathBuf,
}

fn main() {
    let opt = Opt::parse();
    generate_ab_glyph(opt.font_file, 30.0);
}

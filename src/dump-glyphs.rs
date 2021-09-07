//! Reads a (TTF) font file, and dumps a bitmap image for each glyph in it.
use ab_glyph::Font;
use anyhow::{Context, Result};
use clap::Clap;
use image::{DynamicImage, Rgba};

fn dump_glyphs(opts: Opts, size: f32) -> Result<()> {
    let filename = opts.font_file;
    let output_dir = std::path::Path::new(&opts.output_dir).join(filename.file_name().unwrap());
    std::fs::create_dir_all(output_dir.clone())?;

    let file_bytes = &std::fs::read(&filename)?;
    let font = ab_glyph::FontRef::try_from_slice(file_bytes).expect("Error constructing FontRef");
    println!("This font has {} glyphs.", font.glyph_count());

    /*
    What should we use for position and width for each glyph?
    Calling px_bounds() gives `min` and `max`, which correspond to something like this:

    min.x, min.y

                    max.x, max.y

    Output starts with (0, 0) rather than (min.x, min.y), so we want to translate each point by some `shift`.
    We need the width and height to be large enough that all of these hold:
            0 <= shift.x + min.x <= shift.x + max.x < w
            0 <= shift.y + min.y <= shift.y + max.y < h
    The former means that:
            shift.x >= -min.x, over all glyphs. The smallest such value, namely max(-min.x), is shift.x.
    The latter means that:
            w > shift.x + max.x for this particular glyph.
     */
    // First pass: find good `shift` and width and height.
    let mut x_min = i32::MAX; // empty min = infinity, etc.
    let mut x_max = i32::MIN;
    let mut y_min = i32::MAX;
    let mut y_max = i32::MIN;
    for g in 0..font.glyph_count() {
        let glyph =
            ab_glyph::GlyphId(g as u16).with_scale_and_position(size, ab_glyph::point(0.0, 0.0));
        if let Some(q) = font.outline_glyph(glyph) {
            x_min = std::cmp::min(x_min, q.px_bounds().min.x as i32);
            x_max = std::cmp::max(x_max, q.px_bounds().max.x as i32);
            y_min = std::cmp::min(y_min, q.px_bounds().min.y as i32);
            y_max = std::cmp::max(y_max, q.px_bounds().max.y as i32);
        }
    }
    assert_ne!(x_min, i32::MAX);
    assert_ne!(x_max, i32::MIN);
    assert_ne!(y_min, i32::MAX);
    assert_ne!(y_max, i32::MIN);
    let shift = ab_glyph::point(-x_min as f32, -y_min as f32);

    // Second pass: Draw each glyph.
    // A common height because the images will be laid out side-by-side and we want their baselines to align.
    let height = shift.y as i32 + y_max + 1;
    for g in 0..font.glyph_count() {
        let glyph_id: u16 = g as u16;
        let glyph = ab_glyph::GlyphId(glyph_id).with_scale_and_position(size, shift);
        let colour = (0, 0, 0);
        // We can generate images only for glyphs for which we have outlines.
        if let Some(q) = font.outline_glyph(glyph) {
            let mut image = DynamicImage::new_rgba8(
                (shift.x + q.px_bounds().max.x + 1.0) as u32,
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
            let output_filename = output_dir.join(format!("glyph-{:04X}.png", glyph_id));
            image
                .save(&output_filename)
                .with_context(|| format!("Failed to write to {:?}", output_filename))?;

            println!("Generated: {:#?}", output_filename);
        } else {
            // println!("No bounding box for GlyphId {:04X}", gu);
        }
    }
    Ok(())
}

#[derive(Clap, Debug)]
struct Opts {
    font_file: std::path::PathBuf,
    output_dir: std::path::PathBuf,
}

fn main() -> Result<()> {
    let opts = Opts::parse();
    dump_glyphs(opts, 30.0)?;
    Ok(())
}

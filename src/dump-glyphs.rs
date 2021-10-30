//! Reads a (TTF) font file, and dumps a bitmap image for each glyph in it.
//!
//! A TTF font file contains, among other things, a description of the shape of each character (glyph) in the font.
//! This program reads a font file, and dumps bitmap images into a directory.

use ab_glyph::GlyphId;
use clap::Clap;
/// These are the two command-line options the program takes.
#[derive(Clap, Debug)]
struct Opts {
    font_file: std::path::PathBuf,
    output_dir: std::path::PathBuf,
}

use anyhow::{Context, Result};
use image::DynamicImage;
use std::path::PathBuf;
/// As mentioned above: This program reads a font file, and dumps bitmap images into a directory.
fn main() -> Result<()> {
    let opts = Opts::parse();
    let font_file_contents = std::fs::read(&opts.font_file)?;
    dump_glyphs(&font_file_contents, opts.output_dir, 30.0)?;
    Ok(())
}

/// Parse `font_file_contents` as a font, and dump its glyphs into `output_dir`.
///
/// Returns an iterator over optional (glyph_id, image) pairs -- the return type will be cleaner
/// [when Rust has generators](https://stackoverflow.com/questions/16421033/lazy-sequence-generation-in-rust).
///
/// # Implementation
///
/// We use the `ab_glyph` crate to parse the font, and generate images for each glyph.
/// Specifically (see the example in its crate documentation), it has a `ab_glyph::OutlinedGlyph::draw`
/// function, which calls a callback for each position (x, y) and "coverage" c.
fn dump_glyphs(font_file_contents: &[u8], output_dir: PathBuf, size: f32) -> Result<()> {
    use ab_glyph::{Font, FontRef};
    use image::Rgba;

    let font =
        FontRef::try_from_slice(font_file_contents).with_context(|| "Could not parse font.")?;
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
        let glyph = GlyphId(g as u16).with_scale_and_position(size, ab_glyph::point(0.0, 0.0));
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
        let glyph = GlyphId(glyph_id).with_scale_and_position(size, shift);
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
            println!("No bounding box for GlyphId {:04X}", g);
        }
    }
    Ok(())
}

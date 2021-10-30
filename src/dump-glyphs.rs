//! Reads a (TTF) font file, and dumps a bitmap image for each glyph in it.
//!
//! A TTF font file contains, among other things, a description of the shape of each character (glyph) in the font.
//! This program reads a font file, and dumps bitmap images into a directory.

use clap::Clap;
use std::path::{Path, PathBuf};

/// These are the command-line options the program takes.
#[derive(Clap, Debug)]
struct Opts {
    /// Path to the TTF file whose glyphs are to be extracted.
    font_file: PathBuf,
    /// Directory in which to create the bitmap images.
    output_dir: PathBuf,
    /// Approximate height in pixels of the generated images.
    #[clap(short, long, default_value = "30.0")]
    size: f32,
}

use anyhow::{Context, Result};
/// As mentioned above: This program reads a font file, and dumps bitmap images into a directory.
fn main() -> Result<()> {
    let opts = Opts::parse();
    let font_file_contents = std::fs::read(&opts.font_file)?;
    let output_dir = Path::new(&opts.output_dir).join(opts.font_file.file_name().unwrap());
    dump_glyphs(&font_file_contents, output_dir, opts.size)?;
    Ok(())
}

/// Parse `font_file_contents` as a font, and dump its glyphs into `output_dir`.
/// All the glyph images will be of height approximately `size` pixels.
///
/// # Implementation notes
///
/// We use the `ab_glyph` crate to parse the font, and generate images for each glyph.
/// Specifically, `ab_glyph` has a function `ab_glyph::OutlinedGlyph::draw`, which calls a
/// callback for each position (x, y) and "coverage" c.
///
/// Per the example in the `ab_glyph` crate documentation, we could do something like this:
///
/// ```below-is-actually-rust-but-not-doctests-see-https://github.com/rust-lang/rust/issues/63193
/// use ab_glyph::{FontRef, Glyph, GlyphId, point};
/// let font = FontRef::try_from_slice(font_file_contents)?;
/// let glyph: Glyph = GlyphId(42)
///     .with_scale_and_position(/*scale=*/ 24, /*position=*/ point(100.0, 0.0));
/// // Draw it.
/// if let Some(g) = font.outline_glyph(glyph) {
///     g.draw(|x, y, c| { /* draw pixel `(x, y)` with coverage: `c` */ });
/// }
/// ```
///
/// The main question is: what `position` to use? (This is the horizontal/vertical "offset" at which
/// to place the glyph outline, before calling `draw` to rasterize it.) The natural choice would seem
/// to be (0, 0), but this has two problems:
///
/// -   Some glyphs may extend beyond their stated box, so this may result in some positions (x, y)
/// having x or y negative. The callback called by `draw` would have to handle them.
/// -   Conversely, it may happen (e.g. imagine a glyph for a "dot" character) that there is abundant
/// room on all sides (the width or height of the image is too large), while it would be nice to have
/// "tight" bounding boxes instead.
///
/// So what we do instead is:
///
/// 1.  First, use the position (0, 0), and compute the resulting bounding boxes for each glyph.
///
///     -   This is available as `px_bounds()` on the `OutlinedGlyph`, which gives a `Rect` with
///         the top-left point called `min` and the bottom-right called `max`, like:
///
///                 (min.x, min.y)
///                                ....
///                                      (max.x, max.y)
///          
/// 2.  Next, use these bounds (of each glyph) to determine `(bx, by)`, the "best" position to use:
///
///     -   The resulting pixel bounds of the top-left corner would (modulo rounding issues?) be `(bx + min.x, by + min.y)`.
///
///     -   We want each of these to be nonnegative, so `bx >= -min.x` and `by >= -min.y` for each glyph.
///
///     -   So the smallest possible `bx` and `by` are max(-min.x) and max(-min.y).
///
/// 3.  This also gives us the height and width to use for the image:
///
///     -   The resulting pixel bounds of the bottom-right corner would be `(bx + max.x, by + max.y)`.
///
///     -   The max of these `bx + max.x` is the width, and the max of these `by + max.y` is the height.
fn dump_glyphs(font_file_contents: &[u8], output_dir: PathBuf, size: f32) -> Result<()> {
    use ab_glyph::{Font, FontRef, GlyphId, Point};
    use image::{DynamicImage, Rgba};

    let font =
        FontRef::try_from_slice(font_file_contents).with_context(|| "Could not parse font.")?;
    println!("This font has {} glyphs.", font.glyph_count());

    // First pass: find good `shift` and width and height.
    let mut x_min = i32::MAX; // empty min = infinity, etc.
    let mut x_max = i32::MIN;
    let mut y_min = i32::MAX;
    let mut y_max = i32::MIN;
    for g in 0..font.glyph_count() {
        let glyph = GlyphId(g as u16).with_scale_and_position(size, Point { x: 0.0, y: 0.0 });
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
    let shift = Point {
        x: -x_min as f32,
        y: -y_min as f32,
    };

    // A common height, because when glyph images are laid out side-by-side, we want their baselines to align.
    // Adding an extra pixel at the bottom, for reasons I can't remember (perhaps not needed).
    let height = shift.y as i32 + y_max + 1;

    // Second pass: Draw each glyph.
    std::fs::create_dir_all(output_dir.clone())?;
    for glyph_id in 0..font.glyph_count() {
        let glyph = GlyphId(glyph_id as u16).with_scale_and_position(size, shift);
        // `outline_glyph` can return None when bounds are invalid for whatever reason.
        if let Some(q) = font.outline_glyph(glyph) {
            // These better be true, because we picked `shift` such that it is.
            assert!(q.px_bounds().min.x >= 0.0);
            assert!(q.px_bounds().min.y >= 0.0);
            let width = shift.x + q.px_bounds().max.x + 1.0;
            let mut image = DynamicImage::new_rgba8(width as u32, height as u32).to_rgba8();
            q.draw(|x, y, c| {
                // Draw pixel `(x, y)` with coverage `c` (= what fraction of the pixel the glyph covered).
                // It seems that the `(x, y)` positions with which `draw` calls the callback would put the
                // entire glyph at the top-left corner of the image. They are relative to the `q.px_bounds().min`.
                image.put_pixel(
                    x + q.px_bounds().min.x as u32,
                    y + q.px_bounds().min.y as u32,
                    // Using black (#000000) as colour, and the "coverage" fraction as the PNG image's "alpha" value.
                    Rgba([0, 0, 0, (c * 255.0) as u8]),
                )
            });
            let output_filename = output_dir.join(format!("glyph-{:04X}.png", glyph_id));
            image
                .save(&output_filename)
                .with_context(|| format!("Failed to write to {:?}", output_filename))?;

            println!("Generated: {:#?}", output_filename);
        } else {
            println!(
                "No outline for GlyphId {:04X}: are bounds invalid?",
                glyph_id
            );
        }
    }
    Ok(())
}

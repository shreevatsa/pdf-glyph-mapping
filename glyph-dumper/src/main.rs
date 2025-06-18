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
    /// A comma-separated list of which glyphs to dump images for (default: all glyphs).
    #[clap(short, long, use_delimiter = true)]
    glyphs: Option<Vec<u16>>,
}

use anyhow::{Context, Result};
/// As mentioned above: This program reads a font file, and dumps bitmap images into a directory.
fn main() -> Result<()> {
    let opts = Opts::parse();
    println!("Opening file {}", opts.font_file.display());
    let font_file_contents = std::fs::read(&opts.font_file)?;
    let output_dir = Path::new(&opts.output_dir).join(opts.font_file.file_name().unwrap());
    dump_glyphs(&font_file_contents, output_dir, opts.size, opts.glyphs)?;
    Ok(())
}

/// Parses `font_file_contents` as a font, and dumps its glyphs into `output_dir`.
/// All the glyph images will be of height approximately `size` pixels.
///
/// # Implementation notes
///
/// We use the `ab_glyph` crate to parse the font, and generate images for each glyph.
/// Specifically, `ab_glyph` has a function `ab_glyph::OutlinedGlyph::draw`, which calls a
/// callback for each position (x, y) and "coverage" c (i.e. what fraction of the pixel (x, y)
/// is covered by the ideal outline: we can use this value as the pixel's darkness, or in fact
/// its opacity ("alpha" in PNG), to get a reasonable image).
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
/// When we try this, a problem arises: yes, we can draw each glyph's image individually, but they will
/// all be of different heights and widths, and although different glyphs occupy different areas relative
/// to each other (e.g. `^` occurs higher than `-`), it turns out that the `(x, y)` coordinates with which
/// `draw` invokes its callback always start at (0, 0): they seem to be relative to the bounding box
/// of the glyph (in fact, it seems to go row-by-row: for each `y`=0, 1, 2, …, for each `x`=0, 1, 2, …),
/// i.e. if we naively generate images of equal height using `draw`, then every glyph will be placed
/// at the top-left corner of its image.
///
/// (This was observed empirically in the fonts I tried, but I couldn't find it documented anywhere
/// what the `(x, y)` coordinates would be, or meant: neither in the `ab_glyph` documentation, nor
/// in `ttf-parser` that it is based on. Ultimately, I gather from looking at
/// [the OpenType spec](https://docs.microsoft.com/en-us/typography/opentype/otspec183/ttch01#funits-and-the-grid)
/// that the coordinates are essentially arbitrary.
///
/// In the font I tried (specifically, I looked at these glyphs in `font-40532.ttf`):
///
///     743 = 02E7 comma      bounds are Rect { min: point(0.0, -3.0), max: point(5.0, 3.0) }
///     745 = 02E9 full stop  bounds are Rect { min: point(1.0, -3.0), max: point(5.0, 1.0) }
///     6   = 0006 anusvara   bounds are Rect { min: point(-5.0, -20.0), max: point(-1.0, -14.0) }
///     510 = 01FE ddha       bounds are Rect { min: point(0.0, -15.0), max: point(26.0, 10.0) }
///
/// — it seems (see the next paragraph for what the bounds above mean) that `y=0` is the baseline, and
/// `x=0` is something like the left corner (and glyphs that attach to the previous one have negative `x`).
/// Whether or not this is common, the point is that the `draw` coordinates aren't usable as-is.)
///
/// Fortunately, the `px_bounds()` function on the `OutlinedGlyph` provides a workaround: it returns
/// a `Rect` with the top-left corner called `min` and the bottom-right corner called `max`, like:
///
///                 (min.x, min.y)
///                                ....
///                                      (max.x, max.y)
///
/// These bounds (see the example above) seem to be consistent between each other (again, I
/// couldn't find this documented anywhere) so we can interpret each `draw` call's `(x, y)`
/// coordinates by comparing the top-left corner of the glyph's bounding box to that of the
/// "global" bounding box. In the example above, if these were the only glyphs, the global
/// bounding box would be:
///
///     Rect { min: point(-5.0, -20.0), max: point(26.0, 10.0) }
///
/// and we'd interpret the `draw` calls of the four glyphs as, respectively:
///
///     743 = 02E7 comma      top-left is ( 0, -3):    (x, y) -> (x + 5, x + 17)
///     745 = 02E9 full stop  top-left is ( 1, -3):    (x, y) -> (x + 6, x + 17)
///     6   = 0006 anusvara   top-left is (-5, -20):   (x, y) -> (x + 0, y + 0)
///     510 = 01FE ddha       top-left is ( 0, -15):   (x, y) -> (x + 5, y + 5)
///
/// We could also achieve the same effect by positioning each glyph at `(5, 20)` instead of
/// `(0, 0)`: then the coordinates returned by `px_bound()`'s would change by `(5, 20)`, and
/// we could simply add the coordinates of the top-left corner:
///
///     [With position (5, 20)]
///     743 = 02E7 comma      top-left is (5, 17):   (x, y) -> (x + 5, x + 17)
///     745 = 02E9 full stop  top-left is (6, 17):   (x, y) -> (x + 6, x + 17)
///     6   = 0006 anusvara   top-left is (0,  0):   (x, y) -> (x + 0, y + 0)
///     510 = 01FE ddha       top-left is (5,  5):   (x, y) -> (x + 5, y + 5)
fn dump_glyphs(
    font_file_contents: &[u8],
    output_dir: PathBuf,
    size: f32,
    glyph_ids: Option<Vec<u16>>,
) -> Result<()> {
    use ab_glyph::{Font, FontRef, GlyphId, Point};
    use image::{DynamicImage, Rgba};

    let font =
        FontRef::try_from_slice(font_file_contents).with_context(|| "Could not parse font.")?;
    println!("This font has {} glyphs.", font.glyph_count());

    // Low-priority TODO: In principle, if `glyph_count` can be 2^16, this will be incorrect. :-)
    // Find out the idiomatic Rust way of doing this conversion.
    let glyph_ids = glyph_ids.unwrap_or((0..(font.glyph_count() as u16)).collect());

    // First pass: outline the glyphs, and find the global bounding box.
    let mut x_min = i32::MAX; // empty min = infinity.
    let mut y_min = i32::MAX;
    let mut y_max = i32::MIN; // empty max = -infinity.
    let mut glyphs = Vec::new();
    for glyph_id in &glyph_ids {
        let glyph = GlyphId(*glyph_id).with_scale_and_position(size, Point { x: 0.0, y: 0.0 });
        // `outline_glyph` can return None when bounds are invalid for whatever reason.
        if let Some(glyph) = font.outline_glyph(glyph) {
            println!(
                "Glyph {:3} = {:04X}: bounds are {:?}",
                glyph_id,
                glyph_id,
                glyph.px_bounds()
            );
            x_min = std::cmp::min(x_min, glyph.px_bounds().min.x as i32);
            y_min = std::cmp::min(y_min, glyph.px_bounds().min.y as i32);
            y_max = std::cmp::max(y_max, glyph.px_bounds().max.y as i32);
            glyphs.push((glyph_id, glyph));
        } else {
            // For the font I tried, a common reason (apparently, from opening it in FontForge)
            // is that there's simply no glyph at that position. So this is not worth logging.
            // println!("No outline for glyph {:04X}", glyph_id);
        }
    }
    println!("Global bounding box has min: ({}, {})", x_min, y_min);

    // We want a common height for all glyphs, because when glyph images are laid out
    // side-by-side, we want their baselines to align.
    // Adding an extra pixel at the bottom, for reasons I can't remember (perhaps not needed).
    let height = y_max - y_min + 1;

    // Second pass: Generate images out of the outlined glyphs.
    std::fs::create_dir_all(output_dir.clone())?;
    for (glyph_id, glyph) in glyphs {
        let width = glyph.px_bounds().max.x - (x_min as f32) + 1.0;
        let mut image = DynamicImage::new_rgba8(width as u32, height as u32).to_rgba8();
        glyph.draw(|x, y, c| {
            // Draw pixel `(x, y)` with coverage `c` (= what fraction of the pixel the glyph covered).
            // As mentioned in "Implementation notes" above, these `(x, y)` need to be reinterpreted.
            let reinterpret_x = x as i32 + glyph.px_bounds().min.x as i32 - x_min;
            let reinterpret_y = y as i32 + glyph.px_bounds().min.y as i32 - y_min;
            image.put_pixel(
                reinterpret_x as u32,
                reinterpret_y as u32,
                // Using black (#000000) as colour, and the "coverage" fraction as the PNG image's "alpha" (≈ opacity) value.
                Rgba([0, 0, 0, (c * 255.0) as u8]),
            )
        });
        let output_filename = output_dir.join(format!("glyph-{:04X}.png", glyph_id));
        image
            .save(&output_filename)
            .with_context(|| format!("Failed to write to {:?}", output_filename))?;
        println!(
            "For glyph {:3}, generated {:#?}.",
            glyph_id, output_filename
        );
    }
    Ok(())
}
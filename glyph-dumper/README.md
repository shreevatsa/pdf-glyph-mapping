# glyph-dumper

A tool for extracting glyphs from TTF font files as bitmap images.

## Overview

This tool reads a TTF (TrueType Font) file and generates a PNG image for each glyph in the font. It's particularly useful for visualizing and working with fonts, especially when you need to manually map glyphs to Unicode characters.

## Usage

```
glyph-dumper [OPTIONS] <font-file> <output-dir>
```

### Arguments

- `<font-file>`: Path to the TTF file whose glyphs are to be extracted
- `<output-dir>`: Directory in which to create the bitmap images

### Options

- `-g, --glyphs <glyphs>...`: A comma-separated list of which glyphs to dump images for (default: all glyphs)
- `-s, --size <size>`: Approximate height in pixels of the generated images [default: 30.0]
- `-h, --help`: Prints help information
- `-V, --version`: Prints version information

## Example

```bash
glyph-dumper font.ttf output_directory --size 40
```

This will extract all glyphs from `font.ttf` and save them as PNG images in `output_directory/font.ttf/`, with an approximate height of 40 pixels.

## Implementation Details

The tool uses the `ab_glyph` crate to parse TTF fonts and generate images for each glyph. It carefully handles glyph positioning to ensure proper baseline alignment when images are displayed side by side.
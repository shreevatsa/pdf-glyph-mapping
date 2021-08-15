# pdf-glyph-mapping
Some scripts to help make sense of individual glyphs in a PDF, and map them to actual text equivalents.

## What's this?

Text that requires complex text layout (because of being in Indic scripts, say) cannot always be copied correctly from PDFs. Here is a bunch of tools that may help in some cases. (Tested with only a few PDFs so far.)

Roughly, the idea is to extract font data from the PDF file, use visual or other means to associate each glyph with a meaning (roughly: equivalent Unicode sequence, plus a bit more), then use this information to convert each text run to the corresponding text. We could even post-process the PDF file (not implemented yet), to wrap each text sequence in a span of `/ActualText`.

## Example

(TODO)

(But see: [1](https://shreevatsa.github.io/pdf-glyph-mapping/tmp/font-40533.ttf.html) [2](https://shreevatsa.github.io/pdf-glyph-mapping/tmp/font-40534.ttf.html) [3](https://shreevatsa.github.io/pdf-glyph-mapping/tmp/font-40532.ttf.html) [4](https://shreevatsa.github.io/pdf-glyph-mapping/tmp/font-40531.ttf.html).)

## Usage

(Short version: Run `make` and follow instructions.)

1.  (Not part of this repository.) Prerequisites:
    1.  Make sure `mutool` is installed (and also Python and Rust).
    2.  If you know fonts that may be related to the fonts in the directory, run `ttx` (from [fonttools](https://fonttools.readthedocs.io/en/latest/ttx.html)) on them, and put the resulting files inside the `work/helper_fonts/` directory.
2.  Run `make`, from within the `work/` directory. This will do the following:
    1.  Extracts the font data from the PDF file, using `mutool extract`.
    2.  Dumps each glyph from each font as a bitmap image, using the `dump-glyphs` binary from here.
    3.  Extracts each "text operation" (`Tj`, `TJ`, `'`, `"`; see [9.4.3 Text-Showing Operators](https://www.adobe.com/content/dam/acom/en/devnet/pdf/pdfs/PDF32000_2008.pdf#page=258) in the PDF 1.7 spec) in the PDF (which glyphs from which font were used), using the `dump-tjs` binary from here.
    4.  Runs the `sample-runs.py` script from here, which
        1.  generates the glyph_id to Unicode mapping known so far (see [this comment](https://github.com/shreevatsa/pdf-glyph-mapping/blob/bbecd8154c171c97b21e76c612f2b66fdf5f873b/src/sample-runs.py#L212-L258)),
        2.  generates HTML pages with some visual information about each glyph used in the PDF (showing it in context with neighbouring glyphs etc).
3.  Create a new directory called `maps/manual/` and
    1.  copy the `toml` files under `maps/look/` into it,
    2.  (**The main manual grunt work needed**) Edit each of those TOML files, and (using the HTML files that have been generated), for each glyph that is not already mapped in the PDF itself, add the Unicode mapping for that glyph. (Any one format will do; the existing TOML entries are highly redundant but you can be concise: [see the comment](https://github.com/shreevatsa/pdf-glyph-mapping/blob/bbecd8154c171c97b21e76c612f2b66fdf5f873b/src/sample-runs.py#L253-L257).)
4.  Run `make` again. This will do the following:
    1.  Validates that the TOML files you generated are ok (it won't catch mistakes in the Unicode mapping though!), and
    2.  Generates a copy of your original PDF, with data in it about the actual text corresponding to each text operation.

All this has been tested only with one large PDF. These scripts are rather hacky and hard-code decisions about PDF structure etc; for other PDFs they will likely need to be changed.
# pdf-glyph-mapping
Some scripts to help make sense of individual glyphs in a PDF, and map them to actual text equivalents.

## What's this?

Text that requires complex text layout (because of being in Indic scripts, say) cannot always be copied correctly from PDFs. Here is a bunch of tools that may help in some cases. (Tested with only a few PDFs so far.)

Roughly, the idea is to extract font data from the PDF file, use visual or other means to associate each glyph with a meaning (roughly: equivalent Unicode sequence, plus a bit more), then use this information to convert each text run to the corresponding text. We could even post-process the PDF file, to wrap each text sequence in a span of `/ActualText`.

## Example

(TODO)

## Usage

(Short version: Run `make`.)

0.  (Not part of this repository.) Extract the font data from the PDF file, in any way. (E.g.: `mutool extract`).

1.  Run `dump-glyphs` on such a font file, to dump each glyph in it as a bitmap image.

2.  Run `dump-tjs` on the PDF file, to dump the operands of text-showing operators (`Tj`, `TJ`, `'`, `"`; see [9.4.3 Text-Showing Operators](https://www.adobe.com/content/dam/acom/en/devnet/pdf/pdfs/PDF32000_2008.pdf#page=258) in the PDF 1.7 spec), grouped by font (identified by object number in the PDF file).

3.  Run `sample-runs` on the output of the above two. This will generate a HTML file showing a few samples for each glyph, so that you can give each glyph a name.

    Save this output to a file in some format; ideally we want a rule that, given a sequence of glyph ids, gives corresponding text.

4.  (Doesn't exist yet; not started.) Run `fix-actualtext` to apply the fixes back to the PDF.

These scripts are rather hacky and hard-code decisions about filenames and PDF structure etc; they will almost certainly need to be changed.
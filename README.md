# pdf-glyph-mapping
Some scripts to help make sense of individual glyphs in a PDF, and map them to actual text equivalents.

## What's this?

Text that requires complex text layout (because of being in Indic scripts, say) cannot always be copied correctly from PDFs. Here is a bunch of tools that may help in some cases.

Roughly, the idea is to
- extract font data from the PDF file, 
- associate each glyph with its equivalent Unicode sequence (manually if necessary),
- use this information to convert each text run in the PDF to the corresponding text. (By post-processing the PDF file to wrap each text run inside  `/ActualText`.)

## Background

Some PDF files are just a collection of images (scans of pages) — we ignore those. In any other PDF file (e.g. one where you can select a run of text), the text is displayed by laying out glyphs from a font. For example, in a certain PDF that uses the font Noto Sans Devanagari, the word प्राप्त may be formed by laying out four glyphs:

![0112](https://shreevatsa.github.io/pdf-glyph-mapping/work/glyphs/font-40532.ttf/glyph-0112.png)
![0042](https://shreevatsa.github.io/pdf-glyph-mapping/work/glyphs/font-40532.ttf/glyph-0042.png)
![00CB](https://shreevatsa.github.io/pdf-glyph-mapping/work/glyphs/font-40532.ttf/glyph-00CB.png)
![0028](https://shreevatsa.github.io/pdf-glyph-mapping/work/glyphs/font-40532.ttf/glyph-0028.png)

In this font, these glyphs happen to have numerical IDs (like 0112, 0042, 00CB, 0028) that are font-specific. If we'd like to get text out of this, and the PDF does not provide it with `/ActualText`, we need to map the four glyphs to the corresponding Unicode scalar values:

- 0112 (![0112](https://shreevatsa.github.io/pdf-glyph-mapping/work/glyphs/font-40532.ttf/glyph-0112.png)) maps to 
  - 092A DEVANAGARI LETTER PA
  - 094D DEVANAGARI SIGN VIRAMA
  - 0930 DEVANAGARI LETTER RA
- 0042 (![0042](https://shreevatsa.github.io/pdf-glyph-mapping/work/glyphs/font-40532.ttf/glyph-0042.png)) maps to
  - 093E DEVANAGARI VOWEL SIGN AA
- 00CB (![00CB](https://shreevatsa.github.io/pdf-glyph-mapping/work/glyphs/font-40532.ttf/glyph-00CB.png)) maps to 
  - 092A DEVANAGARI LETTER PA
  - 094D DEVANAGARI SIGN VIRAMA
- 0028 (![0028](https://shreevatsa.github.io/pdf-glyph-mapping/work/glyphs/font-40532.ttf/glyph-0028.png)) maps to
  - 0924 DEVANAGARI LETTER TA

Sometimes, part of this mapping (like the last item above) may be included in the PDF itself (CMap), but nontrivial cases (like the first one) often aren't.

Roughly speaking, the glyph ids are laid out in visual order while Unicode text is in phonetic order. So the correspondence may be nontrivial. See the example on [page 36 here](https://itextpdf.com/sites/default/files/2018-12/PP_Advanced_typography_in_PDF-compressed.pdf#page=36); a couple more examples below:

1.  The word विकर्ण may be laid out as:

    ![0231](https://shreevatsa.github.io/pdf-glyph-mapping/work/glyphs/font-40532.ttf/glyph-0231.png)
    ![0039](https://shreevatsa.github.io/pdf-glyph-mapping/work/glyphs/font-40532.ttf/glyph-0039.png)
    ![0019](https://shreevatsa.github.io/pdf-glyph-mapping/work/glyphs/font-40532.ttf/glyph-0019.png)
    ![0027](https://shreevatsa.github.io/pdf-glyph-mapping/work/glyphs/font-40532.ttf/glyph-0027.png)
    ![00B5](https://shreevatsa.github.io/pdf-glyph-mapping/work/glyphs/font-40532.ttf/glyph-00B5.png)

    and we want this to correspond to the sequence of codepoints

    1. 0935 DEVANAGARI LETTER VA
    2. 093F DEVANAGARI VOWEL SIGN I
    3. 0915 DEVANAGARI LETTER KA
    4. 0930 DEVANAGARI LETTER RA
    5. 094D DEVANAGARI SIGN VIRAMA
    6. 0923 DEVANAGARI LETTER NNA

    (The first glyph corresponds to the second codepoint, and the last glyph corresponds to the fourth and fifth codepoints.)

2.  The word धर्मो may be laid out as:

    ![002B](https://shreevatsa.github.io/pdf-glyph-mapping/work/glyphs/font-40532.ttf/glyph-002B.png)
    ![0032](https://shreevatsa.github.io/pdf-glyph-mapping/work/glyphs/font-40532.ttf/glyph-0032.png)
    ![01C3](https://shreevatsa.github.io/pdf-glyph-mapping/work/glyphs/font-40532.ttf/glyph-01C3.png)

    and the word सर्वांग as:

    ![003C](https://shreevatsa.github.io/pdf-glyph-mapping/work/glyphs/font-40532.ttf/glyph-003C.png)
    ![0039](https://shreevatsa.github.io/pdf-glyph-mapping/work/glyphs/font-40532.ttf/glyph-0039.png)
    ![0042](https://shreevatsa.github.io/pdf-glyph-mapping/work/glyphs/font-40532.ttf/glyph-0042.png)
    ![01CB](https://shreevatsa.github.io/pdf-glyph-mapping/work/glyphs/font-40532.ttf/glyph-01CB.png)
    ![001B](https://shreevatsa.github.io/pdf-glyph-mapping/work/glyphs/font-40532.ttf/glyph-001B.png)


## Example

(TODO)

(But see: [this comment](https://github.com/shreevatsa/pdf-glyph-mapping/blob/bbecd8154c171c97b21e76c612f2b66fdf5f873b/src/sample-runs.py#L212-L258) and these files [1](https://shreevatsa.github.io/pdf-glyph-mapping/work/maps/look/font-40533-0-ASZHUB+Times-Roman.html) [2](https://shreevatsa.github.io/pdf-glyph-mapping/work/maps/look/font-40534-0-ASLUDF+Times-Bold.html) [3](https://shreevatsa.github.io/pdf-glyph-mapping/work/maps/look/font-40532-0-ATMSNB+NotoSansDevanagari.html) [4](https://shreevatsa.github.io/pdf-glyph-mapping/work/maps/look/font-40531-0-APZKLW+NotoSansDevanagari-Bold.html).)

## Usage

(Short version: Run `make` and follow instructions.)

1.  (Not part of this repository.) Prerequisites:
    1.  Make sure `mutool` is installed (and also Python and Rust).
    2.  If you know fonts that may be related to the fonts in the directory, run `ttx` (from [fonttools](https://fonttools.readthedocs.io/en/latest/ttx.html)) on them, and put the resulting files inside the `work/helper_fonts/` directory.
2.  Run `make`, from within the `work/` directory. This will do the following:
    1.  Extracts the font data from the PDF file, using `mutool extract`.
    2.  Dumps each glyph from each font as a bitmap image, using the `dump-glyphs` binary from this repository.
    3.  Extracts each "text operation" (`Tj`, `TJ`, `'`, `"`; see [9.4.3 Text-Showing Operators](https://www.adobe.com/content/dam/acom/en/devnet/pdf/pdfs/PDF32000_2008.pdf#page=258) in the PDF 1.7 spec) in the PDF (which glyphs from which font were used), using the `dump-tjs` binary from this repository.
    4.  Runs the `sample-runs.py` script from this repository, which
        1.  generates the glyph_id to Unicode mapping known so far (see [this comment](https://github.com/shreevatsa/pdf-glyph-mapping/blob/bbecd8154c171c97b21e76c612f2b66fdf5f873b/src/sample-runs.py#L212-L258)),
        2.  generates HTML pages with some visual information about each glyph used in the PDF (showing it in context with neighbouring glyphs etc) ([example](https://shreevatsa.github.io/pdf-glyph-mapping/work/maps/look/font-40532-0-ATMSNB+NotoSansDevanagari.html)).
3.  Create a new directory called `maps/manual/` and
    1.  copy the `toml` files under `maps/look/` into it,
    2.  (**The main manual grunt work needed**) Edit each of those TOML files, and (using the HTML files that have been generated), for each glyph that is not already mapped in the PDF itself, add the Unicode mapping for that glyph. (Any one format will do; the existing TOML entries are highly redundant but you can be concise: [see the comment](https://github.com/shreevatsa/pdf-glyph-mapping/blob/bbecd8154c171c97b21e76c612f2b66fdf5f873b/src/sample-runs.py#L253-L257).)
4.  Run `make` again. This will do the following:
    1.  Validates that the TOML files you generated are ok (it won't catch mistakes in the Unicode mapping though!), and
    2.  (**This is slow, may take ~150 ms per page.**) Generates a copy of your original PDF, with data in it about the actual text corresponding to each text operation.

All this has been tested only with one large PDF. These scripts are rather hacky and some decisions about PDF structure etc are hard-coded; for other PDFs they will likely need to be changed.
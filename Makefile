ORIG=../gp-mbh/unabridged
ORIG_PDF=${ORIG}.pdf
ORIG_PDF_DONE=${ORIG_PDF}.extracted
HELPER=helper_fonts/NotoSansDevanagari-Regular.ttx

all: $(ORIG).surrounded.pdf

$(ORIG_PDF):
	echo "File ${ORIG_PDF} does not exist."

# ORIG.pdf   --[mutool extract]--> font-N.ttf    (for several N)
# mutool-extract.sh simply runs `mutool extract` but in a temporary directory.
font-%.ttf: ${ORIG_PDF}
	sh mutool-extract.sh ${ORIG_PDF}

# ORIG.pdf   -----[dump-tjs]-----> Tjs-N and Tjs-N.map (maybe: font-N.tjs and font-N.toml)
Tjs-%: ${ORIG_PDF}
	RUST_BACKTRACE=1 cargo run --release --bin dump-tjs -- ${ORIG_PDF}

# font-N.ttf ---[dump-glyphs]----> font-N-glyph-M.png
glyphs/font-%: font-%.ttf
	RUST_BACKTRACE=1 cargo run --release --bin dump-glyphs -- $<

# font-N.ttf and helper-i.ttx ---[sample-runs.py]---> font-N.html (maybe also: font-N.helped.toml)
font-%.html: font-%.ttf Tjs-% glyphs/font-%
	python3 ../src/sample-runs.py $< ${HELPER}

# Manual intervention: use the .html file to edit font-N.helped.toml into font-N.manual.toml
font-%.manual.toml: font-%.helped.toml font-%.html ${ORIG_PDF}
	echo "Manually edit $< into $@ using $(word 2,$^)."

# font-N.manual.toml ---[validate-toml.py]---> font-N.final.toml
font-%.final.toml: font-%.manual.toml ${ORIG_PDF}
	python3 src/validate-toml.py $< $@

# Dummy
font-*.final.toml: ${ORIG_PDF} font-*.manual.toml
	@echo "Run something like make"; false

# ORIG.pdf and font-N.final.toml ---[dump-tjs]---> ORIG.surrounded.pdf
$(ORIG).surrounded.pdf: ${ORIG_PDF} font-*.final.toml
	RUST_BACKTRACE=1 cargo run --release --bin dump-tjs -- ${ORIG_PDF}

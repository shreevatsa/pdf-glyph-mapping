ORIG=../../gp-mbh/unabridged
# ORIG=../../page-200/out-mbh
ORIG_PDF=${ORIG}.pdf

# Steps:
# Graph from https://gist.github.com/shreevatsa/9a07d68931cd2167e12db4008eef5715 using https://dot-to-ascii.ggerganov.com/
#                         ┌─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
#                         │                                                                                                                                                                                                                                     ▼
# ┏━━━━━━━━━━━━━━━┓     ┏━━━━━━━━━━┓     ┌−−−−−−−−−−−−−−−−−−−┐     ┏━━━━━━━━┓     ┌−−−−−−−−−−−−−┐     ┏━━━━━━━━━┓     ┌−−−−−−−−−−−−−−−−┐     ┏━━━━━━━━━━━━┓     ┌−−−−−−−−−−−−−−−−−−−−−┐     ┏━━━━━━━━━━━━━━┓     ┌−−−−−−−−−−−−−−−−−−┐     ┏━━━━━━━━━━━━━┓     ┌−−−−−┐     ┏━━━━━━━━━━━━━━━━┓
# ┃ helper_fonts/ ┃     ┃ ORIG.pdf ┃ ──▶ ╎ mutool-extract.sh ╎ ──▶ ┃ fonts/ ┃ ──▶ ╎ dump-glyphs ╎ ──▶ ┃ glyphs/ ┃ ──▶ ╎                ╎ ──▶ ┃ maps/look/ ┃ ──▶ ╎ manual intervention ╎ ──▶ ┃ maps/manual/ ┃ ──▶ ╎ validate-maps.py ╎ ──▶ ┃ maps/valid/ ┃ ──▶ ╎ fix ╎ ──▶ ┃ ORIG.fixed.pdf ┃
# ┗━━━━━━━━━━━━━━━┛     ┗━━━━━━━━━━┛     └−−−−−−−−−−−−−−−−−−−┘     ┗━━━━━━━━┛     └−−−−−−−−−−−−−┘     ┗━━━━━━━━━┛     ╎                ╎     ┗━━━━━━━━━━━━┛     └−−−−−−−−−−−−−−−−−−−−−┘     ┗━━━━━━━━━━━━━━┛     └−−−−−−−−−−−−−−−−−−┘     ┗━━━━━━━━━━━━━┛     └−−−−−┘     ┗━━━━━━━━━━━━━━━━┛
#   │                     │                                          │                                                ╎                ╎
#   │                     │                                          └──────────────────────────────────────────────▶ ╎ sample-runs.py ╎
#   │                     ▼                                                                                           ╎                ╎
#   │                   ┌−−−−−−−−−−┐     ┏━━━━━━━━━━━━━━━━━━━┓                                                        ╎                ╎
#   │                   ╎ dump-Tj  ╎ ──▶ ┃    font-usage/    ┃ ─────────────────────────────────────────────────────▶ ╎                ╎
#   │                   └−−−−−−−−−−┘     ┗━━━━━━━━━━━━━━━━━━━┛                                                        └−−−−−−−−−−−−−−−−┘
#   │                                                                                                                   ▲
#   └───────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
#
# Legend:
#                       ┏━━━━━━━━━━┓
#                       ┃   ...    ┃             files
#                       ┗━━━━━━━━━━┛
#                       ┌−−−−−−−−−−┐
#                       ╎   ...    ╎            program
#                       └−−−−−−−−−−┘

all: $(ORIG).fixed.pdf

$(ORIG_PDF):
	@echo "File ${ORIG_PDF} does not exist."; false

# ORIG.pdf  ---[mutool-extract.sh]--->  fonts/font-N.ttf    (for several N)
# mutool-extract.sh simply runs `mutool extract` but in a temporary directory.
fonts/: ${ORIG_PDF}
	sh mutool-extract.sh ${ORIG_PDF}

# ORIG.pdf  ---[dump-tjs]--->  font-usage/font-N.{Tjs,toml}
font-usage/: ${ORIG_PDF}
	RUST_BACKTRACE=1 cargo +nightly run --release --bin dump-tjs -- ${ORIG_PDF} font-usage/ --phase phase1

# fonts/font-N.ttf  ---[dump-glyphs]--->  glyphs/font-N/glyph-M.png
glyphs/: fonts/
	for f in fonts/font-*.ttf; do \
		RUST_BACKTRACE=1 cargo +nightly run --release --bin dump-glyphs -- $$f glyphs/; \
	done

# font-usage/font-N.{Tjs,toml} and helper-i.ttx ---[sample-runs.py]---> maps/look/font-N.{html,toml}
maps/look/: font-usage/ glyphs/ helper_fonts/
	python3 ../src/sample-runs.py font-usage/ glyphs/ helper_fonts/ maps/look/
	# Replace `open` with `xdg-open` on Linux, maybe?
	for f in maps/look/*.html; do open $$f; done

# Manual intervention: use the .html file to edit maps/look/font-N.toml into maps/manual/font-N.toml
# If you already have maps/valid/, then just run "touch maps/valid/" instead, to skip this step.
maps/manual/: maps/look/
	@echo "Manually edit maps/look/font-*.toml into corresponding maps/manual/font-*.toml using the HTML files."; false

# maps/manual/font-N.toml ---[validate-toml.py]---> maps/valid/font-N.toml
maps/valid/: maps/manual/ maps/look/
	mkdir maps/valid/
	python3 ../src/validate_maps.py maps/manual/ maps/valid/

# ORIG.pdf and maps/valid/font-N.toml ---[dump-tjs]---> ORIG.fixed.pdf
$(ORIG).fixed.pdf: ${ORIG_PDF} maps/valid/
	RUST_BACKTRACE=1 cargo +nightly run --release --bin dump-tjs -- ${ORIG_PDF} maps/valid/ ${ORIG}.fixed.pdf --phase phase2

clean:
	rm -vrf glyphs/
	rm -vri font-usage/ fonts/ maps/ || true

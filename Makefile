ORIG=../gp-mbh/unabridged
ORIG_PDF=${ORIG}.pdf
HELPER=helper_fonts/NotoSansDevanagari-Regular.ttx

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

# ORIG.pdf  ---[dump-tjs]--->  font-usage/font-N.{Tjs,toml} (but for now, Tjs-N and Tjs-N.map)
font-usage/: ${ORIG_PDF}
	RUST_BACKTRACE=1 cargo run --release --bin dump-tjs -- ${ORIG_PDF}

# fonts/font-N.ttf  ---[dump-glyphs]--->  glyphs/font-N/glyph-M.png
glyphs/font-%/: fonts/font-%.ttf fonts/
	RUST_BACKTRACE=1 cargo run --release --bin dump-glyphs -- $<

# font-usage/font-N.{Tjs,toml} and helper-i.ttx ---[sample-runs.py]---> maps/look/font-N.{html,toml}
maps/look/: font-usage/ helper_fonts/ glyphs/
	python3 ../src/sample-runs.py
	# Replace `open` with `xdg-open` on Linux, maybe?
	for f in maps/look/*.html; do open $$f; done

# Manual intervention: use the .html file to edit maps/look/font-N.toml into maps/manual/font-N.toml
maps/manual/: maps/look/
	echo "Manually edit maps/look/font-*.toml into corresponding maps/manual/font-*.toml using the HTML files."; false

# maps/manual/font-N.toml ---[validate-toml.py]---> maps/valid/font-N.toml
maps/valid/: maps/manual/ maps/look/
	python3 src/validate-toml.py maps/manual/ maps/valid/

# ORIG.pdf and maps/valid/font-N.toml ---[dump-tjs]---> ORIG.fixed.pdf
$(ORIG).fixed.pdf: ${ORIG_PDF} maps/valid/
	RUST_BACKTRACE=1 cargo run --release --bin dump-tjs -- ${ORIG_PDF}

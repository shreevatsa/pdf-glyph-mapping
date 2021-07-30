set -euxo pipefail

# ORIG=../page-200/out-mbh-mutool.pdf
ORIG=../gp-mbh/unabridged.pdf

tmpdir=tmp-$(date +%s)
echo "Creating ${tmpdir}"
mkdir -p ${tmpdir}
cd ${tmpdir}
mutool extract ../${ORIG}
rm image-*.{png,jpg} || true
RUST_BACKTRACE=1 cargo run --release --bin dump-tjs -- ../${ORIG}
for f in *.ttf; do
    echo $f
    RUST_BACKTRACE=1 cargo run --release --bin dump-glyphs -- $f
    # Yes this takes the font filename and assumes stuff about the Tjs filename etc. Fix later.
    python3 ../src/sample-runs.py $f
    open $f.html # Or xdg-open on Linux, I guess.
done
cd -

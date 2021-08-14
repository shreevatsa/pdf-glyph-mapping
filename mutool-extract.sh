set -euxo pipefail

tmpdir=tmp-$(date +%s)
mkdir -p ${tmpdir}
cd ${tmpdir}
mutool extract ../$1
rm image-*.{png,jpg} || true
mv -i *.ttf ..
cd -
rmdir ${tmpdir}
mkdir -p fonts/
mv -iv font-*.ttf fonts/

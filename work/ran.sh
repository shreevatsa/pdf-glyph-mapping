set -euxo pipefail

# for i in 1 2 3 4 5 6; do
#     echo $i
#     mutool clean ../../gp-mbh/Mahabharata\ Volume\ $i.pdf Mbh-Vol-$i-clean.pdf
#     qpdf --qdf --no-original-object-ids Mbh-Vol-$i-clean.pdf Mbh-Vol-$i-clean-qdf.pdf
#     ORIG=Mbh-Vol-$i-clean-qdf
#     RUST_BACKTRACE=1 cargo +nightly run --release --bin dump-tjs -- ${ORIG}.pdf maps/valid/ ${ORIG}.fixed.pdf --phase phase2
#     pdftotext ${ORIG}.fixed.pdf
# done

for i in 1 2 3 4 5 6; do
    ORIG=Mbh-Vol-$i-clean-qdf
    cp ${ORIG}.fixed.txt replaced-${ORIG}.fixed.txt
    # brew installed GNU sed as "gsed"; change this to "sed" otherwise.
    gsed -i -E "s/(([क-ह]्)*[क-ह][^क-ह]*)र्<CCprec>/र्\1/g" replaced-${ORIG}.fixed.txt
    gsed -i -E "s/ि<CCsucc>(([क-ह]्)*[क-ह])/\1ि/g" replaced-${ORIG}.fixed.txt
done

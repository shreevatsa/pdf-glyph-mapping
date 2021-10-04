for SAN in address leak memory thread; do
    export RUSTFLAGS=-Zsanitizer=$SAN RUSTDOCFLAGS=-Zsanitizer=$SAN
    RUST_BACKTRACE=full cargo +nightly run -Zbuild-std --target x86_64-apple-darwin --bin dump-tjs -- ../../gp-mbh/unabridged.pdf font-usage --phase phase1
done

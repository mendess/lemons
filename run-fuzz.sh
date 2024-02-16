#!/bin/bash

case "$1" in
    fuzz)
        cargo +nightly fuzz run fuzz_target_1
        ;;
    coverage)
        timeout 1m cargo +nightly fuzz run fuzz_target_1
        cargo +nightly fuzz coverage fuzz_target_1
        cargo +nightly cov -- \
            show  \
            ./target/x86_64-unknown-linux-gnu/coverage/x86_64-unknown-linux-gnu/release/fuzz_target_1 \
            --format=html \
            -instr-profile=./fuzz/coverage/fuzz_target_1/coverage.profdata \
            > index.html
            xdg-open index.html
        ;;
esac

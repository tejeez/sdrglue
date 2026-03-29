#!/bin/sh
set -e
cd ../sdrglue
cargo build --release

mkdir -p test_results

target/release/sdrglue \
    --sdr-rx-freq 434e6 \
    --sdr-tx-freq 434e6 \
    --record-iq test_results/received_test_pulses.cf32 32e3 434.1e6 \
    --tx-test-pulse 32e3 434.1e6 1000 \


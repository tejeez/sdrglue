#!/bin/sh
set -e
cd ../sdrglue
cargo build --release

mkdir -p test_results

taskset -c 1 \
target/release/sdrglue \
    --sdr-device "driver=lime" \
    --sdr-rx-freq 434e6 \
    --sdr-rx-fs 512e3 \
    --sdr-rx-ant LB2 \
    --sdr-tx-freq 434e6 \
    --sdr-tx-fs 512e3 \
    --sdr-tx-ant BAND1 \
    --record-iq test_results/received_test_pulses.cf32 32e3 434.1e6 \
    --tx-test-pulse 32e3 434.1e6 1000 \


#!/bin/sh
set -e
cd ../sdrglue
cargo build --release

mkdir -p test_results

DATE="$(date +%s)"

# A quick test interfacing with tetra-bluestation,
# tune RX on DL and see if it receives its own transmission.
# Also log the received signal to check if it makes transmissions in time.
# Log test pulses, too, to figure out where deadline misses are happening.
target/release/sdrglue \
    --sdr-rx-freq 434e6 \
    --sdr-tx-freq 434e6 \
    --record-iq "test_results/received_bluestation_${DATE}.cf32" 32e3 434.1e6 \
    --rx-to-dgram /tmp/bluestation-rx-socket 434.1e6 IQ \
    --tx-from-dgram /tmp/bluestation-tx-socket 434.1e6 IQ \
    --record-iq "test_results/received_test_pulses_${DATE}.cf32" 32e3 434.05e6 \
    --tx-test-pulse 32e3 434.05e6 100 \


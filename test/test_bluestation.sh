#!/bin/sh
set -e
cd ../sdrglue
cargo build --release

# A quick test interfacing with tetra-bluestation:
# tune RX on DL and see if it receives its own transmission.
# For real use, modify this for the correct RX and TX carrier frequencies.
chrt -f 80 target/release/sdrglue \
    --sdr-rx-freq 438e6 \
    --sdr-tx-freq 438.025e6 \
    --rx-to-dgram /tmp/bluestation-rx-socket 438.025e6 IQ \
    --tx-from-dgram /tmp/bluestation-tx-socket 438.025e6 IQ \

#!/bin/sh
set -e
cd ../sdrglue
cargo build --release

# Set SDR RX freq in the guard band between two uplink channels
# so DC peak does not end up in any of them.
chrt -f 80 target/release/sdrglue \
    --sdr-rx-freq 433.0625e6 \
    --sdr-tx-freq 438.0625e6 \
    --sdr-tx-gain MIXER 10 \
    --rx-to-dgram /tmp/bluestation1-rx-socket 433.025e6 IQ \
    --rx-to-dgram /tmp/bluestation2-rx-socket 433.050e6 IQ \
    --rx-to-dgram /tmp/bluestation3-rx-socket 433.075e6 IQ \
    --rx-to-dgram /tmp/bluestation4-rx-socket 433.100e6 IQ \
    --tx-from-dgram /tmp/bluestation1-tx-socket 438.025e6 IQ \
    --tx-from-dgram /tmp/bluestation2-tx-socket 438.050e6 IQ \
    --tx-from-dgram /tmp/bluestation3-tx-socket 438.075e6 IQ \
    --tx-from-dgram /tmp/bluestation4-tx-socket 438.100e6 IQ \

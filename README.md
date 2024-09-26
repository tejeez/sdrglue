# Introduction

Many digital mode decoders, such as
[direwolf](https://github.com/wb2osz/direwolf) or
[horus-gui](https://github.com/projecthorus/horus-gui),
work with demodulated audio from a traditional FM or SSB receiver,
and thus cannot be directly used with software defined radios.
They are sometimes used with rtl_fm
(which provides demodulated audio through a pipe) or with Gqrx
(which can provide demodulated audio through an UDP socket).
These programs, however, are limited to only demodulating
one channel at a time, and do not support transmitting.
Sdrglue aims to fix these limitations, allowing "gluing" multiple
digital mode decoder or packet radio transceiver programs simultaneously
together with a single SDR device.

Sdrglue supports sending FM demodulated audio output through an
UDP socket in a format compatible with Gqrx.
Yes, I know, an UDP socket is not an ideal interface for the purpose,
since it may randomly drop or reorder packets,
both of which might prevent decoding digital modes.
It seems to be, however, a common practice, is already supported
by existing applications, and seems to work reliably enough
when UDP packets are sent to localhost.

Support for other modulations and interfaces might be added
if there is some known usecase for them.

Right now Sdrglue is work-in-progress and is not really usable yet.

# Installing

Sdrglue should work on most common operating systems (including Windows)
but has been so far tested only on Ubuntu and Raspberry Pi OS.
Instruction for installing on these:

## Install Rust compiler and other dependencies

```
sudo apt-get install -y --no-install-recommends libclang-dev libsoapysdr-dev
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Choose "Proceed with standard installation (default - just press enter)"

## Compile sdrglue

```
cd
git clone "https://github.com/tejeez/sdrglue.git"
cd sdrglue/sdrglue
git submodule update --init
. "$HOME/.cargo/env"
cargo build --release
```

## Run sdrglue

```
~/sdrglue/sdrglue/target/release/sdrglue
```

## Install Rust compiler

```
sudo apt-get install -y --no-install-recommends libclang-dev
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Choose "Proceed with standard installation (default - just press enter)"

## Compile sdrglue

```
cd
git clone "https://github.com/tejeez/sdrglue.git"
cd sdrglue/sdrglue
. "$HOME/.cargo/env"
cargo build --release
```

## Run sdrglue

```
~/sdrglue/sdrglue/target/release/sdrglue
```

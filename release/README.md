# Release Build Script (xtask pattern)
This xtask defines a cargo alias `release` [here](/.cargo/config.toml).

This is a cross-platform way to build this project with optimizations that doesn't rely on shell scripts.


## Usage
```bash
cargo release --help

cargo release --wasm

cargo release --target x86_64-unknown-linux-gnu
```

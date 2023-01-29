#!/bin/zsh
cargo clean
cargo build --target wasm32-unknown-unknown --release
cp ../target/wasm32-unknown-unknown/release/non_fungible_token.wasm ../res/
rm -rf neardev
near dev-deploy ../res/non_fungible_token.wasm
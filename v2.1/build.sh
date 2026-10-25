#!/bin/bash
# Build script for Flowsta Private DNA v2.1 - the Holochain 0.7 generation of
# v2.0 (same single Sealed entry type and link; validation on the 0.7 action
# model; the piece lane's capability grant in the 0.7 shape). New DNA hash,
# so a new per-user network; the Vault installs it with the per-user seed.
set -e
echo "Building Flowsta Private DNA v2.1"
if ! command -v ${HC:-hc-0.7} &> /dev/null; then
    echo "Error: hc 0.7.1 not found (cargo install holochain_cli --version 0.7.1)"; exit 1
fi
mkdir -p workdir
RUSTFLAGS='--cfg getrandom_backend="custom"' CARGO_TARGET_DIR=target \
    cargo build --release --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/release/private_data_integrity.wasm workdir/
cp target/wasm32-unknown-unknown/release/private_data_coordinator.wasm workdir/
cp dna.yaml happ.yaml workdir/
${HC:-hc-0.7} dna pack workdir
${HC:-hc-0.7} app pack workdir
echo "Build complete!"
sha256sum workdir/flowsta_private_v2_1_happ.happ workdir/flowsta_private_v2_1.dna

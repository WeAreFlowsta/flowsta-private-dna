#!/bin/bash

set -e

echo "Building private DNA v1.10 (Two-Factor Authentication: TotpConfig stored in Holochain)..."

# Create workdir if it doesn't exist
mkdir -p workdir/dnas

# Build integrity zome
echo "Building private_data integrity zome..."
cd zomes/private_data/integrity
RUSTFLAGS='--cfg getrandom_backend="custom"' cargo build --release --target wasm32-unknown-unknown
cd ../../..

# Build coordinator zome
echo "Building private_data coordinator zome..."
cd zomes/private_data/coordinator
RUSTFLAGS='--cfg getrandom_backend="custom"' cargo build --release --target wasm32-unknown-unknown
cd ../../..

# Copy WASM files to workdir
echo "Copying WASM files..."
cp target/wasm32-unknown-unknown/release/private_data_integrity.wasm workdir/
cp target/wasm32-unknown-unknown/release/private_data_coordinator.wasm workdir/

# Copy config files to workdir
cp dna.yaml workdir/
cp happ.yaml workdir/

# Pack DNA
echo "Packing DNA..."
hc dna pack workdir

# Copy DNA to dnas subdirectory for hApp packing
cp workdir/flowsta_private_v1_10.dna workdir/dnas/

# Pack hApp
echo "Packing hApp..."
hc app pack workdir

echo ""
echo "✅ Build complete (v1.10)!"
echo "DNA bundle: workdir/flowsta_private_v1_10.dna"
echo "hApp bundle: workdir/flowsta_private_v1_10_happ.happ"
echo ""
echo "File sizes:"
ls -lh workdir/*.{dna,happ}

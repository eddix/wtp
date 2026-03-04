#!/bin/bash
# Install wtp to ~/.local/bin

set -e

# Build release binary
echo "Building wtp..."
cargo build --release

# Ensure ~/.local/bin exists
mkdir -p ~/.local/bin

# Install binary
echo "Installing wtp to ~/.local/bin/"
cp target/release/wtp ~/.local/bin/wtp

echo "wtp installed successfully!"
echo "Make sure ~/.local/bin is in your PATH."

#!/usr/bin/env bash
set -euo pipefail

# Install mdbook and preprocessor plugins
cargo install mdbook@0.4.52
cargo install mdbook-katex@0.9.4
cargo install mdbook-mermaid@0.14.0
cargo install mdbook-admonish@1.20.0

# Build the book
cd "$(dirname "$0")"
mkdir -p out
mdbook build -d out
echo "Book rendered to $(pwd)/out/"

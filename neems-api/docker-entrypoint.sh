#!/bin/bash
set -e

# Run TypeScript generation in the background, watching for Rust file changes
cargo watch \
  --features test-staging \
  -w neems-api/src \
  -w neems-data/src \
  -s 'cargo test --features test-staging generate_typescript_types --quiet' &

# Run the main API server with live reload
exec cargo watch \
  -w neems-api \
  -w neems-data \
  -w crates \
  -x 'run --bin neems-api'

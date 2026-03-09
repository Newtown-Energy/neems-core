#!/bin/bash
set -e

# Run database migrations
echo "Running database migrations..."
cd /usr/src/app/neems-api
diesel --database-url="$DATABASE_URL" migration run
cd /usr/src/app

# Build neems-admin if not already built (needed for demo data setup)
if [ ! -f /usr/src/app/target/debug/neems-admin ]; then
  echo "Building neems-admin..."
  cargo build --bin neems-admin
fi

# Run demo data setup script (idempotent - safe to run multiple times)
echo "Setting up demo data..."
export NEEMS_ADMIN_BIN=/usr/src/app/target/debug/neems-admin
/usr/src/app/bin/setup-demo-data || echo "Demo data setup failed or already complete"

# Generate TypeScript types synchronously on startup (before neems-react starts)
echo "Generating TypeScript types (initial)..."
cargo test --features test-staging generate_typescript_types --quiet || true
/usr/src/app/bin/build-local-types-package.sh "$NEEMS_TS_OUTPUT_DIR"

# Run TypeScript generation in the background, watching for Rust file changes
cargo watch \
  --features test-staging \
  -w neems-api/src \
  -w neems-data/src \
  -s 'cargo test --features test-staging generate_typescript_types --quiet && /usr/src/app/bin/build-local-types-package.sh "${NEEMS_TS_OUTPUT_DIR}"' &

# Run the main API server with live reload
exec cargo watch \
  -w neems-api \
  -w neems-data \
  -w crates \
  -x 'run --bin neems-api'

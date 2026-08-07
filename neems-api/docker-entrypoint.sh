#!/bin/bash
set -e

# Debug info is capped to line tables in [profile.dev] in the workspace
# Cargo.toml, not here. It used to be exported as RUSTFLAGS from this script,
# which meant any cargo run another way — `docker compose run ... cargo test`,
# say — had a different fingerprint and built a second full set of artifacts
# into the same target volume. See issue #94.

# --- Keep the shared target volume under a hard ceiling -----------------------
#
# Cargo never garbage-collects target/: every rebuild writes new hash-suffixed
# artifacts and the old ones stay forever. With two cargo watch processes below,
# ~20 integration test binaries that each statically link the whole workspace,
# and three services sharing this volume, that reached 173 GB and filled the
# Docker VM's disk (issue #94).
#
# A size ceiling rather than an age cutoff, because only a ceiling actually
# answers "this will never take more than N". cargo-sweep removes oldest-first
# until the target directory is under CARGO_TARGET_MAXSIZE.
#
# For reference: one clean build of this workspace — every bin and every test
# binary, single fingerprint, no incremental — measures about 5.7 GB. The
# default leaves room for that plus normal churn. Setting it below a full build
# would just sweep away artifacts that are immediately rebuilt.
CARGO_TARGET_MAXSIZE="${CARGO_TARGET_MAXSIZE:-10GB}"

# How often to re-check. Sweeping only at startup is not enough: with
# `restart: unless-stopped` a container can run for weeks between starts, which
# is exactly how the volume got to 173 GB in the first place.
CARGO_TARGET_SWEEP_INTERVAL="${CARGO_TARGET_SWEEP_INTERVAL:-21600}" # 6 hours

# target/debug/incremental gets its own budget because cargo-sweep will not
# touch it — verified with `--dry-run`, which reports zero paths there — while
# still counting it toward the total. Left alone, a 6 GB incremental directory
# under a 10 GB ceiling forces cargo-sweep to delete nearly every artifact it
# *can* reach to compensate, which is a full rebuild for nothing.
#
# So bound it first and separately. It is a pure cache; cargo rebuilds it, and
# the cost of dropping it is one slower compile.
CARGO_INCREMENTAL_MAXSIZE_MB="${CARGO_INCREMENTAL_MAXSIZE_MB:-3072}" # 3 GB

sweep_target() {
  local incremental=/usr/src/app/target/debug/incremental

  if [ -d "$incremental" ]; then
    local size_mb
    size_mb=$(du -sm "$incremental" 2>/dev/null | cut -f1)
    if [ -n "$size_mb" ] && [ "$size_mb" -gt "$CARGO_INCREMENTAL_MAXSIZE_MB" ]; then
      echo "Incremental cache is ${size_mb}MB (limit ${CARGO_INCREMENTAL_MAXSIZE_MB}MB), clearing it"
      rm -rf "${incremental:?}"/* 2>/dev/null || true
    fi
  fi

  if command -v cargo-sweep >/dev/null 2>&1; then
    cargo sweep --maxsize "$CARGO_TARGET_MAXSIZE" /usr/src/app \
      || echo "cargo sweep failed, continuing"
  fi

  # Test database copies from runs that died before their own sweep could run.
  rm -f /usr/src/app/target/test_db_*.db 2>/dev/null || true
}

echo "Sweeping build artifacts down to ${CARGO_TARGET_MAXSIZE}..."
sweep_target

# Keep sweeping for the life of the container. Backgrounded before the `exec`
# below, so it survives as its own process. Failure is never worth stopping the
# container over, hence the guards above.
(
  while sleep "$CARGO_TARGET_SWEEP_INTERVAL"; do
    sweep_target
  done
) &

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

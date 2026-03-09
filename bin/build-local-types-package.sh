#!/bin/bash
#
# Build a local @newtown-energy/types package from generated .ts files.
# Called after each type generation cycle so neems-react can resolve
# the package via a Docker bind mount.
#
# Usage: build-local-types-package.sh <output-dir>

set -e

OUTPUT_DIR="${1:?Usage: build-local-types-package.sh <output-dir>}"

# Check if any .ts files exist (excluding index.ts which we create)
shopt -s nullglob
ts_files=("$OUTPUT_DIR"/*.ts)
shopt -u nullglob

# Filter out index.ts from the list
source_files=()
for f in "${ts_files[@]}"; do
  if [ "$(basename "$f")" != "index.ts" ]; then
    source_files+=("$f")
  fi
done

if [ ${#source_files[@]} -eq 0 ]; then
  echo "No .ts files found in $OUTPUT_DIR — skipping local types package build"
  exit 0
fi

# Generate barrel index.ts
echo "// Auto-generated barrel file — do not edit" > "$OUTPUT_DIR/index.ts"
for f in "${source_files[@]}"; do
  basename="$(basename "$f" .ts)"
  echo "export * from './${basename}';" >> "$OUTPUT_DIR/index.ts"
done

# Generate package.json
cat > "$OUTPUT_DIR/package.json" << 'PKGJSON'
{
  "name": "@newtown-energy/types",
  "version": "0.0.0-local",
  "description": "Locally-built types from neems-core (auto-generated)",
  "types": "./index.ts",
  "exports": {
    ".": "./index.ts"
  }
}
PKGJSON

echo "Local types package built in $OUTPUT_DIR (${#source_files[@]} type files)"

#!/usr/bin/env bash
# Regenerates openapi.json from the Rust API and the TypeScript types the frontend uses.
# CI runs this and fails when the result differs from what is committed.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo run --quiet -- --openapi > openapi.json
(cd web && npm run --silent gen:api)

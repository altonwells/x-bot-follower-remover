#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
cargo build --release --locked
(cd extension && npm run build)
python3 scripts/package.py

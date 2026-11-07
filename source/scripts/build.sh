#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
command -v cargo >/dev/null || { echo 'Install Rust 1.75 or newer on the build machine.' >&2; exit 1; }
cargo fmt --all -- --check
cargo test --locked --offline -- --test-threads=1
cargo clippy --locked --offline --all-targets -- -D warnings
cmake -S . -B build -G Ninja -DCMAKE_BUILD_TYPE=Release
cmake --build build --parallel 4
target/release/wow-release verify-source "$PWD"
target/release/wow-release manifest build/wow-steamos > build/native-build.json
QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software build/wow-steamos --self-check
echo 'Built build/wow-steamos. Read packaging/README.md for the portable bundle.'

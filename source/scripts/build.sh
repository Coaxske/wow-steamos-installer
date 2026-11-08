#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
command -v cargo >/dev/null || { echo 'Install Rust 1.75 or newer on the build machine.' >&2; exit 1; }
build_mode=public
tools_option=OFF
feature_args=(--no-default-features)
if [[ "${1:-}" == "--developer-tools" ]]; then
    build_mode=developer
    tools_option=ON
    feature_args+=(--features developer-tools)
elif [[ -n "${1:-}" ]]; then
    echo 'Usage: scripts/build.sh [--developer-tools]' >&2
    exit 2
fi
cargo fmt --all -- --check
cargo test --locked --offline --target-dir "target/$build_mode" "${feature_args[@]}" -- --test-threads=1
cargo clippy --locked --offline --target-dir "target/$build_mode" "${feature_args[@]}" --all-targets -- -D warnings
cmake -S . -B "build-$build_mode" -G Ninja -DCMAKE_BUILD_TYPE=Release -DWOW_ENABLE_DEVELOPER_TOOLS="$tools_option"
cmake --build "build-$build_mode" --parallel 4
"target/$build_mode/release/wow-release" verify-source "$PWD"
"target/$build_mode/release/wow-release" manifest "build-$build_mode/wow-steamos" > "build-$build_mode/native-build.json"
QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software "build-$build_mode/wow-steamos" --self-check
echo "Built build-$build_mode/wow-steamos. Package with --build-mode $build_mode; see packaging/README.md."

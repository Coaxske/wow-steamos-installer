#!/usr/bin/env bash
# Run only in the disposable project build container, never on the host.
set -euo pipefail
[ -f /run/.containerenv ] || { echo 'Requires disposable container' >&2; exit 1; }
cd -- "$(dirname -- "${BASH_SOURCE[0]}")"
printf '%s  %s\n' 36e2e41557e0fa4a1519315c0f5958a87ccb27e25c51776beb6f1239526447b0 SDL2-2.30.0.tar.gz | sha256sum -c -
work=$(mktemp -d)
tar -xzf SDL2-2.30.0.tar.gz -C "$work"
cmake -S "$work/SDL2-2.30.0" -B "$work/build" -G Ninja -DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX=/usr -DCMAKE_INSTALL_LIBDIR=lib/x86_64-linux-gnu -DSDL_SHARED=ON -DSDL_STATIC=OFF -DSDL_TEST=OFF
cmake --build "$work/build" --parallel 2
DESTDIR="${1:?Supply a fresh staging root}" cmake --install "$work/build"
mkdir -p "$1/usr/share/doc/libsdl2-upstream-2.30.0"
cp "$work/SDL2-2.30.0/LICENSE.txt" "$1/usr/share/doc/libsdl2-upstream-2.30.0/copyright"

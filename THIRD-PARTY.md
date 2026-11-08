# Dependency notices and source

The installer is provided under the included MIT license. World of Warcraft and Battle.net are Blizzard trademarks. This project is not affiliated with Blizzard Entertainment or Valve.

## Current build

Candidate 48 uses Rust 1.75, Debian 12 build inputs, Qt 6.4.2 and upstream SDL 2.30.0. Qt is dynamically linked. The public build includes normal Qt and SDL controls. Separate private developer builds add QtTest, direct QtDBus, GStreamer capture and Tesseract OCR components.

LICENSE-INVENTORY.json in the packaged runtime maps the actual bundled libraries to their notices. Required transitive QtDBus libraries can remain in public mode without providing capture or external input tools. Host graphics drivers and glibc remain system components. Developer capture additionally uses host PipeWire and desktop portal services.

All 19 pinned Rust registry packages have verified archive checksums. Six earlier distribution-modified copies were replaced with exact-version originals: cfg-if 1.0.0, cpufeatures 0.2.11, serde_json 1.0.111, sha2 0.10.8, syn 2.0.48 and typenum 1.16.0. Their full source and license texts remain in vendor/. No package version or enabled product feature was changed for that correction.

## Licenses and corresponding source

Qt and its modules use their applicable LGPL terms. The runtime includes distribution copyright notices and applicable LGPL, GPL and other license texts. Release source archives provide the corresponding distribution sources and patches for the actual runtime closure, plus upstream SDL source. Keep these notices and source available when redistributing.

Rust dependencies and their license texts are in vendor/: serde, serde_derive, serde_json, sha2, digest, crypto-common, block-buffer, libc, generic-array, typenum, cfg-if, cpufeatures, itoa, ryu, proc-macro2, quote, syn, unicode-ident and version_check.

Upstream references:
- Qt: https://download.qt.io/archive/qt/6.4/6.4.2/submodules/
- SDL: https://www.libsdl.org/release/SDL2-2.30.0.tar.gz
- Debian source packages: https://sources.debian.org/

## Replacing LGPL libraries

The portable executable can extract its runtime into a new cache folder:

    ./WoW-SteamOS-Rust-Linux-x86_64 --extract-runtime ~/.cache/wow-runtime-new
    LD_LIBRARY_PATH=~/.cache/wow-runtime-new/lib QT_PLUGIN_PATH=~/.cache/wow-runtime-new/plugins QML2_IMPORT_PATH=~/.cache/wow-runtime-new/qml ~/.cache/wow-runtime-new/bin/wow-steamos

Use compatible modified libraries in that extracted folder. The destination must not already exist and must be on the cache filesystem. Reverse engineering for debugging modifications to LGPL components is permitted as required by their licenses. This project imposes no restriction on that use.

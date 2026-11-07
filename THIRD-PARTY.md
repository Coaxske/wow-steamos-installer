# Current C38 runtime notice

This working package uses a Debian12/Bookworm toolchain with Qt6.4.2. The portable bundles upstream SDL2.30.0 (confirmed by SDL_GetVersion), with licenses/libsdl2-upstream-2.30.0.txt; the build image separately has distribution SDL2.26.5 installed. Do not confuse that installed toolchain package with the shipped runtime library. Older Ubuntu descriptions below are historical. The bundled runtime/LICENSE-INVENTORY.json maps each included dynamic component to its shipped distribution notice. The source and all applicable license notices accompany the portable packaging; preserve them when distributing. No new third-party component was introduced by launcher-choice integration.

## Earlier notices and development references

# Dependency notices

This new application's original code is provided under the included MIT license.
No old installer implementation is bundled. World of Warcraft and Battle.net
are Blizzard trademarks; this project is not affiliated with Blizzard or Valve.

The testing binary dynamically loads Qt 6.4.2 and SDL 2.30.0. Qt libraries and
QML modules are used under their applicable LGPL terms; copyright notices and
LGPL/GPL license texts accompany the runtime and release. Qt is not statically
linked into this application. The native wrapper embeds a redistributable
runtime archive and extracts it before launching the dynamically linked app.

For inspection or replacement of LGPL libraries, run:

```sh
./WoW-SteamOS-Rust-Linux-x86_64 --extract-runtime ~/.cache/wow-runtime-new
LD_LIBRARY_PATH=~/.cache/wow-runtime-new/lib \
QT_PLUGIN_PATH=~/.cache/wow-runtime-new/plugins \
QML2_IMPORT_PATH=~/.cache/wow-runtime-new/qml \
~/.cache/wow-runtime-new/bin/wow-steamos
```

The destination must not already exist and must be on the cache filesystem.
Replace libraries with compatible modified versions in that extracted tree.
Reverse engineering for debugging modifications to LGPL components is permitted
as required by their licenses. This project imposes no restriction on that use.

Upstream source locations:
- Qt: https://download.qt.io/archive/qt/6.4/6.4.2/submodules/
- SDL: https://www.libsdl.org/release/SDL2-2.30.0.tar.gz
- Distribution source/patches for this build: Ubuntu Noble package source
  repositories for qt6-base, qt6-declarative, qt6-wayland, libsdl2 and the other
  package names listed in `licenses/`. Refer to their copyright files.

Rust dependencies and their full local sources/license notices are included in
`vendor/`: serde/serde_derive/serde_json, sha2/digest/crypto-common/block-buffer,
libc, generic-array, typenum, cfg-if, cpufeatures, itoa, ryu, proc-macro2, quote,
syn, unicode-ident, and version_check. They are pinned to the lockfile versions
and sourced from Ubuntu distribution packages. Some distribution-generated
Cargo checksums are marked “Could not get crate checksum”; the bundled directory
sources are the authoritative build input and the release ZIP has a separate
SHA-256 manifest. Review each dependency's own license before redistributing.

Other dynamically loaded support libraries are listed in the extracted runtime's
`lib/` directory; a machine-generated library-to-notice inventory and distribution copyright notices are included in
`licenses/`. The native graphics stack and glibc remain system dependencies.


Candidate 25 native observation/input uses Qt DBus and the XDG RemoteDesktop/ScreenCast portal, GStreamer/PipeWire capture and Tesseract English OCR. GStreamer plugins, plugin scanner, Tesseract/Leptonica dependencies and eng.traineddata are bundled with distribution copyright notices. Host PipeWire client ABI and the desktop portal/KDE services remain system components. No Python runtime, Codex service or private test artifacts are required. OS capability tokens stay in the installer’s private data directory, never in bundles or reports.

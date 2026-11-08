# Portable build modes

The default public build excludes developer capture/input/window/OCR tools, test checklists and addon-manager downloader/registration execution. Normal setup, repair, removal, folder access, manual pointer instructions, SDL navigation and Steam input ownership checks remain in the public build. Basic --self-check and --demo remain available.

Build public mode with scripts/build.sh, then python3 scripts/package.py --qt-prefix /usr --build-mode public. CMake uses WOW_ENABLE_DEVELOPER_TOOLS=OFF, build-public and target/public. The Rust developer-tools feature defaults off.

For private developer use, build with scripts/build.sh --developer-tools and package with --build-mode developer --output dist-developer. CMake uses WOW_ENABLE_DEVELOPER_TOOLS=ON, build-developer and target/developer. The developer portable has a distinct filename and must not enter public release staging.

Separate Rust/native output directories prevent mode reuse. Both native variants compare their linked Rust feature mode before opening the GUI/core. --build-info reports mode, version and source identity. Packaging checks native/checker/manifest mode and source identities before collecting libraries. Its dependency inventory describes actual bundled files.

Only developer mode packages GStreamer capture plugins/scanner and the English OCR model. Public mode can still include transitive QtDBus dependencies required by ordinary Qt plugins; it does not provide desktop capture or remote input tools. Keep the applicable notices and corresponding source for the actual dependency closure.

The portable uses a private 0700 cache directory, does not require FUSE, and contains no Python runtime. Child Steam/browser launches restore the original library/plugin environment. Preserve matching source, dependency sources, licenses, notices, version, mode and checksums. Exclude device settings, credentials, logs, captures, private helpers and backups.

--self-check renders a labeled installer preview; it does not establish live Steam, device controls or gameplay acceptance. --demo also uses preview state. Developer-only detailed checks and screenshots require an explicit developer build.

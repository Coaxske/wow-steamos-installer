# Portable build

Build on the oldest supported Linux baseline with Qt/SDL development packages.
Run `scripts/build.sh`, then `python3 scripts/package.py --qt-prefix /usr`.
The script gathers the app, Qt QML modules/plugins, their dynamic dependencies
and audited notices (packaging fails on an unmapped library), and compiles `launcher.rs` with that archive embedded.

The launcher extracts into a newly created 0700 directory under the user cache (on disk) and starts
the real application. It retains no browser or Python process. Child Steam and
browser launches restore the pre-bundle library/plugin environment.

`--demo` shows a labeled preview without invoking the filesystem-capable core.
`--self-check` renders that preview and checks the QML root/footer. It does not
validate Steam, graphical session startup, real hardware or gameplay.
`--page setup --size 800x600 --screenshot /tmp/setup.png` can accompany self-check
for visual review. Source-run self-check is separate from packaged self-check.

Preserve the source, vendor dependency tree, runtime copyright/license texts,
version, and checksum manifest with any testing bundle. Do not include personal
runtime state, Steam data, settings backups, development credentials or caches.
This project has no publishing script or remote repository configuration.

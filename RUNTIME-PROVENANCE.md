# Candidate 48 runtime provenance

The Linux x86_64 portable uses Debian 12 Bookworm build inputs, glibc 2.36, Qt 6.4.2 and upstream SDL 2.30.0. Host graphics drivers and glibc remain system-provided. No FUSE or Python runtime is required.

The public mode omits developer capture, external-input, window-reading, checklist and addon-manager download execution. GStreamer capture plugins, the OCR model and their exclusive library closure are not packaged. Ordinary Qt/SDL navigation and their required transitive platform libraries remain.

The final LICENSE-INVENTORY.json identifies actual bundled library-to-notice mappings. Runtime notices and corresponding source are checked against that closure. Upstream copyright and license attributions are retained. These are software attribution, not device or user records.

Source.zip is the matching installer source, including vendored Rust inputs and the optional private developer build switch. Legacy standalone QA drivers and their adapter are excluded from this source snapshot and kept privately. Public assets exclude device reports, settings, logs, captures, private QA packets, backups and transfer URLs.

C48's acceptance is limited to the Bazzite read-only scan and simulated packaged interface checks before Apply. Earlier Steam Deck evidence is described separately in ACCEPTANCE-SUMMARY.md.

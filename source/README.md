# Deckhand source

Deckhand helps set up Blizzard's Battle.net app and one Battle.net button in Steam. Choose an installed WoW edition inside Battle.net. This is the source for Candidate 51, version 0.2.0-alpha.10-candidate.51.

The repository and installer download filename have not changed. Existing installation records remain in wow-steamos-rust. The visible name change does not move games, Proton prefixes, controller associations or guide preferences.

## Build

Use Rust 1.75 or newer and the Qt dependencies described in packaging/README.md. Dependencies are vendored for offline builds.

Run scripts/build.sh for the public build. It runs formatting, offline tests, strict clippy, native compilation, source verification and the interface self-check. Run scripts/build.sh --developer-tools only for a separate private developer build.

Package public mode with python3 scripts/package.py --qt-prefix /usr --build-mode public. The public filename remains WoW-SteamOS-Rust-Linux-x86_64. Keep matching source, runtime source and license notices with release files.

## Update safety and local diagnostics

Updates require explicit Update now consent, the published checksum and the signature trusted by the existing production public key. No signing private key or test feed is included here.

Candidate 50 tracks updater-created temporary files so abandoned files can be removed only when their ownership and identities remain verified. Active work, recovery-referenced files, backups and changed or unrecognized files stay protected. Older unrecorded leftovers may remain.

Normal startup attempts to keep one previous private diagnostic as startup.previous.log before writing startup.log. Logging failures are optional and do not by themselves stop startup. A live or unverifiable native window lock skips logging and leaves Qt to decide whether another window can open. A genuinely held startup.lock still refuses a concurrent launcher. Helper calls leave the saved diagnostic unchanged. These logs can contain local details and must stay private.

## Verification scope

The packaged self-check uses an installer preview and disposable state. It checks the first-sign-in hint on the games-download screen without opening Battle.net. Private updater fixtures use simulated feeds and interface actions with separate test keys; those checks do not establish live GitHub updating, device input or gameplay.

Current release instructions and exact acceptance limits are in the repository's root documentation. Do not relabel earlier Candidate 48 or 49 device results as Candidate 50 results.

## Candidate 51 reports and optional guide

The public build adds a strict cleaned-report snapshot and an optional manual CurseForge guide. Unknown free text is discarded; displayed report text and Copy use the same value. The guide only describes detected folders and opens fixed publisher pages after a choice. It never downloads CurseForge, Gear Lever, runtimes or addons. The existing guided setup remains unchanged. Easy Mode is not included while its vendor-installer handoff remains unresolved.

Candidate 51 checks use isolated profiles and fixtures. They do not establish a real Battle.net sign-in resume or Steam Deck controls. Earlier Candidate 50 results retain their original scope.

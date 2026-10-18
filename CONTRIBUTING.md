# Contributing

Fork the repository, create a branch, and submit a pull request. Run the tests in README before submitting. No contributor license agreement is required; contributions are under the repository's MIT license.

Use a GitHub noreply address or a neutral project identity for public Git author and committer metadata. Check both identities with `git log --format=fuller` before pushing. Upload tools may use the connected account's personal email even when the source files contain no identifying information. Never add personal names, emails, machine-specific home paths, or account-specific links to distributed examples.

Keep catalog additions in `wowsteam/catalog.json`. Folder names are hints, never entitlement checks or authoritative client identity. Document a reliable primary source or an actual install observation for a new hint. Future games should remain selectable manually when metadata changes.

Changes to Steam writes need regression tests for unrelated entries, existing IDs, malformed input, and failure/recovery behavior. Changes to downloads must keep HTTPS, redirect restrictions, bounded transfers, and explicit error paths. Never add `curl | bash`, shell interpolation of user data, silent process killing, or game-directory deletion.

Never ask users for passwords, authentication codes, raw Battle.net logs, full Steam configuration, or complete backups. Report only relevant sanitized diagnostics. Do not include an API key or a proprietary installer in a contribution.

A successful automated test run is not a hardware test. Record SteamOS/Bazzite version, Steam version/channel, Proton version, WoW edition/build, and observed behavior when reporting a real-device test. Test first on a non-Hardcore character.

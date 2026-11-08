Candidate 49 is a test version. It adds optional installer updates.

On a Steam Deck that already had Battle.net and WoW set up, Candidate 49 found both. These have not been checked on a Steam Deck yet: the new update buttons, the update on/off switch, and closing the installer normally. Updating automatically from GitHub has not been confirmed yet.

Candidate 48 was tested on a Steam Deck that already had Battle.net and WoW set up. A brand-new installation has not been tested with Candidate 48 or 49 yet. Bazzite testing is in progress.

Download **[WoW-SteamOS-Rust-Linux-x86_64](https://github.com/Coaxske/wow-steamos-installer/releases/download/v0.2.0-alpha.10-candidate.49/WoW-SteamOS-Rust-Linux-x86_64)**. It is the only file you need to run. Candidate 48 users must download Candidate 49 once by hand to get the updater. Then follow the [step-by-step install guide](https://github.com/Coaxske/wow-steamos-installer#download-and-install).

## Changes

- Quietly checks this project's releases at most once every 24 hours. Test releases are included.
- Offers Update now, Not now and View release page. Only Update now authorizes installing an update.
- Keeps newer-release notices visible when verification files are missing. Automatic installation stays unavailable.
- Provides a Home switch to turn checks off. If saving the choice fails, the switch returns to the saved setting and shows a short error.
- Lets idle unfinished setups check for releases and open the release page. Finish or remove that setup before Update now. Actively running setup, repair or removal blocks checks and installation.
- Requires the exact executable's checksum and digital signature, compares versions numerically and never downgrades.
- Keeps the newest verified previous installer after a successful update. Older completed backups and records are removed only when the updater's ownership records and the files still match. Unrelated files and unresolved recovery are preserved.
- If startup cannot be confirmed and rollback is unsafe, shows the verified backup location when available, plain recovery steps and the releases-page link. It does not guess an unverified backup path.
- Keeps the existing Battle.net button, games, Proton environment, settings, controller layouts and access folders. Developer tools stay excluded from the public build.

## Checks and limits

On the Bazzite build machine, 278 library tests, 41 regression tests, one public-build refusal test and nine portable-wrapper checks passed. Strict clippy and native compilation passed in both public and developer build modes.

Sixteen private packaged scenarios passed, including two successive updates with the same Home and portable, an unrelated lookalike, the actual public build's wrong-key refusal, unconfirmed startup, failed preference saving, idle unfinished setup and an active operation. Test feed replies and Qt actions were simulated. Cryptography, file replacement, restart and rollback operations used real packaged programs with known private-fixture keys. The two-update test advanced only its processes' realtime clock; it left the host clock and saved throttle records unchanged. Recovery dialog arguments were captured by a private substitute, not physically inspected on a device.

The production public key is included for verifying signed updates. The private signing key is stored outside the project and is not included in the installer or source files. Private automated checks use separate test keys. The partial Deck checks do not establish the Home controls, update-setting persistence or normal window exit. A live automatic update from GitHub has not yet been confirmed. Optional stale partial-download cleanup is deferred.

C48's existing-installation results remain separate evidence and do not test the C49 updater. Fresh installation, full gameplay and performance are not established. Bazzite gameplay testing remains in progress.

GitHub receives ordinary connection information during release checks. No game settings, account names or installer records are sent. See [security and privacy](SECURITY.md) and [acceptance details](ACCEPTANCE-SUMMARY.md).

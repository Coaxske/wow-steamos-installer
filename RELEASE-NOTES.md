Candidate 49 is a development test version. Earlier published Candidate 48 was tested on Steam Deck in Game Mode; the new updater has not been checked on Deck.

Download **WoW-SteamOS-Rust-Linux-x86_64** once by hand when Candidate 49 is published. It is the only file you need to run. Candidate 48 does not include the updater. Follow the [step-by-step install guide](https://github.com/Coaxske/wow-steamos-installer#download-and-install). Candidate 49 is not published yet.

## Changes

- Quietly checks this project's GitHub releases at most once every 24 hours, including test releases.
- Offers Update now, Not now and View release page. Only Update now authorizes installing an update.
- Includes a Home switch to turn checks off. Offline failures let the installer continue silently.
- Pauses updates during setup, repair or removal and while any saved setup is unfinished.
- Requires the exact executable's SHA-256 checksum and Ed25519 signature before use. Versions are compared numerically and never downgraded.
- Keeps the old portable as a backup. If replacement or startup fails, it is retained or restored. If the new process's exit cannot be confirmed, recovery files are retained instead of attempting an unsafe rollback.
- Keeps the existing single Battle.net button, installation, Proton environment, settings, controller layouts and access-folder behavior. Developer tools stay excluded from the public build.

## Checks and limits

Offline tests and simulated packaged checks ran on the Bazzite build machine. Private-feed cases covered signed update, cache aliases, failed-start rollback, checksum and signature failures, interrupted downloads, an unwritable folder, Not now, persisted opt-out and silent offline throttling. Test feed replies and Qt actions were simulated; the packaged file, signature and restart operations were real. Temporary fixture keys were used. This does not verify live GitHub transport or an actual Deck session.

No production signing key is configured in this development copy, so automatic installation is refused. Key provisioning, final release files and Candidate 49's Deck checks remain pending. This version must not be published in that state.

Published Candidate 48 has existing-installation Steam Deck Game Mode Retail and Classic Era results, Desktop discovery/Repair/folder checks, and a firsthand Forever observation with recorded interruption limits. Those results do not test Candidate 49's updater. Fresh installation was not part of that device check. Bazzite gameplay testing remains in progress.

GitHub receives ordinary connection information when checking public releases. No game settings, account names or installer records are sent. See [security and privacy](https://github.com/Coaxske/wow-steamos-installer/blob/main/SECURITY.md) and [acceptance details](https://github.com/Coaxske/wow-steamos-installer/blob/main/ACCEPTANCE-SUMMARY.md).

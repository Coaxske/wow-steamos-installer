Candidate 50 is a test version. The installer is now called Deckhand. It adds guarded cleanup of interrupted update files and keeps the previous startup log.

Download **[WoW-SteamOS-Rust-Linux-x86_64](https://github.com/Coaxske/wow-steamos-installer/releases/download/v0.2.0-alpha.10-candidate.50/WoW-SteamOS-Rust-Linux-x86_64)**. It is the only file you need to run. Keep it somewhere permanent and follow the [step-by-step install guide](https://github.com/Coaxske/wow-steamos-installer#download-and-install). Candidate 48 users must download an installer with the updater once by hand. Candidate 49 already includes update checks.

## Changes

- The installer window and visible name are Deckhand. The download filename, saved setup records and Battle.net button in Steam stay the same.
- Beside Open Battle.net during setup, a new hint explains what to do if the first sign-in shows an unexpected-error message. This is guidance, not a proven fix for that unexplained crash.
- On the next startup, abandoned temporary update files can be removed only when private ownership records and file identities still match. Active updates, unresolved recovery files, backups, the installed executable, unrelated files and changed files are preserved.
- Older leftover files without trustworthy ownership records stay in place. Unknown files created inside a temporary folder also prevent its removal.
- Normal startup keeps one previous private diagnostic as startup.previous.log before writing startup.log. The restarted installer writes its own early startup messages. Helper commands and another Candidate 50 window for the same active profile leave the existing logs alone.
- Games, Proton setup, controller layouts, settings and game-folder links are unchanged. Developer capture, mouse and window tools stay excluded from the public build.

## Checks and limits

On the Bazzite build machine, 287 library tests, 41 regression tests, one public-build refusal test, 14 portable-wrapper checks and 12 private packaged scenarios passed. Formatting, strict clippy and native builds passed in public and developer modes. The developer offline suite also passed 287 library and 41 regression tests. Checks used disposable profiles. Simulated network replies, interface actions and failures exercised interrupted downloads, staged files, active writers, recovery, restart and rollback. Real packaged programs performed checksum and signature checks and file operations with separate test keys. The public build retains the existing production verifier.

These checks do not establish a live update from published Candidate 49 to Candidate 50. That test is planned after publication on Bazzite first, then Steam Deck, with sampled observations of the restarted window for 30 minutes. Candidate 50 has not been run on Steam Deck. No new game, physical-controller, fresh-installation or performance result is claimed.

Earlier Candidate 48 Steam Deck game checks and Candidate 49 fresh Bazzite installation and update checks remain separate evidence. The unexplained later closure of the updated Candidate 49 Deck window is still recorded. Startup logs may help investigate a future failure; they do not establish the cause of that earlier event.

GitHub receives ordinary connection information during update checks. No game settings, account names or installer records are sent. See [security and privacy](SECURITY.md) and [acceptance details](ACCEPTANCE-SUMMARY.md).

# Validation status

On 2026-10-18, all 37 automated tests passed locally, including a simulated fresh setup, Battle.net target replacement, two client registrations, and a rerun retaining IDs. Compilation and shell syntax checks also passed. GitHub-hosted CI for the previous alpha passed on Python 3.10–3.13, including source packaging. No Blizzard credentials, real Steam installation, WoW client, or physical Steam Deck is available in the development environment. **Automated checks are not end-to-end game compatibility tests.**

Four additional packaging tests exercise the standalone zipapp and desktop Exec command, catalog loading outside the source tree, file URLs and paths with spaces, corrupt embedded payload rejection, reproducibility, and distribution contents. KDE trust prompts and terminal integration still need a physical-device test.

Tests exercise real reads/writes against temporary fake Steam directories. Only host boundaries such as running-process checks, prompts, downloads, and launcher opening are replaced by test doubles.

Covered risks include binary/text VDF integrity, compact mapping edits, malformed data, stable IDs, name collisions, preserved artwork, manual conflicts, matching shared-prefix options, repeat runs, conditional interrupted-write recovery, non-destructive removal, duplicate clients, path quoting, process recognition, and failed-download cleanup.

## Device checks before a stable release

Record the OS version, Steam channel/version, Proton tool, and client build for each result. Leave untested cells marked untested; do not infer one edition from another.

| Scenario | Steam Deck SteamOS | Bazzite native Steam |
| --- | --- | --- |
| Fresh setup and Battle.net login | Untested | Untested |
| Two selected editions, full downloads | Untested | Untested |
| Direct client launch and authentication | Untested | Untested |
| Gaming Mode controller layout retained | Untested | Untested |
| Update in Battle.net, exit, relaunch | Untested | Untested |
| Add edition after setup | Untested | Untested |
| Repeat setup retains IDs/artwork | Untested | Untested |
| Remove owned tiles preserves games | Untested | Untested |
| Desktop launcher opens from extracted ZIP | Untested | Untested |
| Existing-prefix explicit reuse | Untested | Untested |

Test these editions separately when available: Retail, Forever, Era, progression, Anniversary, PTR, beta, and a custom selection. In Era, use a non-Hardcore character for control and connection checks before attempting Hardcore play.

Back up your Steam configuration and account data before real-device testing. Do not test recovery by killing Steam while it is writing; fault-injection tests already cover interrupted tool writes without risking an actual library.

## Local commands

```bash
python3 -m unittest discover -s tests -v
python3 -m compileall -q wowsteam wow_on_steam.py
bash -n start.sh
python3 scripts/package.py
```

The package builder includes only explicitly allowed source/docs paths. Run the tests again after extracting the ZIP into a path with spaces to catch missing package resources and path assumptions.

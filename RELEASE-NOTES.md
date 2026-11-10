Candidate 51 is a test version. It adds cleaned problem reports and optional addon help.

The current download is **[WoW-SteamOS-Rust-Linux-x86_64](https://github.com/Coaxske/wow-steamos-installer/releases/download/v0.2.0-alpha.10-candidate.50/WoW-SteamOS-Rust-Linux-x86_64)**, Candidate 50. Candidate 51 is under review and has not been published. Follow the [step-by-step install guide](https://github.com/Coaxske/wow-steamos-installer#download-and-install) for the current download.

## Changes

- View private diagnostics shows one cleaned snapshot of startup.log and startup.previous.log. The preview is exactly what Copy diagnostic text copies. Recognized errors and exit codes are kept. Unknown free text and private details are omitted. Nothing is sent automatically.
- An optional CurseForge guide starts off. It opens the official publisher page only when requested and explains how to add a downloaded AppImage with Gear Lever. Deckhand does not download, install, bundle or re-host either app.
- The guide shows each detected game's actual game and AddOns folders, including available connected drives. Missing, unavailable or ambiguous folders are reported instead of guessed or created. Existing addons stay in place.
- The existing guided setup remains available. Easy Mode is not included: the supported handoff through Blizzard's interactive installer is not yet established.
- Existing update verification, installation records, the single Battle.net button and developer-tool separation are retained.

## Checks and limits

On the Bazzite build machine, 314 library tests and 41 regression tests passed in both build modes; one additional public-build refusal test passed. Formatting, strict clippy and native builds passed in both modes. Seven private packaged scenarios and 16 wrapper checks passed. These used isolated profiles, synthetic files and a network-disabled container. They did not use the working Steam client, Blizzard account or game installation. The acceptance details record their limits.

No Candidate 51 Steam Deck, real Battle.net sign-in, game, controller, fresh-installation or live-update result is claimed. The simulated checks do not prove real sign-in or resume. Earlier Candidate 48, 49 and 50 results retain their original version and method limits.

CurseForge's official Linux support is for Ubuntu. This manual guide does not establish support on Steam Deck or Bazzite. If chosen, Gear Lever comes from Flathub through Discover; it and additional Flatpak runtimes must be reviewed and installed by the player.

See [security and privacy](SECURITY.md) and [acceptance details](ACCEPTANCE-SUMMARY.md).

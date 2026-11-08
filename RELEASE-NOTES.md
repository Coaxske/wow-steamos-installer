Candidate 48 was tested on a Steam Deck that already had Battle.net and WoW set up. A brand-new installation has not been tested with this version yet. Bazzite testing is in progress.

Download **[WoW-SteamOS-Rust-Linux-x86_64](https://github.com/Coaxske/wow-steamos-installer/releases/download/v0.2.0-alpha.10-candidate.48/WoW-SteamOS-Rust-Linux-x86_64)**. It is the only file you need to run. The other files on this page contain instructions, checksums, licenses and source code. Then follow the [step-by-step install guide](https://github.com/Coaxske/wow-steamos-installer#download-and-install).

## Changes

- Finds additional existing installations using verified locations from Steam shortcuts.
- Adds **Choose existing folder** when nothing is found.
- Keeps separate copies distinct and requires review before reuse.
- Explains empty results and missing matching Steam shortcuts. Finding files alone does not establish that the game will reach Play.
- Includes missing Battle.net button repair.
- Excludes developer checklists, screen-capture, external mouse-control, window-reading and addon-manager download tools from the public executable.

## Checks and limits

Candidate 48 was tested on a Steam Deck that already had Battle.net and World of Warcraft set up.

- **Game Mode:** Retail and Classic Era each started from the Battle.net button and reached the character screen. Battle.net closed itself while the game kept running. The trackpad worked as a mouse in menus, and quitting through the game's own Exit Game option returned to the same Battle.net button, ready to play again.
- **Desktop Mode:** the installer found the existing installation without needing a folder to be chosen. Repair kept the existing Battle.net button, the Battle.net app, its Proton setup and the controller settings files. Open folders worked, and removing and re-creating the folder shortcuts left the checked game and addon files in place.
- **Forever:** the tester saw Forever working, but the recorded test was interrupted, so Forever is still marked experimental.

Not yet tested with this version: a brand-new installation, the Choose existing folder option on a Steam Deck (it was checked with simulated tests on Bazzite), full gameplay and performance. Bazzite testing is in progress. Error134 and WOW51900319, seen in earlier testing, are still unexplained.

See [acceptance details](ACCEPTANCE-SUMMARY.md) for the exact test records and their limits.

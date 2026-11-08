Published Candidate 48 was tested on Steam Deck in Game Mode. Bazzite testing is in progress.

Download **[WoW-SteamOS-Rust-Linux-x86_64](https://github.com/Coaxske/wow-steamos-installer/releases/download/v0.2.0-alpha.10-candidate.48/WoW-SteamOS-Rust-Linux-x86_64)**. It is the only file you need to run. The other files on this page contain instructions, checksums, licenses and source code. Then follow the [step-by-step install guide](https://github.com/Coaxske/wow-steamos-installer#download-and-install).

## Changes

- Finds additional existing installations using verified locations from Steam shortcuts.
- Adds **Choose existing folder** when nothing is found.
- Keeps separate copies distinct and requires review before reuse.
- Explains empty results and missing matching Steam shortcuts. Finding files alone does not establish that the game will reach Play.
- Includes missing Battle.net button repair.
- Excludes developer checklists, screen-capture, external mouse-control, window-reading and addon-manager download tools from the public executable.

## Checks and limits

Published C48 was tested on an existing Steam Deck setup. Retail and Classic Era reached character screens in Game Mode. Passive observation recorded automatic main Battle.net exit while the same correct game stayed alive, with Agent tracked separately. The tester used the physical trackpad for pointer/menu input, quit through WoW's own Exit Game, and returned to the same Battle.net button ready to Play. Reopening that button switched from Retail to Era; separate same-edition relaunch pairs were not run.

In Desktop Mode, automatic discovery found the working copy without the folder picker. Normal reviewed Repair retained the existing tile, launcher, Proton environment and controller files. Open folders and link-only removal/recreation reached the expected installation and AddOns targets and preserved the checked contents. Preservation checks were bounded samples, not a full game-tree comparison.

The tester personally observed Forever working correctly. An initial recorded client 1.60.1.70862 run showed automatic main-launcher exit while the same game survived. A later session was interrupted and used a different game process. These separate observations do not establish one uninterrupted instrumented Forever cycle. Forever remains experimental.

Optional fresh setup was not run and awaits separate approval. Bazzite's existing-folder picker checks used simulated interface tests before Apply; the actual Deck did not need that picker. Bazzite gameplay testing remains in progress.

Full gameplay and performance are not established. Numeric launcher exit codes, continuous process/display traces and the effective live controller-layout association were not established. UNKNOWN launcher-window queries are not absence proof. Historical Error134 and WOW51900319 remain separate unexplained results, with no cause or repair claimed.

See [acceptance details](https://github.com/Coaxske/wow-steamos-installer/blob/main/ACCEPTANCE-SUMMARY.md) for exact build identities and evidence limits.

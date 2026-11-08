# Candidate 48 acceptance

Earlier versions were tested on Steam Deck. This version was checked on a Bazzite PC.

Public installer SHA-256: **19508288b4ace65e230930c704609b23be6e176fe1fc9a837a693abc13e2f6ac**.
Matching Source.zip SHA-256: **eebea66950b2235d4cb3c08be4dc8b039b7ee3e26f5952d70ae63f63ceaf8150**.
Source fingerprint: **2ab87185e310d415cca876cb72af059c06563596866dc2ec8356ad3a7e026017**.

| Check | Result and limit |
|---|---|
| C48 offline source tests | PASS: 246 library tests, 41 regression tests and 1 production public-mode refusal test; formatting and strict clippy passed on Rust 1.75. |
| C48 public build | PASS: default public build excludes developer checklist, capture, external mouse control, window reading and addon-manager download execution. The separate private developer build compiles and its offline tests pass. |
| C48 actual Bazzite discovery | PASS: an empty-record, network-disabled read-only scan found two Battle.net installations and seven WoW client paths with no warnings. No adoption, download, launcher or game launch occurred. |
| C48 packaged interface, simulated | PASS: ordinary folder-picker cancel, empty-folder retry, recognized external folder, missing-shortcut refusal, separate-copy selection and correct launcher/prefix/Proton/no-download review. Stopped before Apply. A separately labeled simulated pending-route case checked Back cancellation. |
| C48 preservation | PASS within the sampled boundary: active installer records, selected metadata/controller/game samples and access-link identities were unchanged. Live paths were mounted read-only during discovery. This is not a full game-tree comparison. |
| C48 source and runtime | PASS: 894 source inputs match Source.zip exactly; 768 runtime files match the embedded payload; all 87 library-to-notice mappings remain covered. Private QA drivers and device records are excluded. |
| C48 Steam Deck, game and physical input | NOT TESTED. No C48 Deck, gameplay, physical-controller or installation-Apply pass is claimed. |
| Earlier C46 Bazzite repair | Real Steam missing-tile repair retained the saved identity, launcher, prefix and Proton mapping. A separate adopted Classic Era launcher check offered Play without pressing it. These are earlier-build observations, not C48 acceptance. |
| Earlier C43 Steam Deck Game Mode | Era and Retail used the single Battle.net tile. The main launcher exited while the same identified WoW process survived; normal quitting returned to that tile. Era pointer/menu input passed with the existing generic Battle.net layout. |
| Earlier Forever | Forever works on Steam Deck as of C44. Earlier formal checks reached character selection/creation. Client 1.60.1.70862 has no separate recorded test. It remains experimental. |
| Earlier installation and removal | Accepted prerequisite/rerun, protected-data, folder-link and backed-up removal/restoration checks retain their original build and simulated or actual-device method limits. They were not repeated for C48. |

The C48 interface driver is private and external to the public executable. It invokes ordinary public controls with disposable files in an offscreen environment. These are simulated interface checks, not live Desktop or Steam Deck interaction.

Historical observations keep Battle.net, Agent, the game process and launcher windows separate. UNKNOWN window results are not absence. Startup/current process correlation is not a continuous lifetime trace or a numeric launcher-exit-status proof. Earlier brightness checks are bounded observations, not continuous physical measurements. Normal display and sleep policies remain unchanged.

Error134 and WOW51900319 remain separate unexplained historical results. Later success does not prove their cause or a repair. Historical QA time limits are not product startup requirements. Full gameplay and performance are not established.

Private logs, captures, settings, backups, device identifiers, handoffs, QA packets and transfer addresses are not public assets.

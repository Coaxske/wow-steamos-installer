# Candidate 49 development acceptance

Candidate 49 is not released. The production public verifier is configured; the private key is outside the project.

On a Steam Deck with an existing installation, Candidate 49’s command-line check found Battle.net and WoW. The installer’s Home controls, whether the update setting persisted after reopening, and normal window exit remain unverified. A live automatic update from GitHub has not yet been confirmed. No Candidate 49 game-session or physical-controller pass is claimed.

## Automated development checks

These checks ran on the Bazzite build machine in an isolated container, using disposable files and records. They did not use the working Steam installation, game prefix or Blizzard account.

| Check | Result and limits |
| --- | --- |
| Required formatting and offline tests | PASS: 278 library tests, 41 regression tests and one public-build refusal test. |
| Public native build | PASS: strict clippy in both public and developer modes, both native builds, source correspondence and offscreen native self-check. |
| Portable handoff checks | PASS: nine serial checks, including READY/exit binding, retained recovery, cache-parent aliases and refusal of a final runtime symlink. |
| Packaged private-feed update | PASS: simulated feed and synthetic compiled test key; actual checksum/signature, staging, atomic replacement, restart and prior-version backup. A second case used an aliased cache parent. |
| Packaged failed-start rollback | PASS: controlled candidate exit before readiness restored the original executable and inode, then reopened it. No real service or game was stopped. |
| Packaged refusal paths | PASS: incorrect checksum, incorrect signature, interrupted download and unwritable portable directory kept the original usable. |
| Newer releases with missing verification files | PASS in simulated packaged checks: missing signature or checksum kept the newer-release notice and canonical page available, disabled Update now and made no asset download. Offline checks also cover foreign or duplicate asset metadata and manual-only cache defaults. |
| Packaged ordinary choices | PASS: Not now kept Home usable; opt-out persisted after reopening; offline failure remained silent and did not repeat within the day. |
| URL and release policy | Offline checks cover numeric ordering, prereleases, drafts, no downgrade, daily throttling, opt-out, busy or unfinished workflows, repository URL boundaries and redirect/header rejection. No live GitHub transport test is claimed. |
| New dependencies | Eighteen exact-version additions were checked against official archive metadata and complete per-file manifests. The original nineteen packages remain unchanged from the verified C48 baseline; their sparse inherited manifests are not a new complete upstream-file audit. Notices are included. |
| A: newest previous-installer retention | PASS: two successive authenticated private updates used the same Home and target. The newest verified previous copy and record remained; the older completed pair was removed. An unrelated lookalike pair retained its bytes, inode and mode. Offline tests preserve changed-inode backups and reject missing or wrong-nonce readiness. |
| B: unconfirmed startup recovery | PASS in a private proof-collision fixture: rollback stayed blocked, the current and previous portable bytes remained, and the requested error-dialog arguments named the actual verified backup path with safe recovery steps and the canonical releases URL. This is simulated dialog capture, not physical display acceptance. |
| C: failed Off save | PASS: a read-only fixture settings directory made the ordinary checkbox save fail. A fresh readback restored the visible persisted setting, showed a short error and left the notice undiscarded. A later ordinary Off save and reopening agreed on disabled checks. |
| D: unfinished versus active work | PASS: a held operation lock blocked the check and installation, with no network request or daily-attempt consumption. Reopening the same idle unfinished fixture fetched metadata once, kept the release-page link, disabled Update now and returned the exact required backend refusal. A simulated saved-stage completion followed by normal init/status handlers refreshed readiness without another feed request. |
| Configured public verifier | PASS: the intended production public key is present in the actual packaged native executable; the test verifier is absent. The actual public portable rejects the private feed's otherwise valid test-key signature and keeps its original bytes. Missing signatures, tampered bytes and wrong-key files remain refused. The production private key was not used in these checks. |
| Actual C49 Deck command-line identity and discovery | Partial scoped check: verified public-mode build and backend discovery of the existing Battle.net/WoW files. This is not Home/control evidence. |
| Actual C49 normal invocation and window | One normal no-argument invocation and one matching desktop window were recorded. Process and KWin window metadata only; no Home-render or switch proof. No person observed. |
| Actual C49 update attempt | A genuine saved attempt appeared with enabled=true and no candidate. Attempt/budget evidence only; successful HTTP, notice and live updating are unverified. History is preserved. |
| Actual C49 Home controls, preference persistence and normal exit | UNVERIFIED. The existing accessibility route did not identify native-owned controls; no switch action or reopen test ran. The report ended with one owned window open. Hand check is deferred; no tests queued. |
| Actual C49 preservation | Earlier bounded comparison:52of54 files byte-identical, with wrapper startup-log truncation and an app-info-only Steam metadata change as exceptions; writer/cause of that metadata change unknown. No new full comparison after the window launch, full game-tree equality or live layout proof. |
| Actual C49 games or physical input | NOT RUN. Earlier C48 results below remain labeled C48. |
| Production signing and final release package | Public verifier configured. Live signed-release testing and final release approval are pending. No private key is included in the build or source. These development snapshots are not final release artifacts. |

The positive update and rollback checks start from a copy of this candidate with a separate known test public key compiled in. The C50 and C51 targets are retained private fixtures from the earlier updater checks, with their original source and method limits. The actual public candidate is separately checked for rejection of those test-key signatures. Their hashes differ from the public-mode development snapshot. Network replies and Qt actions were simulated privately. The two-update fixture advanced only its test processes' realtime clock; it did not edit saved throttle records or change the host clock. No private feed override, synthetic key, QA driver, log, capture, settings or backup is part of the public build.

Daily checks and the release-page link remain available for idle unfinished saved setups, including older copies. Update now stays disabled and the backend refuses installation until the setup is finished or removed. Active operation locks block both checks and installation without consuming the daily attempt. Unverified child exit prevents automatic rollback; the backup and recovery records are retained for manual reopening. Normal display and sleep policy is unchanged.

## Public-mode development snapshot, not a release

Portable SHA-256: **d17151020ccce00d7fd2e384b33e7ec7c437549552795079a50bf2084a71fd5d**.

Matching Source.zip SHA-256: **1c03e2443d4ae8f8802ad418e443d968836037bc3ec6a6811846a214528b8e7f**.

Source fingerprint: **7f137a4546a3101b799e923927d4335e9bde993cf76c48d18d5feb7e0d721775**.

These identify the private development snapshot containing the production public verifier. It has not been signed with the production private key and is not a released Candidate 49 installer. A recovery-test fixture was corrected after CI exposed an inode-reuse assumption. The replacement is now created before the original is replaced. Production recovery code is unchanged, and the corrected full suite passes. Earlier private packaged results retain their original simulation limits.

---

# Candidate 48 acceptance

Candidate 48 was tested on a Steam Deck that already had Battle.net and WoW set up. A brand-new installation has not been tested with this version yet. Bazzite testing is in progress.

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
| C48 actual Steam Deck, existing setup | PASS within the recorded scope: verified published executable, private recovery backups, automatic working-copy discovery, normal reviewed Repair, real Steam close/reopen and existing Battle.net button retained. The folder picker was not needed or exercised on Deck. |
| C48 actual Steam Deck Game Mode, Retail and Classic Era | PASS within the recorded scope: Retail 12.1.5.70783 and Era 1.15.9.70576 reached character screens; passive startup/lifetime evidence recorded automatic main Battle.net exit while the same correct game survived. Agent was tracked separately. |
| C48 Deck physical input and normal quit | PASS by firsthand physical reports: trackpad pointer/menu click, WoW's own Exit Game and the same Battle.net button ready to Play. Retail was followed by Era through that button. This is cross-edition reopening, not a fresh same-edition pair for each game. |
| C48 actual Deck Desktop folders and links | PASS within the sampled boundary: Open folders reached the expected file-manager location and seven links; link-only removal and recreation preserved verified targets. Controller-file contents, sizes and modes were unchanged. The effective live layout association was not independently read back. This does not establish full game-tree equality or addon functionality. |
| C48 public interface on Deck | Observed normal Home, Games, Repair, Remove and Advanced surfaces exposed no Test this build, capture or addon-manager navigation. |
| C48 Forever | The tester personally observed Forever working correctly. The initial client 1.60.1.70862 run recorded automatic main-launcher exit while the same correct game survived. A later session was interrupted and had a different game lifetime with unrecorded startup/launcher overlap. These remain separate observations, not one uninterrupted instrumented cycle. Forever remains experimental. |
| C48 optional fresh setup | NOT RUN, pending separate approval. No new-installation pass is claimed. |
| Earlier C46 Bazzite repair | Real Steam missing-tile repair retained the saved identity, launcher, prefix and Proton mapping. A separate adopted Classic Era launcher check offered Play without pressing it. These are earlier-build observations, not C48 acceptance. |
| Earlier C43 Steam Deck Game Mode | Era and Retail used the single Battle.net tile. The main launcher exited while the same identified WoW process survived; normal quitting returned to that tile. Era pointer/menu input passed with the existing generic Battle.net layout. |
| Earlier Forever | Forever works on Steam Deck as of C44. Earlier formal checks reached character selection/creation. The earlier lack of a separate recorded test for client 1.60.1.70862 is historical; the limited C48 observations above add evidence without establishing an uninterrupted full cycle. |
| Earlier installation and removal | Accepted prerequisite/rerun, protected-data, folder-link and backed-up removal/restoration checks retain their original build and simulated or actual-device method limits. They were not repeated for C48. |

The C48 interface driver is private and external to the public executable. It invokes ordinary public controls with disposable files in an offscreen environment. These are simulated interface checks, not live Desktop or Steam Deck interaction.

Deck checks kept main Battle.net, Agent, the game process and launcher windows separate. The passive observer was armed before Play. Numeric process exit codes were not established, and an observer success code establishes only its recorded overlap/lifetime condition. A destroyed launcher's later window query remained UNKNOWN. File comparisons used fresh baselines and samples; settings changes after games and before link checks were not attributed to link operations. No continuous physical brightness measurement was made. Historical observations also keep Battle.net, Agent, the game process and launcher windows separate. UNKNOWN window results are not absence. Startup/current process correlation is not a continuous lifetime trace or a numeric launcher-exit-status proof. Earlier brightness checks are bounded observations, not continuous physical measurements. Normal display and sleep policies remain unchanged.

Error134 and WOW51900319 remain separate unexplained historical results. Later success does not prove their cause or a repair. Historical QA time limits are not product startup requirements. Full gameplay and performance are not established.

Private logs, captures, settings, backups, device identifiers, handoffs, QA packets and transfer addresses are not public assets.

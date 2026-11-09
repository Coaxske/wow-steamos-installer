# Candidate 50 acceptance

Candidate 50 is prepared for commit-specific review. It is not published. These checks ran on the Bazzite build machine in an isolated container with disposable profiles. No C50 Steam Deck, live GitHub update, game-session or physical-controller result is claimed.

## C50 automated checks

| Check | Result and limits |
| --- | --- |
| Formatting | PASS: cargo fmt and standalone packaging rustfmt checks. |
| Public offline suite | PASS: 287 library tests, 41 regression tests and one public-build refusal test. |
| Private developer offline suite | PASS: 287 library tests and 41 regression tests. |
| Static checks and native builds | PASS: strict clippy and native compilation in both modes; source correspondence and offscreen self-checks. |
| Portable wrapper | PASS: 14 serial checks, including five new startup-log rotation, active-window, link and lifetime checks. |
| Public identity and setup hint | PASS: the packaged preview reports version 0.2.0-alpha.10-candidate.50, Deckhand and the exact first-sign-in hint on the game-download screen beside Open Battle.net. The hint is hidden on the install step and Home. No launcher was opened by this check. |
| Saved guide preference | PASS: a disposable profile using the earlier application-name namespace retained its guide choice. The renamed app did not create a different preference file. |
| Abandoned updater files | PASS in nine new focused simulated unit tests and packaged interruption checks. Owned partial downloads and staged files were removed after abandonment; repeated cleanup was harmless. Active owners or child-held activity locks, unresolved handoff/recovery files, backups, current executables, unknown content, unrelated lookalikes, replaced identities and edited sealed files were preserved. Cleanup also ran with daily checks off, without consuming the daily attempt, and deferred for an active operation. |
| Packaged update compatibility | PASS: private C49-to-C50 and C50-to-higher-version fixtures verified signatures/checksums, retained the previous executable and restarted. These fixtures replace only the production public verifier with a separate synthetic test verifier; the higher-version fixture also changes its version. They are not live GitHub updates or the published C49 executable. |
| Packaged recovery | PASS: a controlled pre-readiness failure restored and reopened the prior copy. An unconfirmed-successor fixture retained the verified backup and recovery records and named that backup in captured error-dialog arguments. |
| Startup diagnostics | PASS: normal rotation, helper calls, duplicate C50 windows, an active legacy C49 window, update restart, rollback and an injected early extraction failure. The real packaged launcher recorded the injected early failure even with its outer standard error discarded. Separate current/previous logs survived. |
| Public trust and exclusions | PASS: the actual public portable rejects the private feed's otherwise valid wrong-key signature. The existing production verifier is retained, with developer tools disabled. URL boundaries, consent, checksum/signature, no-downgrade, throttle and unfinished/active-operation checks remain in the offline suite. |
| Source and licenses | PASS: Source.zip CRC and 1396 entries match the packaged source inputs. No dependency or runtime-component version changed. The 87-entry runtime license inventory matches the previously reviewed C49 inventory; retained runtime source/notices archives passed CRC checks. |
| Packaged scenario total | PASS: 12 private scenarios. Network replies, Qt actions, keys and failures were simulated; file identity, locking, cryptography, replacement, restart and rollback used actual packaged programs. |

## C50 build identity

- Portable SHA-256: **0cb72df9ae462c51f78b8fa2fce856c1de59964165ffce50eef08a60b88090b7**
- Source.zip SHA-256: **18704dc999f609b12dde6a020229928b3cbad32ca3e9bc781084633b12c41d62**
- Source fingerprint: **b18cf27dd81bb33737c50f377b17a787efe34b37956293fb9ce52ca95d7eafd4**
- Native payload SHA-256: **4e0a892c595a535928b397eab9adc199bacb81ad6e6a220c8db35323b6ae1903**
- Build mode: public; developer tools disabled.

The private test fixtures have different hashes and remain outside public files. No production private key was used for these checks. The production verifier is unchanged.

## C50 limits and later live test

Cleanup requires durable ownership records and matching file identities. Legacy files, an interruption before ownership registration, unknown helper-created cache content and uncertain records may leave leftovers. No name-only cleanup is attempted. Files still needed by active work or unresolved recovery are kept.

Candidate 50 prevents its own helper and duplicate-window paths from erasing diagnostics and checks the existing native window lock before rotating a legacy window's log. It cannot retrofit the older Candidate 49 launcher: a rollback into C49 still uses C49's logging behavior. An unwritable or invalid profile may prevent any log from being created.

The sign-in hint is wording only. The C49 first-sign-in crash and later Deck updater-window closure remain unexplained. Log preservation does not establish a cause or guarantee a fix.

After review and publication, use the actual published C49 in one disposable profile per device with the normal GitHub feed: Bazzite first, then Steam Deck. Check Not now, normal close/reopen, Update now, production signature, previous and replacement hashes, restart, sampled survival over 30 minutes and ordinary close. This has not run for C50. If the window disappears, preserve both logs and last-seen/first-absent times before reopening. No repeated gameplay or controller check is part of that plan.

Private QA code, raw logs, captures, settings, backups, device identifiers and handoffs are excluded from this repository and public release files. Existing games, Steam metadata, layouts and display settings were not changed by these isolated checks.

---

## Historical Candidate 49 acceptance

The following earlier records retain their original build and method limits.

Candidate 49 is a published pre-release. The production public verifier is included; the private key remains outside the project.

On a Steam Deck that already had Battle.net and WoW set up, Candidate 49 found both. In a live test on that Deck, an older test copy of the installer found Candidate 49 on GitHub. After Update now was pressed, it downloaded and installed the signed update, kept the previous copy as a backup, and restarted as Candidate 49. That updated window later closed unexpectedly before it could be closed normally. The cause is not known, but the update had finished: Candidate 49 and the backup were still in place. In a separate check, Candidate 49 stayed open on the Deck through checks every two minutes for 30 minutes, then closed normally. The update on/off switch also worked on the Deck: after it was turned off, it stayed off when the installer was reopened. No Candidate 49 game-session or physical-controller pass is claimed.

## Automated development checks

The offline and private-feed checks in this section ran on the Bazzite build machine in an isolated container, using disposable files and records. They did not use the working Steam installation, game prefix or Blizzard account. Rows labeled Earlier prepublication preserve separate historical Deck and package outcomes; later live-device results are recorded separately below.

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
| URL and release policy | Offline checks cover numeric ordering, prereleases, drafts, no downgrade, daily throttling, opt-out, busy or unfinished workflows, repository URL boundaries and redirect/header rejection. These offline checks do not establish live GitHub transport. |
| New dependencies | Eighteen exact-version additions were checked against official archive metadata and complete per-file manifests. The original nineteen packages remain unchanged from the verified C48 baseline; their sparse inherited manifests are not a new complete upstream-file audit. Notices are included. |
| A: newest previous-installer retention | PASS: two successive authenticated private updates used the same Home and target. The newest verified previous copy and record remained; the older completed pair was removed. An unrelated lookalike pair retained its bytes, inode and mode. Offline tests preserve changed-inode backups and reject missing or wrong-nonce readiness. |
| B: unconfirmed startup recovery | PASS in a private proof-collision fixture: rollback stayed blocked, the current and previous portable bytes remained, and the requested error-dialog arguments named the actual verified backup path with safe recovery steps and the canonical releases URL. This is simulated dialog capture, not physical display acceptance. |
| C: failed Off save | PASS: a read-only fixture settings directory made the ordinary checkbox save fail. A fresh readback restored the visible persisted setting, showed a short error and left the notice undiscarded. A later ordinary Off save and reopening agreed on disabled checks. |
| D: unfinished versus active work | PASS: a held operation lock blocked the check and installation, with no network request or daily-attempt consumption. Reopening the same idle unfinished fixture fetched metadata once, kept the release-page link, disabled Update now and returned the exact required backend refusal. A simulated saved-stage completion followed by normal init/status handlers refreshed readiness without another feed request. |
| Configured public verifier | PASS: the intended production public key is present in the actual packaged native executable; the test verifier is absent. The actual public portable rejects the private feed's otherwise valid test-key signature and keeps its original bytes. Missing signatures, tampered bytes and wrong-key files remain refused. The production private key was not used in these checks. |
| Earlier prepublication C49 Deck command-line identity and discovery | Partial scoped check: verified public-mode build and backend discovery of the existing Battle.net/WoW files. This is not Home/control evidence. |
| Earlier prepublication C49 normal invocation and window | One normal no-argument invocation and one matching desktop window were recorded. Process and KWin window metadata only; no Home-render or switch proof. No person observed. |
| Earlier prepublication C49 update attempt | A genuine saved attempt appeared with enabled=true and no candidate. Attempt/budget evidence only; successful HTTP, notice and live updating are unverified. History is preserved. |
| Earlier prepublication C49 Home controls, preference persistence and normal exit | UNVERIFIED. The existing accessibility route did not identify native-owned controls; no switch action or reopen test ran. The report ended with one owned window open. At that earlier point, a hand check was deferred; the later results below add evidence without rewriting this outcome. |
| Earlier prepublication C49 preservation | Earlier bounded comparison: 52 of 54 files byte-identical, with wrapper startup-log truncation and an app-info-only Steam metadata change as exceptions; writer/cause of that metadata change unknown. No new full comparison after the window launch, full game-tree equality or live layout proof. |
| Earlier prepublication C49 games or physical input | NOT RUN. Earlier C48 results below remain labeled C48. |
| Earlier prepublication signing/package status | Public verifier configured. Live signed-release testing and final release approval are pending. No private key is included in the build or source. These development snapshots are not final release artifacts. |

The positive update and rollback checks start from a copy of this candidate with a separate known test public key compiled in. The C50 and C51 targets are retained private fixtures from the earlier updater checks, with their original source and method limits. The actual public candidate is separately checked for rejection of those test-key signatures. Their hashes differ from the public-mode development snapshot. Network replies and Qt actions were simulated privately. The two-update fixture advanced only its test processes' realtime clock; it did not edit saved throttle records or change the host clock. No private feed override, synthetic key, QA driver, log, capture, settings or backup is part of the public build.

Daily checks and the release-page link remain available for idle unfinished saved setups, including older copies. Update now stays disabled and the backend refuses installation until the setup is finished or removed. Active operation locks block both checks and installation without consuming the daily attempt. Unverified child exit prevents automatic rollback; the backup and recovery records are retained for manual reopening. Normal display and sleep policy is unchanged.

## Prepublication build identity, subsequently released unchanged

Portable SHA-256: **d17151020ccce00d7fd2e384b33e7ec7c437549552795079a50bf2084a71fd5d**.

Matching Source.zip SHA-256: **1c03e2443d4ae8f8802ad418e443d968836037bc3ec6a6811846a214528b8e7f**.

Source fingerprint: **7f137a4546a3101b799e923927d4335e9bde993cf76c48d18d5feb7e0d721775**.

These hashes identify executable and source bytes later published as Candidate 49. The production signature and all 19 release assets remain the publication-time snapshot. A recovery-test fixture was corrected after CI exposed an inode-reuse assumption. The replacement is now created before the original is replaced. Production recovery code is unchanged, and the corrected full suite passes. Earlier private packaged results retain their original simulation limits.

## Live checks on published Candidate 49

These are device-specific observations using the published executable. Raw captures, logs, profile data and process identifiers remain private. Earlier private-feed checks retain their simulation limits.

| Check | Result and limits |
| --- | --- |
| Bazzite live signed update | PASS within the recorded scope: one disposable older-version copy and one profile used the real GitHub feed and production public key. Not now kept the starter unchanged; ordinary close and same-profile reopening returned the cached notice. Update now installed the published Candidate 49, retained the original starter backup and restarted Candidate 49. The owner pressed both buttons and closed the updated window normally. Automated checks recorded the interface and verified checksum, signature, backup and fresh process/window identities. Numeric GUI exit codes were not established. This is not Bazzite gameplay acceptance. |
| Steam Deck live signed update | The real notice, Not now, first normal starter close, cached same-profile notice, one Update now action, production signature, exact replacement, retained backup and initial restarted Candidate 49 Home were recorded. The owner pressed the two buttons. The updated window and processes later disappeared before a final normal-close action. Cause, exact exit time and numeric exit statuses remain unknown. The final disk check still showed Candidate 49, its completed updater record and backup. This earlier lifetime has no clean final-close pass. |
| Steam Deck separate detached survival check | PASS for sampled survival only: the same window, wrapper/native identities and disposable profile were present at 16 checks about two minutes apart over 1,800.053 seconds. This was a separate lifetime and installed no update. It is not continuous observation between samples and does not explain the earlier disappearance. |
| Steam Deck scheduled close and later close | The scheduled-close observer raised a D-Bus overload-selection TypeError before sending a close request. That tool failure remains recorded separately; it is not a clean scheduled-close pass. A later helper reverified and sent one ordinary close to the same instance without relaunch. The window and both processes exited; wrapper wait returned 0, while the native numeric exit code was not recorded. No forced termination was used. |
| Steam Deck update switch | PASS for one disposable profile across three detached launches: initial visible On; owner click Off and normal close; visible Off after reopening; owner click On and normal close; visible On after the third launch and normal close. Recorded screenshots established the actual visible states; launches, identity checks, saved-record comparisons and normal closes were automated. The owner also performed normal wake/unlock. Only enabled changed during the toggles. No new durable attempt or candidate change appeared while Off in the compared snapshots. All three wrapper waits returned 0; native numeric exit codes were not recorded. |
| C49 Bazzite removal | PASS: three complete installer-owned environment removals through the normal interface, with verified addon and game settings backups of 43, 23 and 22 files. Documented journal rotation pruned three older metadata journals. Those original journals and the existing backup files were independently preserved privately; that separate preservation is not credited to C49 retaining every journal in place. |
| C49 Bazzite fresh install | PASS with limits: published Candidate 49 on an existing Bazzite desktop and Steam account, with previous installer records moved aside. A new Battle.net installation and prefix were created. Battle.net 2.53.8.17986 crashed after the first private sign-in with 0xc0000005; the cause remains unknown. It worked on a plain second launch with unchanged Proton 11 and no workaround. The exact reopening button was not recorded. Classic Era 1.15.9.70576 required a fresh 4.77 GB download and showed 5.30 GB required storage. One local Battle.net tile was retained; Close after Play was configured through the reviewed flow with a verified original-settings backup. The correct game reached its character screen, then exited normally through WoW's own Exit Game. Steam showed the same local tile with green Play. Input was injected, not a physical-controller check. Sampled process observation initially mistook the loss of a named executable mapping for exit; direct checks of the same process lifetime, game argument, prefix data mapping and recorded window corrected that inference. The same game remained alive after main Battle.net exited; Agent was tracked separately. Underlying sampler records remain private. No character was created or world entered. No Steam Deck fresh-install, physical-controller, gameplay or performance pass is claimed. |

Identical saved records do not prove zero network traffic, and last_attempt alone does not establish successful transport. Initial startup while On created its record asynchronously; the earliest absent record was not a saved-preference failure. The switch check did not reset daily history or install an update. The protected real update-history bytes matched the sampled baseline; no full filesystem comparison is claimed. Deck's recorded-interface reviews remain attributed to the producing device; the raw screenshots were not transferred for independent Bazzite inspection. C49 gameplay and physical-controller testing were not repeated. The earlier unexplained closure and original tool failures remain recorded. No further device check is queued; stale partial-download cleanup remains deferred.

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

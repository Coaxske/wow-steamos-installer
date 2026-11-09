# Deckhand Candidate 50 features and limits

- One installer-managed Battle.net button per selected installation. Choose Retail, Classic Era or experimental Forever inside Battle.net.
- Guided setup, missing-button repair, reviewed reuse of an existing setup, and removal with separately verified settings backups.
- Existing launcher, prefix, Proton, settings, game files and controller layouts stay in place when reusing a setup.
- Scoped discovery checks verified locations from Steam shortcuts. Separate copies need separate review.
- Choose existing folder is available when no installation is found. Cancel keeps choices; empty results explain how to try again.
- Existing-folder reuse needs a matching Steam shortcut and working Proton choice. Finding files alone does not authorize adoption or establish Play readiness.
- Required, backed-up Close after Play choice applies to that installation in both Desktop and Gaming Mode.
- World of Warcraft in the Home folder links to detected installation and AddOns folders. Reruns reuse verified links; link removal preserves targets.
- Manual pointer and click guide for the Battle.net button. Working custom or shared controls are preserved.
- The public executable excludes the developer checklist, capture, external mouse control, window reading and addon-manager downloader. Separate private builds can include developer tools.
- No automatic edition routing, extra edition buttons, addon installation or addon updates.
- Published Candidate 48 was tested on an existing Steam Deck setup in Game Mode: Retail and Classic Era automatic main-launcher exit while the same game survived, physical pointer/menu input and normal quit to the same Battle.net button. Desktop discovery, Repair and access-link removal/recreation were checked with recorded limits.
- Forever was personally observed working correctly. Its initial automatic launcher-exit point and interrupted later session are separate evidence, not one uninterrupted instrumented cycle. It remains experimental.
- Published Candidate 49 installed Battle.net and Classic Era from scratch on an existing Bazzite desktop and Steam account, reached the character screen and quit normally. The first post-sign-in Battle.net crash remains unexplained; its plain second launch worked without a workaround. This earlier-build result is not Candidate 50 device acceptance.
- Exact build identities, earlier results and method limits are listed in ACCEPTANCE-SUMMARY.md.

- Installer update checks, explicit Update now consent, checksum and signature verification, and backup/rollback passed offline and simulated private-feed checks. The production public verifier is configured; private checks use separate test keys. Idle unfinished setups permit release checks and manual downloads; automatic installation waits until they are finished or removed. Live-update and device results are detailed in ACCEPTANCE-SUMMARY.md.
- On a Steam Deck that already had Battle.net and WoW set up, Candidate 49 found both. In a live test on that Deck, an older test copy of the installer found Candidate 49 on GitHub. After Update now was pressed, it downloaded and installed the signed update, kept the previous copy as a backup, and restarted as Candidate 49. That updated window later closed unexpectedly before it could be closed normally. The cause is not known, but the update had finished: Candidate 49 and the backup were still in place. In a separate check, Candidate 49 stayed open on the Deck through checks every two minutes for 30 minutes, then closed normally. The update on/off switch also worked on the Deck: after it was turned off, it stayed off when the installer was reopened. The gameplay and controller results above remain Candidate 48 evidence.

- Candidate 50 changes the visible installer name to Deckhand without moving its existing records, settings, games or Proton environment.
- The game-download step includes a first-sign-in retry hint beside Open Battle.net. It does not change Battle.net launch behavior or diagnose the earlier crash.
- Ownership-checked cleanup targets abandoned update temporary files, while retaining active, recovery-referenced, unrecognized or changed files and backups.
- Normal startup retains one previous private startup log. Restarted copies write their own diagnostics; helpers and another window must not erase them.
- Candidate 50 live GitHub updating and Steam Deck acceptance are not yet established. The release review separates automated and private simulated checks from earlier actual-device evidence.

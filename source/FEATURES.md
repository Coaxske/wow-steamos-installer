# Candidate 49 development features and limits

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
- Fresh installation was not part of the device check. Bazzite checks used a read-only scan and simulated interface tests before Apply; Bazzite gameplay testing remains in progress.
- Exact build identities, earlier results and method limits are listed in ACCEPTANCE-SUMMARY.md.

- Installer update checks, explicit Update now consent, checksum and signature verification, and backup/rollback passed offline and simulated private-feed checks. No production key is configured; updates fail closed until reviewed key provisioning. Unfinished saved setups pause updates. Live GitHub transport and C49 Deck checks remain unrun.
- Candidate 49 has no Steam Deck acceptance yet. The earlier existing-installation results above belong to published Candidate 48.

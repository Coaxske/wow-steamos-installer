# WoW SteamOS Installer

**Tested on Steam Deck in Game Mode. Bazzite testing in progress.**

Set up Retail, Classic Era or experimental Forever through one installer-managed Steam tile named **Battle.net**. Open it, choose an installed edition inside Battle.net and press Play. Guided setup configures backed-up automatic main-launcher exit while WoW continues. Quit WoW normally to return to the same tile.

[Download Candidate 44](https://github.com/Coaxske/wow-steamos-installer/releases/tag/v0.2.0-alpha.10-candidate.44) and verify the matching checksum. In Desktop Mode, enable execution in Properties and open the portable. No FUSE is required.

**~/World of Warcraft** provides links to detected games and their actual AddOns folders. Choose **Open folders** on completion; extract a compatible addon's folder into **Retail AddOns**, **Classic Era AddOns** or **Forever AddOns**. Files remain in the existing installation. Link-only removal preserves targets.

Read [install/addon/removal instructions](INSTALL-AND-REMOVE.md), [acceptance evidence](ACCEPTANCE-SUMMARY.md), [limitations](SUPPORTED-EDITIONS-AND-LIMITATIONS.md) and [release notes](RELEASE-NOTES.md). Existing controls are preserved; pointer setup uses the normal manual Steam guide.

C43 actual Steam Deck Era/Retail Game Mode acceptance is inherited by unchanged C44 launch/controller code. C44 folder checks are automated disposable verification, not a new device/game session. **Forever works on Steam Deck as of C44.** Earlier recorded checks were bounded to character selection/creation; there is no separate recorded test of client 1.60.1.70862. No new test or full-gameplay claim is added by this documentation correction.

Installer source is in [source](source/README.md); matching Source.zip, corresponding runtime sources, licenses and notices are release assets. Private device/QA records, captures, logs, settings and backups are excluded. Earlier internal alpha assets/history are historical and are not the recommended download.

# WoW SteamOS Installer

Candidate 49 is in development. Published Candidate 48 was tested on Steam Deck in Game Mode; those results do not test the new updater.

This early test version installs Blizzard's Battle.net app, adds one Battle.net button to Steam, and makes game and AddOns folders easier to find.

## Download and install

You need Steam as it comes on Steam Deck or Bazzite (the Flatpak version of Steam is not supported), an internet connection, and a Blizzard account with access to the game you want to play.

1. On Steam Deck, press the Steam button, choose Power, then Switch to Desktop. On Bazzite, use the desktop.
2. Open Steam and sign in.
3. [Download WoW-SteamOS-Rust-Linux-x86_64](https://github.com/Coaxske/wow-steamos-installer/releases/download/v0.2.0-alpha.10-candidate.48/WoW-SteamOS-Rust-Linux-x86_64). This is the only file you need to run. The other release files contain instructions, checksums, licenses and source code.
4. Keep the file somewhere permanent so you can reopen it for repair or removal.
5. Right-click the file, choose Properties, open the Permissions tab, check Is executable, then click OK.
6. Open it and follow the instructions. Steam may close and reopen during setup.
7. Sign in only inside Battle.net. The installer never asks for your password or authenticator codes.

Setup asks you to approve Battle.net's Close after Play setting. Before changing the setting, it backs up the original Battle.net settings. An already-enabled setting can be confirmed without changing it. This affects the selected installation in both Desktop and Gaming Mode.

## Installer updates in Candidate 49

Candidate 48 users must download Candidate 49 once by hand to get the updater. Candidate 49 is not published yet; the download above still points to published Candidate 48.

The installer can quietly check this project's GitHub releases at most once every 24 hours. The home screen's **Check for installer updates once a day** switch turns these checks off. Offline or failed checks let setup continue. Checks and replacement are paused during setup, repair or removal. An unfinished saved setup also pauses updates until it is completed or resolved through normal recovery.

When a newer version is found, choose **Update now**, **Not now** or **View release page**. Only **Update now** downloads and installs it. Not now leaves the current installer usable. Updates include test releases and never select an older version. If a newer release lacks usable verification files, the notice still appears with the release page; automatic installation stays unavailable.

Before replacement, the updater checks both the published SHA-256 checksum and a digital signature for the exact installer file. The old installer is retained or restored if replacement or restart fails. If automatic restart or the new process's exit cannot be confirmed, recovery files may be retained for manual reopening. If an update cannot be verified, use **View release page** to download it by hand.

This development copy has no production signing key configured. It refuses to install updates until the reviewed public key is included. Temporary signing keys are used only in private automated checks.

The GitHub check requests this repository's public release list. It does not send game settings, account names or installer records. GitHub receives ordinary connection information, such as your IP address and request headers. The check identifies itself as WoW-SteamOS-Updater/1; it does not send this installer's version or device identifiers. See [security and privacy](../SECURITY.md).

## Already have Battle.net or WoW?

The installer checks known locations and verified locations from Steam shortcuts. Separate copies stay separate.

If nothing is found, choose **Choose existing folder** below **Already have Battle.net or WoW installed?** Select the existing folder and review the results. Cancelling the picker keeps your choices unchanged. An empty folder produces instructions to try another location.

Finding a folder does not guarantee it can be reused. If no matching Battle.net Steam shortcut is found, follow the on-screen instructions to add the existing Battle.net app to Steam and choose its working Proton version, then retry. Finding files does not approve adoption, restore lost setup records or prove the game will reach Play.

## Play and controls

On Steam Deck, return to Gaming Mode. Open **Battle.net** under Non-Steam in your Steam library. Choose your installed edition and account, then press Play. With Close after Play enabled, Battle.net closes while WoW stays open.

Quit through WoW's own **Exit Game** menu. Steam's Exit game stops the whole Battle.net game session.

If the mouse pointer or clicking does not work in Battle.net, open **Gaming Mode pointer setup** in the installer for step-by-step help. In short: in Steam's controller settings for the Battle.net button, set Right Trackpad Behavior to Mouse and Right Trackpad Click to Left Mouse Click. If your controls already work, leave them unchanged.

## Addons

After setup, choose **Open folders**, or open **World of Warcraft** in your Home folder. Links appear for detected installations and their existing AddOns folders.

Download an addon for your edition, unzip it, and put its folder into the matching AddOns link. For example:

Retail AddOns/ExampleAddon/ExampleAddon.toc

Make sure you do not end up with a folder inside a folder.

The installer does not download or update addons.

## Repair or remove

Reopen the same installer.

- **Repair Battle.net tile** repairs the existing Steam button without downloading the game again. Ambiguous or conflicting saved identities are refused.
- **Remove access links only** removes verified installer-created links. Games and addons remain; the empty folder may remain too.
- Full installation removal is a separate reviewed choice. Choose **Keep addon and game settings** to back up Interface and WTF. Screenshots, game files and other folders are not included. Copy other files you want to keep before removal.

Read the [install and removal guide](../INSTALL-AND-REMOVE.md) for backup locations and restoration.

## What was checked

Candidate 49 passed offline tests and simulated packaged updater checks on Bazzite, including signed replacement, failed-start rollback, checksum/signature failures, interrupted downloads, unwritable folders, Not now, opt-out and silent offline checks. The network responses and temporary signing keys were private test fixtures. This does not verify live GitHub transport, a production signing key or Steam Deck operation. No Candidate 49 game-session result is claimed. The following are published Candidate 48 existing-installation results carried forward with their limits.

Published Candidate 48 was tested on an existing Steam Deck setup. In Game Mode, Retail and Classic Era reached character screens. Passive observation recorded the main Battle.net process exiting while the same identified game kept running; Agent was tracked separately. The tester used the physical trackpad to move and click in menus, quit through WoW's own Exit Game, and returned to the same Battle.net button with Play available. Reopening that button switched from Retail to Classic Era; this was not a separate same-edition relaunch pair for each game.

In Desktop Mode, automatic discovery found the working installation, so the existing-folder picker was not needed. The reviewed repair kept the existing Battle.net button, launcher, Proton environment and controller files. Open folders reached the expected installation and AddOns links. Link-only removal and recreation preserved the checked targets and contents. These were bounded comparisons, not a full comparison of every game file.

The tester also personally observed Forever working correctly. An initial recorded run showed the main launcher exiting while the same Forever process stayed alive. A later session was interrupted and used a different game process. The firsthand observation and recorded points are separate; they do not establish one uninterrupted instrumented Forever cycle. Forever remains experimental, and full gameplay and performance are not established.

Fresh installation was not part of this device check. The existing-folder picker was checked with simulated interface tests on Bazzite, not exercised on the actual Deck. Bazzite gameplay testing remains in progress.

Earlier Error134 and WOW51900319 remain separate unexplained historical results. No cause or repair is claimed from these later successes.

See [acceptance details](../ACCEPTANCE-SUMMARY.md) and [supported editions and limitations](../SUPPORTED-EDITIONS-AND-LIMITATIONS.md).

## Help

[Report a problem](https://github.com/Coaxske/wow-steamos-installer/issues) with a description and the error message. Do not upload Steam configuration, Battle.net settings, captures or backup folders.

Free and open source under the [MIT License](LICENSE). This community project is not affiliated with Blizzard Entertainment or Valve.

## Building from source

See packaging/README.md. Public builds are the default. Developer tools require the separate explicit developer build switch.

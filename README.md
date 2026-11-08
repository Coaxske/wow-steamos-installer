# WoW SteamOS Installer

Earlier versions were tested on Steam Deck. This version was checked on a Bazzite PC.

This early test version installs Blizzard's Battle.net app, adds one Battle.net button to Steam, and makes game and AddOns folders easier to find.

## Download and install

You need native Steam, an internet connection and a Blizzard account with access to your chosen game.

1. On Steam Deck, switch to Desktop Mode. On Bazzite, use the desktop.
2. Open Steam and sign in.
3. [Download WoW-SteamOS-Rust-Linux-x86_64](https://github.com/Coaxske/wow-steamos-installer/releases/download/v0.2.0-alpha.10-candidate.48/WoW-SteamOS-Rust-Linux-x86_64). This is the only file you need to run. The other release files contain instructions, checksums, licenses and source code.
4. Keep the file somewhere permanent so you can reopen it for repair or removal.
5. Right-click it, open Properties, and allow it to run as a program.
6. Open it and follow the instructions. Steam may close and reopen during setup.
7. Sign in only inside Battle.net. The installer never asks for your password or authenticator codes.

Setup asks you to approve Battle.net's Close after Play setting. Before changing the setting, it backs up the original Battle.net settings. An already-enabled setting can be confirmed without changing it. This affects the selected installation in both Desktop and Gaming Mode.

## Already have Battle.net or WoW?

The installer checks known locations and verified locations from Steam shortcuts. Separate copies stay separate.

If nothing is found, choose **Choose existing folder** below **Already have Battle.net or WoW installed?** Select the existing folder and review the results. Cancelling the picker keeps your choices unchanged. An empty folder produces instructions to try another location.

Finding a folder does not guarantee it can be reused. If no matching Battle.net Steam shortcut is found, follow the on-screen instructions to add the existing Battle.net app to Steam and choose its working Proton version, then retry. Finding files does not approve adoption, restore lost setup records or prove the game will reach Play.

## Play and controls

On Steam Deck, return to Gaming Mode. Open **Battle.net** under Non-Steam in your Steam library. Choose your installed edition and account, then press Play. With Close after Play enabled, Battle.net closes while WoW stays open.

Quit through WoW's own **Exit Game** menu. Steam's Exit game stops the whole Battle.net game session.

Use **Gaming Mode pointer setup** in the installer if mouse movement or clicking does not work. Keep working controls unchanged. Before editing shared controls, save the original layout and use Steam's unique configuration for this device. The guide covers pointer and menu controls, not a complete gameplay layout.

## Addons

After setup, choose **Open folders**, or open **World of Warcraft** in your Home folder. Links appear for detected installations and their existing AddOns folders.

Download an addon for your edition, unzip it, and put its folder into the matching AddOns link. For example:

Retail AddOns/ExampleAddon/ExampleAddon.toc

The installer does not download or update addons.

## Repair or remove

Reopen the same installer.

- **Repair Battle.net tile** repairs the existing Steam button without downloading the game again. Ambiguous or conflicting saved identities are refused.
- **Remove access links only** removes verified installer-created links. Games and addons remain; the empty folder may remain too.
- Full installation removal is a separate reviewed choice. Choose **Keep addon and game settings** to back up Interface and WTF. Screenshots, game files and other folders are not included. Copy other files you want to keep before removal.

Read the [install and removal guide](INSTALL-AND-REMOVE.md) for backup locations and restoration.

## What was checked

Candidate 48 was checked on Bazzite with a read-only scan and simulated interface tests. These checked the folder picker and review screens, stopping before Apply. Candidate 48 has not been run on a Steam Deck or through a new game session.

The Steam Deck results come from earlier versions. Retail and Classic Era reached character screens, Battle.net closed while WoW stayed open, and normal quitting returned to the same Battle.net button. Earlier pointer and menu checks also passed.

Forever works on Steam Deck as of Candidate 44. It remains experimental because recorded checks only reached character selection or creation, and client 1.60.1.70862 has no separate recorded test.

Full gameplay and performance are not established. Earlier Error134 and WOW51900319 remain separate unexplained historical results.

See [acceptance details](ACCEPTANCE-SUMMARY.md) and [supported editions and limitations](SUPPORTED-EDITIONS-AND-LIMITATIONS.md).

## Help

[Report a problem](https://github.com/Coaxske/wow-steamos-installer/issues) with a description and the error message. Do not upload Steam configuration, Battle.net settings, captures or backup folders.

Free and open source under the [MIT License](LICENSE). This community project is not affiliated with Blizzard Entertainment or Valve.

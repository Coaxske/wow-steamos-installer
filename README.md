# Deckhand

Play World of Warcraft on Steam Deck and Bazzite. Deckhand installs Blizzard's Battle.net app, adds one Battle.net button to your Steam library, and makes your game and addon folders easy to find. It is free, and its source code is public. Not affiliated with Blizzard or Valve.

## Deckhand or NonSteamLaunchers?

[NonSteamLaunchers](https://github.com/moraroy/NonSteamLaunchers-On-Steam-Deck) is a free tool that installs many game launchers, including Battle.net, and adds them to Steam. If you want several launchers, it is a good choice.

Deckhand only sets up Battle.net for World of Warcraft. In return, it can:
- repair the Battle.net button in Steam if it breaks
- remove what it set up, with backups
- take over a Battle.net setup you already have
- add shortcuts to your AddOns folders
- close the Battle.net launcher once WoW starts

**This is an early test version.** Updating from Candidate 49 to Candidate 50 was tested on Bazzite and Steam Deck and worked on both. The restarted window stayed open through a 30-minute check and closed normally. Candidate 50 game and controller checks have not been run. Earlier versions were tested on a Steam Deck that already had Battle.net and WoW set up, and with a brand-new installation on a Bazzite PC. Please report problems on the [Issues page](https://github.com/Coaxske/wow-steamos-installer/issues).

## Is it safe?

- You sign in only inside Battle.net. Deckhand never asks for your password or authenticator codes.
- It backs up Battle.net's settings before changing them. It can also remove what it set up, keeping a backup of your addon and game settings.
- It checks for its own updates at most once a day. You can turn that off. An update installs only when you choose Update now, after its digital signature is verified.
- Its update check does not send your game settings, account names or installer records.

## Download and install

You need Steam as it comes on Steam Deck or Bazzite (the Flatpak version of Steam is not supported), an internet connection, and a Blizzard account with access to the game you want to play.

1. On Steam Deck, press the Steam button, choose Power, then Switch to Desktop. On Bazzite, use the desktop.
2. Open Steam and sign in.
3. [Download WoW-SteamOS-Rust-Linux-x86_64](https://github.com/Coaxske/wow-steamos-installer/releases/download/v0.2.0-alpha.10-candidate.50/WoW-SteamOS-Rust-Linux-x86_64). This is the only file you need to run. The other release files contain instructions, checksums, licenses and source code.
4. Keep the file somewhere permanent so you can reopen it for repair or removal.
5. Right-click the file, choose Properties, open the Permissions tab, check Is executable, then click OK.
6. Open it and follow the instructions. Steam may close and reopen during setup.
7. Sign in only inside Battle.net. The installer never asks for your password or authenticator codes.

If Battle.net shows "The application encountered an unexpected error" right after you sign in, close the message and open Battle.net again. In testing, it worked the second time.

Setup asks you to approve Battle.net's Close after Play setting. Before changing the setting, it backs up the original Battle.net settings. An already-enabled setting can be confirmed without changing it. This affects the selected installation in both Desktop and Gaming Mode.

## Installer updates

Candidate 48 users must download the current installer once by hand to get the updater. Candidate 49 already includes update checks.

The installer can quietly check this project's GitHub releases at most once every 24 hours. The home screen's **Check for installer updates once a day** switch turns these checks off. Offline or failed checks let setup continue. Checks and replacement are paused while setup, repair or removal is actively running. An unfinished saved setup can still check for releases, but you must finish or remove it before choosing Update now. View release page stays available.

When a newer version is found, choose **Update now**, **Not now** or **View release page**. Only **Update now** downloads and installs it. Not now leaves the current installer usable. Updates include test releases and never select an older version. If a newer release lacks usable verification files, the notice still appears with the release page; automatic installation stays unavailable.

Before replacement, the updater checks both the published SHA-256 checksum and a digital signature for the exact installer file. After a successful update, the newest verified previous installer is kept. Older backups are removed only when the updater's own records and the files still match. The old installer is retained or restored if replacement or restart fails. If automatic restart or the new process's exit cannot be confirmed, recovery files may be retained for manual reopening. If an update cannot be verified, use **View release page** to download it by hand.

The installer includes the production public key for verifying signed updates. The private signing key stays outside the project. Private automated checks use separate test keys. A live signed update passed on Bazzite. The Steam Deck update and later checks are described below with their limits.

The GitHub check requests this repository's public release list. It does not send game settings, account names or installer records. GitHub receives ordinary connection information, such as your IP address and request headers. The check identifies itself as WoW-SteamOS-Updater/1; it does not send this installer's version or device identifiers. See [security and privacy](SECURITY.md).

## Leftover update files and startup logs

Candidate 50 can remove abandoned temporary update files only when its private records prove that it created them and that their identities still match. Files used by an active update or unresolved recovery, backups and unrelated or changed files stay in place. Older leftovers without trustworthy ownership records may remain.

Normal startup attempts to keep the previous private diagnostic in startup.previous.log before writing startup.log. Logging is optional, so a logging error does not by itself stop startup. If an existing window lock looks live or uncertain, the logs stay untouched and the normal window check decides whether another installer can open. A genuinely held startup lock still refuses another launcher. Helpers leave these logs alone. Keep both logs private. They may help investigate an unexpected exit, but they do not guarantee its cause can be found.

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

Read the [install and removal guide](INSTALL-AND-REMOVE.md) for backup locations and restoration.

## What was checked

Candidate 50 checks cover its name, setup hint, guarded update-file cleanup and startup logs using isolated profiles and private fixtures on the Bazzite build machine. Network replies, interface actions and failures were simulated; packaged signature checks, file replacement, restart and rollback used real programs with separate test keys. Updating from Candidate 49 to Candidate 50 was tested on Bazzite and Steam Deck and worked on both. The restarted window stayed open through a 30-minute check and closed normally. Candidate 50 game and controller checks have not been run.

The following device results belong to earlier versions:

- Steam Deck with Battle.net and WoW already set up: checked in Game Mode and Desktop Mode (Candidate 48).
- Brand-new installation: worked on a Bazzite PC (Candidate 49). Battle.net and Classic Era installed from scratch, and the game reached the character screen. Battle.net crashed the first time after sign-in and worked when opened again. Not yet tested on Steam Deck.
- Updates from GitHub: installed correctly on Bazzite and Steam Deck (Candidate 49). On the Deck, the updated window later closed unexpectedly; the cause is not known.

On a Steam Deck that already had Battle.net and WoW set up, Candidate 49 found both. In a live test on that Deck, an older test copy of the installer found Candidate 49 on GitHub. After Update now was pressed, it downloaded and installed the signed update, kept the previous copy as a backup, and restarted as Candidate 49. That updated window later closed unexpectedly before it could be closed normally. The cause is not known, but the update had finished: Candidate 49 and the backup were still in place. In a separate check, Candidate 49 stayed open on the Deck through checks every two minutes for 30 minutes, then closed normally. The update on/off switch also worked on the Deck: after it was turned off, it stayed off when the installer was reopened.

Candidate 49 also passed offline and private simulated updater checks on the Bazzite build machine. These are separate from live GitHub updating. Those updater checks did not exercise game sessions or physical controllers.

The following results are from Candidate 48:

Candidate 48 was tested on a Steam Deck that already had Battle.net and World of Warcraft set up.

- **Game Mode:** Retail and Classic Era each started from the Battle.net button and reached the character screen. Battle.net closed itself while the game kept running. The trackpad worked as a mouse in menus, and quitting through the game's own Exit Game option returned to the same Battle.net button, ready to play again.
- **Desktop Mode:** the installer found the existing installation without needing a folder to be chosen. Repair kept the existing Battle.net button, the Battle.net app, its Proton setup and the controller settings files. Open folders worked, and removing and re-creating the folder shortcuts left the checked game and addon files in place.
- **Forever:** the tester saw Forever working, but the recorded test was interrupted, so Forever is still marked experimental.

Not tested in that Candidate 48 Deck session: a brand-new installation, the Choose existing folder option (it was checked with simulated tests on Bazzite), full gameplay and performance. Bazzite testing is in progress. Error134 and WOW51900319, seen in earlier testing, are still unexplained.

See [acceptance details](ACCEPTANCE-SUMMARY.md) for the exact test records and their limits, and [supported editions and limits](SUPPORTED-EDITIONS-AND-LIMITATIONS.md) for what the installer covers.

## Help

[Report a problem](https://github.com/Coaxske/wow-steamos-installer/issues) with a description and the error message. Do not upload Steam configuration, Battle.net settings, captures or backup folders.

Free and open source under the [MIT License](LICENSE). This community project is not affiliated with Blizzard Entertainment or Valve.

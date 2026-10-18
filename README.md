# WoW SteamOS Installer

**World of Warcraft installer for Steam Deck, SteamOS, and native-Steam Linux desktops.**

Choose the World of Warcraft editions you want, install them through Battle.net, and give each installed client its own Steam shortcut.

**Version: 0.1.0-alpha.2 — hardware testing required.** Automated tests cover file handling and a simulated install flow. This release has not yet been validated end to end on a physical Steam Deck or Bazzite computer. It is an early community testing release, not a claim that every WoW client currently works with Proton.

Designed for **Steam Deck / SteamOS and x86-64 Linux desktops such as Bazzite using native Steam**. Python 3.10+ and Steam are required. No pip packages, administrator password, Decky, Lutris, or addon manager are required. Flatpak Steam is not supported by this release.

## What it does

- Lets you select one or several editions during setup.
- Offers Retail, Forever, Classic Era (including available Hardcore/Season of Discovery realms), Classic Progression, Anniversary, PTRs, betas, and an **Other / future client** option.
- Downloads the Battle.net Windows installer directly from Blizzard, or accepts an installer you downloaded yourself.
- Creates a shared Proton environment and a Battle.net Steam tile.
- Guides you through choosing and installing editions **inside Battle.net**. It does not select those dropdowns or download game assets itself.
- Finds actual installed WoW executables and asks you to confirm each selection before creating direct game tiles.
- Uses the same Proton environment and compatibility tool for all of its tiles.
- Keeps existing tile IDs on reruns, preserving their association with controller layouts and artwork.
- Backs up Steam files, detects external edits, and provides conditional recovery after interrupted writes.

You authenticate only in Battle.net or WoW. The project has no telemetry, account server, game assets, Blizzard executables, or credentials in its source distribution.

## Quick start

### Single-file launcher (recommended)

1. Switch your Steam Deck to **Desktop Mode**. Open native Steam and sign in once.
2. Open this repository's **Releases** page and download **`WoW-SteamOS-Installer.desktop`** from Assets. This is the complete launcher; you do not need the source ZIP alongside it.
3. In Dolphin, right-click the downloaded file → **Properties → Permissions → Is executable** if needed. Double-click it and approve KDE's **Execute / Trust and Launch** prompt if shown.
4. A terminal opens with the setup wizard. Choose your WoW editions, then follow its Battle.net installation and Steam-exit prompts. Keep the launcher to rerun setup later.

The launcher contains the exact versioned Python application; it does not fetch a changing setup script. Python 3.10+ is still required. A `.desktop` file is a Linux executable launcher, not a Windows `.exe` or an AppImage. The wizard is text-based.

If desktop launching is unavailable, download `WoW-SteamOS-Installer.pyz` from the same release and run `python3 WoW-SteamOS-Installer.pyz` in a terminal. It also accepts commands such as `plan --editions retail,era` or `register --editions forever`.

### Source folder and walkthrough

1. In **Desktop Mode**, open native Steam and sign in once. Install pending Steam updates.
2. Download this repository using **Code → Download ZIP**, or download a source ZIP attached to a release. Extract the entire folder.
3. Keep all extracted files together. Open the folder in a terminal and run:

   ```bash
   bash start.sh
   ```

   Alternatively, launch `Run-From-Source.desktop` in that extracted folder. Keep the source files together. KDE may ask you to allow execution.

4. Select your WoW editions by number, for example `1,2,3` for Retail, Forever, and Classic Era. If several Steam accounts are present, select the account directory you use.
5. Choose one Proton tool. **Proton Experimental** is the default suggestion. Proton 10.0 and a custom installed compatibility-tool ID are also offered. Steam downloads its built-in tool as needed; install a custom tool separately first.
6. Review the plan. The wizard pauses whenever Steam files need to change. Exit WoW and Battle.net, then use **Steam → Exit**; closing the Steam window alone is insufficient.
7. The Battle.net installer tile opens through Steam. Install Battle.net in its **default** location, sign in, then exit Battle.net and Steam when the wizard asks.
8. The wizard updates that same tile to the installed Battle.net launcher. Open it to install the selected WoW editions. The wizard prints a Windows-style `Z:\...` path for the recommended game-download parent.
9. In Battle.net's version dropdown, select each desired edition and finish its download. Subscriptions, purchases, invitations, server availability, and region restrictions still apply.
10. Exit Battle.net and Steam. Confirm each game's actual executable when the wizard lists candidates, or paste a path. Skip unavailable editions and add them later.
11. Open Steam. Your entries appear under **Non-Steam**. Check Properties → Compatibility on the Battle.net tile and a game tile: both should use the selected Proton tool.
12. Test each client, then configure Steam Input separately for each game tile. A mouse layout is useful for Battle.net. Controller addons and in-game bindings are outside this installer’s scope.

The wizard is text-based, so a keyboard or Steam's on-screen keyboard helps. It deliberately asks before applying a concrete list of shortcuts. `q` cancels at any prompt. Rerun setup after a normal interruption; use recovery if the tool reports an incomplete transaction.

## Which “versions” are supported?

The catalog is a set of **installation choices and discovery hints**, not a promise that every client is available, that a particular executable name remains unchanged, or that authentication works without Battle.net.

| Choice | How it is handled |
| --- | --- |
| Retail | Finds installed retail client(s), usually under `_retail_`. |
| Forever | Looks for known Forever/beta folders and asks for confirmation. October 2026 beta reports identify `_classic_beta_/WowB.exe`; release paths may differ. |
| Classic Era | One client tile. Choose an available Era, Hardcore, or Season of Discovery realm in the game. |
| Classic Progression | Finds `_classic_`; the expansion name changes over time. |
| Anniversary | Offers known possible dedicated-folder hints and explicit path selection. Some offerings share a client; the wizard avoids adding the same executable twice. |
| PTR / beta | Requires an installed client and access. Blizzard can reuse folder/product identifiers between tests. |
| Other / future client | Give it a label and choose the actual installed `Wow*.exe` client. Can be used repeatedly for multiple additional clients. |

**A folder name alone does not identify the game.** Confirm the corresponding Battle.net selection. For example, `_classic_beta_` has hosted different tests, and can appear as a suggestion for both Forever and a Classic beta.

The tool registers game executables, not `Launcher.exe` stubs. An unfamiliar non-`Wow*.exe` name will be refused; a maintainer can extend the discovery rule after checking it.

## Playing and updating

- Launch a game through its own Steam tile. Direct launch can require entering credentials and an authenticator code in the game.
- If a particular client/build requires a launcher-authenticated session, use **Battle.net [WoW SteamOS Installer]**, select the desired version, and launch from there. Direct login is not guaranteed for every edition.
- For updates: exit the game, open Battle.net, let updates finish, exit Battle.net, then launch the game tile.
- Do not stop Battle.net's Steam session while a WoW process in the same environment is running.
- This release does not automate Battle.net's window settings, updates, addons, controller configuration, or background services. Those can be added as separately reviewed features.

## Add another edition later

```bash
bash start.sh setup --editions retail,era
```

If it is already installed and you only need a tile:

```bash
bash start.sh register --editions ptr
```

For an arbitrary installed client:

```bash
bash start.sh register --editions other
```

Previously registered, unselected editions are retained. Selecting a subset never removes other games. The same executable is registered only once; realm modes do not require duplicate downloads.

## Preview, inspect, remove

```bash
bash start.sh list-editions
bash start.sh plan --editions forever,retail,era
bash start.sh status
bash start.sh remove-shortcuts
bash start.sh recover
```

`plan` performs no downloads, directory creation, or Steam edits. `status` reads local setup state and checks target existence; it does not certify a working installation.

`remove-shortcuts` removes only owned Steam entries and their compatibility mappings after checking for conflicts. **It does not delete games, the prefix, addons, settings, the setup state, or backups.** `recover` is for an interrupted transaction, not a generic restore-to-an-old-date command. See [Recovery](docs/RECOVERY.md).

## Storage and existing installations

Default layout:

The data-folder name remains `wow-on-steam` for compatibility with the earlier alpha. Renaming the project does not move your games, prefix, state, or existing shortcuts.

```text
~/Games/wow-on-steam/prefix/       Shared Proton environment and Battle.net
~/Games/wow-on-steam/games/        Recommended parent for game downloads
~/Games/wow-on-steam/downloads/    Official installer downloaded at setup
~/Games/wow-on-steam/state.json    Owned tile IDs, target paths and Proton choice
~/Games/wow-on-steam/backups/      Pre-edit Steam files and transaction manifests
```

Battle.net controls the actual installation path. If you accept its C: default instead of the printed Z: location, your games can still end up inside the prefix; the scanner checks that location too. **Never delete the prefix assuming the game is somewhere else.**

This tool does not silently adopt or overwrite older scripts' shortcuts. For an existing Battle.net installation, use its known compatdata root explicitly on the first run and select its existing Proton tool:

```bash
bash start.sh setup --prefix '/absolute/path/to/compatdata/1234567890' --proton proton_10
```

Do not append `/pfx` to `--prefix`. Existing tiles are left alone, so sharing a prefix requires keeping their Proton version consistent manually. Locate already-downloaded games in Battle.net or supply their executable paths; reinstalling is not required. Review new tiles before retiring your old ones.

`--root`, `--steam-root`, `--account`, `--games`, and `--installer` are available; see `--help`. Storage overrides apply on first setup; in-place prefix or Proton migrations are intentionally refused. SD cards and non-Linux filesystems have not been validated; use internal Linux storage for initial testing. Paths containing shell-sensitive characters are rejected with an explanation.

## Testing and contribution

```bash
python3 -m unittest discover -s tests -v
python3 -m compileall -q wowsteam wow_on_steam.py
```

Tests use temporary Steam fixtures and fake PE files, not actual Blizzard binaries or your real Steam files. See [testing](docs/TESTING.md), [contributing](CONTRIBUTING.md), and [architecture](docs/ARCHITECTURE.md). The included GitHub Actions workflow is configured to run tests across Python 3.10–3.13 after publication. A release workflow packages version tags as **prereleases** with SHA-256 checksums.

The MIT license allows use, modification, redistribution, and forks with its notice retained. WoW, Battle.net, Steam, Steam Deck, and other names belong to their respective owners; this is an independent community project and includes no Blizzard or Valve artwork.

## References

- [Blizzard: installing World of Warcraft](https://us.support.blizzard.com/en/article/195680)
- [Blizzard: Forever beta installation and access](https://news.blizzard.com/en-us/article/24304160/the-world-of-warcraft-forever-beta-now-live)
- [Valve Proton](https://github.com/ValveSoftware/Proton)
- [Valve discussion: shared Proton environments](https://github.com/ValveSoftware/Proton/issues/8486)
- [Valve discussion: Steam compatibility-data location](https://github.com/ValveSoftware/steam-for-linux/issues/7158)

See [publishing](docs/PUBLISHING.md) for creating your own public GitHub repository and release.

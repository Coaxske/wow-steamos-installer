# Recovery without deleting games

## First response

Exit WoW, exit Battle.net, then use Steam > Exit in Desktop Mode. Closing a window is not always enough. If a process remains and you cannot close it normally, restart the computer and return to Desktop Mode.

Rerun setup for a normal cancellation. The setup records ownership after each completed Steam transaction. It can replace the installer target with the installed Battle.net executable and register additional games without recreating existing owned IDs.

## Interrupted Steam-file write

The tool writes `pending.json` before changing `shortcuts.vdf`, `config.vdf`, and its own `state.json`. If it reports an interrupted transaction:

```bash
bash start.sh recover
```

Use the same `--root`, `--steam-root`, and `--account` overrides if applicable. Recovery restores pre-edit bytes only when every affected file matches either its recorded before or after hash. If a file was edited later by Steam or another tool, recovery stops rather than discarding that later work.

Each backup directory includes a private `manifest.json` mapping numbered `.bin` files to their original locations and SHA-256 hashes. Do not upload these files publicly. If automatic recovery stops, compare the relevant backup and current file locally or seek help using a sanitized description. Do not restore an entire old Steam configuration over newer changes blindly.

## User edits conflict with managed launch settings

Names and artwork may be changed in Steam. The tool manages only the executable, working directory, launch options, and Proton selection. If you edit those outside the tool, it refuses to overwrite them. Compare `state.json`'s saved `signature` values with Properties > Shortcut, and restore the managed values if you want the tool to continue managing that entry. Set the same Proton tool on every shared-prefix entry.

If you prefer manual ownership, retain your working shortcuts and stop using the tool for those entries. There is no force flag which bypasses conflicts.

## Remove this tool's shortcuts

```bash
bash start.sh remove-shortcuts
```

Game data, settings, prefix, and backups remain. It never recursively deletes a game or Proton directory. The old setup script's tiles are not owned by this tool and will remain.

## Missing or moved game

Use Battle.net's Locate Game function to find the correct installation, then rerun `register` and supply the existing game's actual executable. If only its path changed, this updates the owned tile without changing its ID. Do not move the shared Proton environment or change the setup root casually: those paths are also stored in Steam launch options.

## Never infer where a game lives

For each edition, use Battle.net's Show in Explorer or game settings to confirm its actual location. A games folder outside the prefix does not prove every version was installed there. Preserve each version's `Interface` and `WTF` folders before any manual uninstall. This tool offers no automatic prefix-deletion operation.

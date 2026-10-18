# Architecture

`wow_on_steam.py` is the entry point. `start.sh` and the desktop launcher resolve their own directory, so downloaded source can run without installation into system folders.

Release packaging also produces a standalone Python zipapp and a `.desktop` launcher with the zipapp embedded. The desktop launcher verifies its embedded bytes, runs them from a private temporary directory, then removes that temporary copy. Game and setup state use the same persistent data root as source-folder runs. The catalog is loaded with `importlib.resources` so it also works inside the zipapp. The embedded hash detects corruption; it is not a publisher signature.

- `catalog.json`: edition labels, explanatory hints, and possible folder names.
- `cli.py`: interactive choices and two-stage Battle.net bootstrap, then executable registration. It never collects passwords or automates clicks in a signed-in launcher.
- `system.py`: bounded file discovery, Windows PE signature checks, native Steam discovery, process checks, path validation, HTTPS installer download.
- `vdf.py`: ordered binary VDF reader/writer, text VDF parser, and narrow compatibility-mapping edits. The text replacement uses token spans so compact VDF never loses neighboring fields.
- `manager.py`: owned app IDs, stable updates, backups, journaled writes, conditional rollback, and managed-shortcut removal.

## Steam ownership

The first creation chooses an unused high-bit non-Steam app ID. Later updates use the recorded ID, not a title match. Another shortcut with the same title is not adopted. Existing owned names, icon metadata, tags, and IDs remain. The project does not write controller layout files or download artwork.

All its Windows tiles receive `STEAM_COMPAT_DATA_PATH="..." %command%` and one selected `CompatToolMapping`. The Proton prefix is the parent containing `pfx`, not `pfx` itself. The initial Battle.net installer tile is reused for the installed launcher.

Game tiles target the installed client executable. There is no hard-coded Battle.net product launch command and no promise of launcher-free login on every build. A shared Battle.net tile provides update and authentication fallback.

## Transactions

1. Require Steam, WoW, and Battle.net/Agent to be closed.
2. Read the original files; reject malformed input, unknown binary types, or conflicting owned settings.
3. Produce and reparse proposed contents before any write.
4. Verify originals have not changed; save original bytes and hashes in a unique private backup directory.
5. Durably write a pending journal.
6. Recheck running processes and original bytes before each atomic file replacement.
7. Remove the pending journal only after all writes complete.

A setup lock prevents concurrent instances of this tool. The pending journal handles partial multi-file writes; it is not a transaction inside the Steam client. Users must keep Steam closed during the edit. Recovery never overwrites a file whose current bytes differ from both expected states.

## Extension boundaries

Folder hints are intentionally editable. Candidate discovery is depth-bounded and skips `Data`, `Cache`, `Interface`, `WTF`, logs, errors, and symlinked subtrees. Exact executable paths remain available when hints fail. Duplicate executable selection is collapsed into one tile.

Version-specific launcher APIs, addon managers, Flatpak Steam permissions, boot services, controller defaults, and automatic updates are separate future features. They should not be implemented by silently adding unrelated privileged installers.

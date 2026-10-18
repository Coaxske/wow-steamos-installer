"""Owned shortcut changes, durable backups, interrupted-write recovery."""
from __future__ import annotations
import copy
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import re
import uuid
from . import vdf
from .system import SetupError, atomic_write, require_idle, safe_path

FIELDS = ('Exe', 'StartDir', 'LaunchOptions')
SCHEMA = 1


def json_bytes(obj):
    return (json.dumps(obj, indent=2, ensure_ascii=True) + '\n').encode()


def digest(data):
    return None if data is None else hashlib.sha256(data).hexdigest()


def read_bytes(path):
    if path.is_symlink():
        raise SetupError(f'Refusing a symlink at a managed file: {path}')
    return path.read_bytes() if path.exists() else None


def signature(fields):
    return {name: (vdf.kv_get(fields, name) or [None, None, None])[2] for name in FIELDS}


def quoted(path):
    return '"' + str(safe_path(path)) + '"'


class Manager:
    def __init__(self, root, steam, account):
        self.root = safe_path(root)
        self.steam = safe_path(steam)
        if not str(account).isdecimal() or str(account) == '0':
            raise SetupError('Invalid Steam account directory.')
        self.account = str(account)
        self.state_file = self.root / 'state.json'
        self.shortcuts_file = self.steam / 'userdata' / self.account / 'config/shortcuts.vdf'
        self.config_file = self.steam / 'config/config.vdf'
        self.pending = self.root / 'pending.json'
        self.files = (self.shortcuts_file, self.config_file, self.state_file)

    def load(self):
        raw = read_bytes(self.state_file)
        if raw is None:
            return {'schema': SCHEMA, 'steam': str(self.steam), 'account': self.account,
                    'prefix': str(self.root / 'prefix'), 'games': str(self.root / 'games'),
                    'proton': 'proton_experimental', 'tiles': {}}
        try:
            state = json.loads(raw)
            if state['schema'] != SCHEMA or state['steam'] != str(self.steam) or state['account'] != self.account or not isinstance(state['tiles'], dict):
                raise ValueError('State belongs to another Steam account, root, or schema.')
            safe_path(state['prefix'])
            safe_path(state['games'])
            if not re.fullmatch(r'[A-Za-z0-9_.-]+', state['proton']):
                raise ValueError('Invalid Proton tool name.')
            return state
        except (ValueError, KeyError, TypeError) as e:
            raise SetupError(f'Invalid state file; refusing to overwrite it: {e}') from e

    def check_pending(self):
        if self.pending.exists():
            raise SetupError('An interrupted transaction exists. Run the recover command before continuing. Do not delete state or backups.')

    def commit(self, outputs, originals):
        require_idle()
        self.check_pending()
        if set(outputs) != set(self.files) or set(originals) != set(self.files):
            raise SetupError('Invalid transaction file set.')
        for path in self.files:
            if read_bytes(path) != originals[path]:
                raise SetupError(f'{path.name} changed during setup; no changes applied. Close Steam and retry.')
        if all(outputs[p] == originals[p] for p in self.files):
            return None
        stamp = datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%SZ') + '-' + uuid.uuid4().hex[:8]
        backup = self.root / 'backups' / stamp
        backup.mkdir(parents=True, mode=0o700)
        entries = []
        for i, p in enumerate(self.files):
            before = originals[p]
            if before is not None:
                atomic_write(backup / f'{i}.bin', before)
            entries.append({'path': str(p), 'backup': str(backup / f'{i}.bin') if before is not None else None,
                            'before': digest(before), 'after': digest(outputs[p])})
        journal = {'schema': SCHEMA, 'entries': entries}
        atomic_write(backup / 'manifest.json', json_bytes(journal))
        atomic_write(self.pending, json_bytes(journal))
        # The journal is durable before any changed Steam files are written.
        # Any partial write leaves pending.json for conditional recovery.
        for p in self.files:
            require_idle()
            if read_bytes(p) != originals[p]:
                raise SetupError('A managed file changed during commit. Run recover; do not overwrite it manually.')
            atomic_write(p, outputs[p])
        self.pending.unlink()
        return backup

    def recover(self):
        require_idle()
        if not self.pending.exists():
            return False
        journal = json.loads(self.pending.read_text())
        entries = journal.get('entries', [])
        if journal.get('schema') != SCHEMA or [e.get('path') for e in entries] != [str(p) for p in self.files]:
            raise SetupError('Recovery journal has an unexpected file set.')
        restorations = []
        for e, p in zip(entries, self.files):
            current = read_bytes(p)
            if digest(current) not in (e['before'], e['after']):
                raise SetupError(f'{p.name} changed after the interrupted operation. Automatic recovery would overwrite later edits, so it stopped. Compare backups manually.')
            data = None
            if e['backup']:
                b = Path(e['backup']).resolve()
                if not b.is_relative_to((self.root / 'backups').resolve()):
                    raise SetupError('Recovery backup points outside the backup directory.')
                data = b.read_bytes()
            if digest(data) != e['before']:
                raise SetupError('Recovery backup hash mismatch.')
            restorations.append((p, current, data))
        for p, current, data in restorations:
            require_idle()
            if read_bytes(p) != current:
                raise SetupError('File changed during recovery. Close Steam and retry.')
            if data is None:
                p.unlink(missing_ok=True)
            else:
                atomic_write(p, data)
        self.pending.unlink()
        return True

    def update(self, specs, settings=None, remove=False):
        """Only alter app IDs owned by this installation's state, never by name."""
        require_idle()
        self.check_pending()
        originals = {p: read_bytes(p) for p in self.files}
        state = self.load()
        if settings:
            for key in ('prefix', 'games', 'proton'):
                if key in settings and state['tiles'] and settings[key] != state[key]:
                    raise SetupError(f'Changing {key} after setup is not supported. Keep the existing shared environment.')
            state.update(settings)
        if not re.fullmatch(r'[A-Za-z0-9_.-]+', state['proton']):
            raise SetupError('Invalid Proton tool identifier.')
        safe_path(state['prefix'])
        safe_path(state['games'])
        shortcuts = vdf.Shortcuts(self.shortcuts_file)
        if originals[self.config_file] is None:
            raise SetupError('Steam config.vdf is missing; start Steam and sign in first.')
        config = originals[self.config_file].decode('utf-8', 'surrogateescape')
        vdf.vdf_parse_tree(config)  # fail before the first write on malformed config
        # Preflight every owned entry, including ones not selected this time.
        for key, record in state['tiles'].items():
            fields = shortcuts.find_by_appid(record['appid'])
            if fields is not None and signature(fields) != record['signature']:
                raise SetupError(f'The {record["name"]} launch settings were edited outside this tool. Restore those three fields from state.json before rerunning; nothing was overwritten.')
            tool = vdf.get_compat_tool(config, str(record['appid']))
            if tool not in (None, state['proton']):
                raise SetupError('A managed tile has a different Proton version. Restore the shared version in Steam first.')
        if remove:
            for record in state['tiles'].values():
                shortcuts.remove(record['appid'])
                config = vdf.remove_compat_tool(config, str(record['appid']))
            state['tiles'] = {}
        else:
            for key, name, exe in specs:
                exe = safe_path(exe)
                if not exe.is_file():
                    raise SetupError(f'Executable is missing: {exe}')
                record = state['tiles'].get(key)
                fields = shortcuts.find_by_appid(record['appid']) if record else None
                if fields is None:
                    taken = shortcuts.appids() | {int(x.key) for x in (vdf.find_path(vdf.vdf_parse_tree(config), vdf.STEAM_SECTION + ['CompatToolMapping']) or vdf.VNode('',0,0)).children if x.key.isdecimal()}
                    # Reuse an owned, removed tile's ID unless a mapping now occupies it.
                    if record:
                        appid = record['appid']
                    else:
                        appid = vdf.new_shortcut_appid(quoted(exe), name, taken)
                    fields = vdf.make_shortcut_fields(appid, name, '', '', '')
                    shortcuts.entries.append([vdf.T_MAP, str(len(shortcuts.entries)), fields])
                else:
                    appid = record['appid']
                launch = 'STEAM_COMPAT_DATA_PATH=' + quoted(state['prefix']) + ' %command%'
                for field, value in zip(FIELDS, (quoted(exe), quoted(exe.parent), launch)):
                    vdf.kv_set(fields, vdf.T_STR, field, value)
                # Existing name, artwork, tags, last-played data and app ID survive.
                state['tiles'][key] = {'appid': appid, 'name': (vdf.kv_get(fields, 'AppName'))[2], 'signature': signature(fields), 'exe': str(exe)}
                config = vdf.set_compat_tool(config, str(appid), state['proton'])
        for i, entry in enumerate(shortcuts.entries):
            entry[1] = str(i)
        new_shortcuts = vdf.bvdf_dumps(shortcuts.root)
        vdf.bvdf_loads(new_shortcuts)
        vdf.vdf_parse_tree(config)
        return self.commit({self.shortcuts_file: new_shortcuts,
                            self.config_file: config.encode('utf-8', 'surrogateescape'),
                            self.state_file: json_bytes(state)}, originals)

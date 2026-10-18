"""Host checks, official installer transfer, and bounded game discovery."""
from __future__ import annotations
from importlib.resources import files
import hashlib
import json
import os
from pathlib import Path
import re
import struct
import tempfile
import urllib.request
from urllib.parse import urlsplit

INSTALLER_URL = "https://www.battle.net/download/getInstallerForGame?os=win&gameProgram=BATTLENET_APP&version=Live"
MAX_DOWNLOAD = 200 * 1024 * 1024

class SetupError(Exception):
    """An actionable failure which does not justify deleting user data."""


def safe_path(value):
    p = Path(value).expanduser().absolute().resolve()
    # Steam substitutes %command%, and shell parsing varies by client build.
    # Reject metacharacters instead of guessing at nested escaping semantics.
    if any(c in str(p) for c in '\n\r\x00"`$%\\'):
        raise SetupError('Path contains unsupported characters (quotes, $, %, backticks, backslash, or a newline). Choose a simpler folder name.')
    return p


def atomic_write(path, data):
    path = Path(path)
    if path.is_symlink():
        raise SetupError(f'Refusing to replace a symlink: {path}')
    path.parent.mkdir(parents=True, exist_ok=True)
    mode = path.stat().st_mode & 0o777 if path.exists() else 0o600
    fd, name = tempfile.mkstemp(prefix='.' + path.name + '-', dir=path.parent)
    try:
        with os.fdopen(fd, 'wb') as f:
            f.write(data)
            f.flush()
            os.fsync(f.fileno())
        os.chmod(name, mode)
        os.replace(name, path)
        dfd = os.open(path.parent, os.O_RDONLY)
        try:
            os.fsync(dfd)
        finally:
            os.close(dfd)
    finally:
        if os.path.exists(name):
            os.unlink(name)


def catalog():
    return json.loads(files('wowsteam').joinpath('catalog.json').read_text(encoding='utf-8'))


def parse_selection(value):
    items = catalog()
    if value.strip().lower() == 'all':
        return [x for x in items if x['id'] != 'other']
    chosen = []
    for token in re.split(r'[,\s]+', value.strip()):
        item = next((x for i, x in enumerate(items, 1) if token in (str(i), x['id'])), None)
        if item is None:
            raise SetupError(f'Unknown edition: {token!r}. Use the listed numbers or IDs.')
        if item not in chosen:
            chosen.append(item)
    return chosen


def native_steam(home):
    for p in (home / '.local/share/Steam', home / '.steam/steam', home / '.steam/root'):
        if (p / 'config/config.vdf').is_file():
            return p.resolve()
    raise SetupError('Native Steam was not found. Open native Steam and sign in once. Flatpak Steam is not supported by this first release.')


def accounts(steam):
    base = steam / 'userdata'
    return sorted(p.name for p in base.iterdir() if p.is_dir() and p.name.isdecimal() and p.name != '0') if base.exists() else []


def busy_processes(proc=Path('/proc')):
    found = set()
    for p in proc.iterdir():
        if not p.name.isdecimal():
            continue
        try:
            if p.stat().st_uid != os.getuid():
                continue
            comm = (p / 'comm').read_text().strip().lower()
            args = (p / 'cmdline').read_bytes().decode('utf-8', 'replace').split('\0')
        except (FileNotFoundError, ProcessLookupError):
            continue
        except PermissionError as e:
            raise SetupError('Cannot inspect your processes. Refusing to edit Steam without a reliable running-process check.') from e
        if comm in {'steam', 'steamwebhelper', 'steam.exe'}:
            found.add('Steam')
        # Wine may put the Windows image after its own executable; check all arguments.
        for arg in args:
            name = arg.replace('\\', '/').rsplit('/', 1)[-1].lower()
            if name in {'battle.net.exe', 'battle.net launcher.exe', 'agent.exe', 'agenthelper.exe', 'battle.net-setup.exe'}:
                found.add('Battle.net / Agent')
            if re.fullmatch(r'wow(?:classic)?[a-z0-9_-]*\.exe', name) and 'launcher' not in name:
                found.add('WoW')
    return sorted(found)


def require_idle():
    names = busy_processes()
    if names:
        raise SetupError('Close these applications completely before editing Steam: ' + ', '.join(names) + '. Use Steam > Exit in Desktop Mode. If a process lingers, restart the computer; do not stop Battle.net while WoW is running.')


def pe_file(path):
    """Validate a bounded PE signature, not the publisher or authenticity."""
    try:
        with Path(path).open('rb') as f:
            header = f.read(64)
            if len(header) != 64 or header[:2] != b'MZ':
                return False
            offset = struct.unpack_from('<I', header, 60)[0]
            if offset < 64 or offset > 16 * 1024 * 1024:
                return False
            f.seek(offset)
            return f.read(4) == b'PE\0\0'
    except OSError:
        return False


def game_exe(path):
    p = Path(path)
    return bool(re.fullmatch(r'wow(?:classic)?[a-z0-9_-]*\.exe', p.name, re.I)) and 'launcher' not in p.name.lower() and pe_file(p)


def discover(roots, folder_hints):
    """Walk only explicit game roots, skipping large payloads and all symlinks."""
    matches = []
    seen = set()
    for root in roots:
        root = Path(root)
        if not root.is_dir():
            continue
        for directory, dirs, files in os.walk(root, followlinks=False):
            here = Path(directory)
            depth = len(here.relative_to(root).parts)
            dirs[:] = [d for d in dirs if d.lower() not in {'data', 'cache', 'wtf', 'interface', 'logs', 'errors'} and not (here / d).is_symlink()] if depth < 4 else []
            for filename in files:
                candidate = here / filename
                if candidate.is_symlink() or (folder_hints and here.name not in folder_hints):
                    continue
                if candidate not in seen and game_exe(candidate):
                    seen.add(candidate)
                    matches.append(candidate)
    return sorted(matches)


def bnet_exe(prefix):
    for base in ('Program Files (x86)', 'Program Files'):
        for filename in ('Battle.net Launcher.exe', 'Battle.net.exe'):
            p = prefix / 'pfx/drive_c' / base / 'Battle.net' / filename
            if pe_file(p):
                return p
    return None


def allowed_download(url):
    parsed = urlsplit(url)
    host = (parsed.hostname or '').lower()
    return parsed.scheme == 'https' and any(host == d or host.endswith('.' + d) for d in ('battle.net', 'blizzard.com', 'blizzard.net'))


class BlizzardRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        if not allowed_download(newurl):
            raise SetupError('Installer redirected outside the approved Blizzard HTTPS domains. Download it manually from battle.net and pass --installer PATH.')
        return super().redirect_request(req, fp, code, msg, headers, newurl)


def download_installer(dest):
    dest = Path(dest)
    dest.parent.mkdir(parents=True, exist_ok=True)
    opener = urllib.request.build_opener(BlizzardRedirect())
    request = urllib.request.Request(INSTALLER_URL, headers={'User-Agent': 'WoW-SteamOS-Installer/0.1'})
    fd, name = tempfile.mkstemp(prefix='.bnet-download-', dir=dest.parent)
    try:
        count = 0
        with os.fdopen(fd, 'wb') as f, opener.open(request, timeout=30) as response:
            if not allowed_download(response.geturl()):
                raise SetupError('Unexpected installer download origin.')
            while True:
                chunk = response.read(1024 * 1024)
                if not chunk:
                    break
                count += len(chunk)
                if count > MAX_DOWNLOAD:
                    raise SetupError('Installer exceeded the 200 MiB download limit.')
                f.write(chunk)
        if count < 1024 or not pe_file(name):
            raise SetupError('Downloaded file is not a valid Windows installer. It was not installed.')
        atomic_write(dest, Path(name).read_bytes())
        return hashlib.sha256(dest.read_bytes()).hexdigest()
    finally:
        Path(name).unlink(missing_ok=True)

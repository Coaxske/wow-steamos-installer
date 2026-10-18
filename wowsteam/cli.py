"""Interactive, resumable setup. Game installation and login stay in Battle.net."""
from __future__ import annotations
import argparse
from contextlib import contextmanager
import fcntl
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import sys
import urllib.error
from . import __version__, vdf
from .manager import Manager
from .system import (SetupError, accounts, bnet_exe, catalog, discover, download_installer,
                     game_exe, native_steam, parse_selection, pe_file, require_idle, safe_path)


def prompt(message):
    try:
        answer = input(message).strip()
    except EOFError as e:
        raise SetupError('Input ended. Setup stopped; rerun interactively to continue.') from e
    if answer.lower() == 'q':
        raise SetupError('Cancelled. Existing game files were not deleted.')
    return answer


def yes(message):
    return prompt(message + ' [y/N] ').lower() in {'y', 'yes'}


def choose(items, label):
    if not items:
        raise SetupError('No choices are available.')
    for i, x in enumerate(items, 1):
        print(f'  {i}. {x}')
    while True:
        value = prompt(label + ' (number, q to quit): ')
        if value.isdecimal() and 1 <= int(value) <= len(items):
            return items[int(value) - 1]
        print('Choose a listed number.')


def editions(value=None):
    for i, item in enumerate(catalog(), 1):
        print(f'  {i}. {item["name"]} [{item["id"]}]')
    if value:
        return parse_selection(value)
    while True:
        try:
            return parse_selection(prompt('Choose editions (e.g. 1,2,3; q to quit): '))
        except SetupError as e:
            if str(e).startswith('Cancelled') or str(e).startswith('Input ended'):
                raise
            print(e)


def wait_for_exit():
    print('\nClose WoW and Battle.net, then choose Steam > Exit. Keep this terminal open.')
    while True:
        prompt('Press Enter after all three are closed (q to stop): ')
        try:
            require_idle()
            return
        except SetupError as e:
            print(e)


def open_tile(manager, key):
    state = manager.load()
    appid = state['tiles'][key]['appid']
    gameid = (appid << 32) | 0x02000000
    # Never invoke arbitrary shell text; Steam owns launching Proton and its runtime.
    if shutil.which('steam'):
        subprocess.Popen(['steam', f'steam://rungameid/{gameid}'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    else:
        print('Open Steam manually and press Play on ' + state['tiles'][key]['name'])


def select_exe(item, roots):
    candidates = discover(roots, item['folders'])
    print('\n' + item['name'] + '\n' + item['hint'])
    if candidates:
        selected = choose([str(p) for p in candidates] + ['Enter another path', 'Skip for now'], 'Select the installed game executable')
        if selected == 'Skip for now':
            return None
        if selected != 'Enter another path':
            p = safe_path(selected)
            if yes(f'Confirm this is {item["name"]}: {p}?'):
                return p
            return None
    value = prompt('Paste the full game .exe path, or press Enter to skip: ')
    if not value:
        return None
    p = safe_path(value.strip('"'))
    if not game_exe(p):
        raise SetupError('That is not a recognized WoW Windows executable. Choose Wow.exe, WowB.exe, WowClassic.exe, or another Wow*.exe game client, not a Launcher.exe.')
    if not yes(f'Confirm this is the desired {item["name"]} client: {p}?'):
        return None
    return p


def register(manager, selected):
    state = manager.load()
    prefix = Path(state['prefix'])
    roots = [Path(state['games']), prefix / 'pfx/drive_c/Program Files (x86)', prefix / 'pfx/drive_c/Program Files']
    specs = []
    used = {r['exe']: key for key, r in state['tiles'].items() if key != 'bnet'}
    for item in selected:
        exe = select_exe(item, roots)
        if exe is None:
            continue
        key = item['id']
        name = 'WoW — ' + item['name'] + ' [WoW SteamOS Installer]'
        if key == 'other':
            custom = prompt('Name this client (e.g. Next Expansion Beta): ')
            if not re.fullmatch(r'[\w .()-]{1,60}', custom):
                raise SetupError('Use a name of 1–60 letters, digits, spaces, periods, parentheses, or hyphens.')
            import hashlib
            key = 'custom-' + hashlib.sha256(str(exe).encode()).hexdigest()[:12]
            name = 'WoW — ' + custom + ' [WoW SteamOS Installer]'
        if str(exe) in used and used[str(exe)] != key:
            print('This client already has a tile. Use its realm/version selection; no duplicate tile was created.')
            continue
        used[str(exe)] = key
        specs.append((key, name, exe))
    if not specs:
        print('No new clients selected. Run register again after installing them.')
        return
    print('\nTiles to add or update:')
    for _, name, exe in specs:
        print(f'  {name}\n    {exe}')
    if not yes('Apply these Steam changes?'):
        return
    wait_for_exit()
    backup = manager.update(specs)
    print('Registered clients. Backup: ' + str(backup or 'no changes needed'))
    print('Open Steam > Non-Steam. Test each tile, log in if requested, and configure its controller layout.')
    print('Direct login varies by client/build. The Battle.net tile remains available for updates and fallback launches.')


def setup(manager, args, selected):
    state = manager.load()
    settings = {}
    if not state['tiles']:
        if args.prefix:
            prefix = safe_path(args.prefix)
            if not bnet_exe(prefix):
                raise SetupError('--prefix must be an existing Battle.net compatdata directory containing pfx/drive_c/.../Battle.net. Do not pass the pfx subfolder itself.')
            settings['prefix'] = str(prefix)
            print('Reusing an existing prefix. Choose the SAME Proton tool as its existing Steam shortcuts.')
        if args.games:
            settings['games'] = str(safe_path(args.games))
        if args.proton:
            tool = args.proton
        else:
            tool = choose(['proton_experimental', 'proton_10', 'Enter another Steam compatibility tool identifier'], 'Choose one Proton tool for all tiles')
            if tool.startswith('Enter'):
                tool = prompt('Internal Proton tool identifier (e.g. GE-Proton10-32): ')
        if not re.fullmatch(r'[A-Za-z0-9_.-]+', tool):
            raise SetupError('Invalid Proton identifier.')
        settings['proton'] = tool
    else:
        for option, key in ((args.prefix, 'prefix'), (args.games, 'games'), (args.proton, 'proton')):
            if option and (str(safe_path(option)) if key != 'proton' else option) != state[key]:
                raise SetupError(f'This setup already uses {key}={state[key]}; changing it requires a deliberate migration.')
    state.update(settings)
    prefix = safe_path(state['prefix'])
    games = safe_path(state['games'])
    if games == prefix or games.is_relative_to(prefix) or prefix.is_relative_to(games):
        raise SetupError('Keep the game directory and Proton prefix in separate, non-nested folders.')
    print('\nSetup plan')
    print('  Prefix: ' + str(prefix))
    print('  Game installation parent: ' + str(games))
    print('  Proton: ' + state['proton'])
    print('  Editions: ' + ', '.join(x['name'] for x in selected))
    print('  One Battle.net tile plus selected installed clients. Existing unrelated shortcuts are preserved.')
    if not yes('Continue with this setup?'):
        return
    prefix.mkdir(parents=True, exist_ok=True)
    games.mkdir(parents=True, exist_ok=True)
    executable = bnet_exe(prefix)
    if executable is None:
        installer = safe_path(args.installer) if args.installer else manager.root / 'downloads/Battle.net-Setup.exe'
        if args.installer:
            if not pe_file(installer):
                raise SetupError('The supplied installer is not a valid Windows PE file.')
        else:
            print('Downloading Battle.net from Blizzard over HTTPS...')
            sha = download_installer(installer)
            print('Downloaded installer SHA-256: ' + sha)
            print('This hash identifies the download; it is not a Blizzard-published signature.')
        wait_for_exit()
        manager.update([('bnet', 'Battle.net [WoW SteamOS Installer]', installer)], settings)
        print('\nSteam will open the Battle.net installer. If the tile does not launch, press Play manually.')
        print('Check Properties > Compatibility: ' + state['proton'] + '. Steam may download it first; custom tools must already be installed.')
        print('Install Battle.net in its default location, sign in INSIDE Battle.net, then exit Battle.net and Steam.')
        open_tile(manager, 'bnet')
        wait_for_exit()
        executable = bnet_exe(prefix)
        if executable is None:
            raise SetupError('Battle.net was not found at its default location. Rerun setup after completing installation. Nothing was deleted.')
    wait_for_exit()
    manager.update([('bnet', 'Battle.net [WoW SteamOS Installer]', executable)], settings)
    if yes('Open Battle.net to install/update the selected WoW editions now?'):
        print('\nIn Battle.net, select and install each desired version:')
        for item in selected:
            print('  ' + item['name'] + ': ' + item['hint'])
        windows_path = 'Z:' + str(games).replace('/', '\\')
        print('Choose this installation parent in the Windows folder picker: ' + windows_path)
        print('Battle.net may append World of Warcraft or another product directory. Confirm the path before installing.')
        print('Wait for complete downloads. Exit Battle.net, then Steam. Return here to register the game executables.')
        open_tile(manager, 'bnet')
        wait_for_exit()
    register(manager, selected)


@contextmanager
def exclusive(root):
    root.mkdir(parents=True, exist_ok=True, mode=0o700)
    lock = root / '.setup.lock'
    if lock.is_symlink():
        raise SetupError('Refusing a symlink at the setup lock.')
    with lock.open('a') as f:
        try:
            fcntl.flock(f, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as e:
            raise SetupError('Another setup process is already running.') from e
        yield


def parser():
    p = argparse.ArgumentParser(description='WoW SteamOS Installer: choose editions and create shared-prefix Steam shortcuts. Run in Desktop Mode; never sudo.')
    p.add_argument('command', nargs='?', choices=['setup', 'register', 'plan', 'status', 'remove-shortcuts', 'recover', 'list-editions'], default='setup')
    p.add_argument('--version', action='version', version=__version__)
    p.add_argument('--root', help='Managed data root; default ~/Games/wow-on-steam')
    p.add_argument('--steam-root', help='Native Steam installation directory')
    p.add_argument('--account', help='Numeric Steam userdata directory; prompted if multiple exist')
    p.add_argument('--editions', help='Comma-separated catalog IDs or numbers')
    p.add_argument('--prefix', help='Existing Battle.net compatdata root, first setup only')
    p.add_argument('--games', help='Game download parent, first setup only')
    p.add_argument('--proton', help='Internal Steam compatibility tool ID, first setup only')
    p.add_argument('--installer', help='Official Battle.net Windows installer downloaded manually')
    return p


def main(argv=None):
    args = parser().parse_args(argv)
    try:
        if args.command == 'list-editions':
            for item in catalog():
                print(item['id'] + ': ' + item['name'] + '\n  ' + item['hint'])
            return 0
        if args.command == 'plan':
            chosen = parse_selection(args.editions) if args.editions else catalog()
            print('READ-ONLY PLAN — no directories, downloads, or Steam changes.\n')
            for item in chosen:
                print(item['id'] + ': ' + item['name'] + '\n  ' + item['hint'])
            print('\nSetup creates an isolated shared prefix and one Battle.net tile. You install editions in Battle.net; the wizard registers their real executables. No realm selection or access is granted by this tool.')
            return 0
        if not sys.platform.startswith('linux') or platform.machine().lower() not in ('x86_64', 'amd64'):
            raise SetupError('This alpha targets x86-64 Linux with native Steam, including SteamOS and Bazzite.')
        if os.geteuid() == 0:
            raise SetupError('Run as your normal desktop user, without sudo.')
        root = safe_path(args.root or Path.home() / 'Games/wow-on-steam')
        if args.command in {'remove-shortcuts', 'recover', 'status', 'register'} and not (root / 'state.json').exists() and not (root / 'pending.json').exists():
            raise SetupError('No setup state exists at this root. Run setup first.')
        steam = safe_path(args.steam_root) if args.steam_root else native_steam(Path.home())
        users = accounts(steam)
        account = args.account
        # Prefer this tool's explicitly recorded account on subsequent runs.
        if not account and (root / 'state.json').is_file():
            try:
                account = json.loads((root / 'state.json').read_text())['account']
            except (ValueError, KeyError, TypeError) as e:
                raise SetupError('Invalid state.json; refusing to infer a replacement account.') from e
        if account and account not in users:
            raise SetupError('The selected account does not exist under this Steam userdata directory.')
        if not account:
            account = users[0] if len(users) == 1 else choose(users, 'Choose the Steam account directory')
        manager = Manager(root, steam, account)
        if args.command == 'status':
            state = manager.load()
            print('Recovery pending: ' + str(manager.pending.exists()))
            print('Proton: ' + state['proton'])
            for r in state['tiles'].values():
                print(r['name'] + ': ' + ('executable exists' if Path(r['exe']).is_file() else 'executable MISSING'))
            return 0
        if not sys.stdin.isatty():
            raise SetupError('Run this command in an interactive terminal. Use plan for unattended preview.')
        with exclusive(root):
            if args.command == 'recover':
                wait_for_exit()
                print('Recovered the interrupted transaction.' if manager.recover() else 'No recovery is pending.')
                return 0
            manager.check_pending()
            if args.command == 'remove-shortcuts':
                if yes('Remove only this tool\'s Steam tiles and Proton mappings? Game files, prefix, backups, and settings will remain.'):
                    wait_for_exit()
                    manager.update([], remove=True)
                    print('Managed Steam shortcuts removed. Game files and the shared prefix are untouched.')
                return 0
            selected = editions(args.editions)
            if args.command == 'register':
                register(manager, selected)
            else:
                setup(manager, args, selected)
        return 0
    except (SetupError, vdf.VdfError, OSError, ValueError, urllib.error.URLError) as e:
        print('\nStopped: ' + str(e), file=sys.stderr)
        print('No game-deletion operation is performed by this tool. See docs/RECOVERY.md.', file=sys.stderr)
        return 1
    except KeyboardInterrupt:
        print('\nCancelled. Rerun to continue; if a transaction was interrupted, use recover.', file=sys.stderr)
        return 130

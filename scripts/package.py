#!/usr/bin/env python3
"""Build reproducible source, executable zipapp, and standalone KDE launcher."""
import base64
import hashlib
import io
from pathlib import Path
import textwrap
import zipfile

ROOT = Path(__file__).resolve().parents[1]
VERSION = '0.1.0-alpha.2'
SLUG = 'wow-steamos-installer'
FILES = {'README.md', 'LICENSE', 'NOTICE', 'CONTRIBUTING.md', 'SECURITY.md',
         'CHANGELOG.md', '.gitignore', '.gitattributes', 'wow_on_steam.py',
         'start.sh', 'Run-From-Source.desktop'}
DIRS = ('wowsteam', 'tests', 'docs', 'scripts', '.github')
# A temporary publication workflow is deliberately excluded from distributions.
EXCLUDED = {'.github/workflows/privacy-reset.yml'}


def selected_files(root):
    selected = [root / p for p in FILES]
    for directory in DIRS:
        selected.extend(p for p in (root / directory).rglob('*') if p.is_file()
                        and '__pycache__' not in p.parts and p.suffix not in ('.pyc', '.pyo'))
    result = []
    for p in sorted(selected):
        relative = p.relative_to(root).as_posix()
        if relative in EXCLUDED:
            continue
        if p.is_symlink():
            raise RuntimeError(f'Refusing symlink: {relative}')
        if p.suffix.lower() in {'.exe', '.bin', '.log'} or p.name in {'state.json', 'pending.json', '.env'}:
            raise RuntimeError(f'Refusing private/runtime file: {relative}')
        result.append(p)
    return result


def zip_bytes(entries):
    buffer = io.BytesIO()
    with zipfile.ZipFile(buffer, 'w', compression=zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
        for name, data, mode in sorted(entries):
            info = zipfile.ZipInfo(name, date_time=(2026, 10, 18, 0, 0, 0))
            info.create_system = 3
            info.external_attr = (0o100000 | mode) << 16
            info.compress_type = zipfile.ZIP_DEFLATED
            archive.writestr(info, data)
    return buffer.getvalue()


def desktop_bytes(app):
    digest = hashlib.sha256(app).hexdigest()
    payload = '\n'.join('    ' + repr(line) for line in textwrap.wrap(base64.b64encode(app).decode('ascii'), 100))
    launcher = f'''import base64, hashlib, subprocess, sys, tempfile
from pathlib import Path
encoded = (
{payload}
)
app = base64.b64decode(encoded, validate=True)
if hashlib.sha256(app).hexdigest() != {digest!r}:
    raise SystemExit('Damaged embedded installer. Download the release again.')
with tempfile.TemporaryDirectory(prefix='wow-steamos-installer-') as directory:
    executable = Path(directory) / 'installer.pyz'
    executable.write_bytes(app)
    result = subprocess.run([sys.executable, str(executable), *sys.argv[1:]]).returncode
if not sys.argv[1:] and sys.stdin.isatty():
    try:
        input('Press Enter to close this window...')
    except (EOFError, KeyboardInterrupt):
        pass
raise SystemExit(result)
'''
    # %k is passed as one argument by the desktop launcher; never interpolate it
    # into a shell command. File URLs and paths containing spaces both work.
    command = ("import pathlib,sys,urllib.parse; u=urllib.parse.urlparse(sys.argv[1]); "
               "p=pathlib.Path(urllib.parse.unquote(u.path) if u.scheme=='file' else sys.argv[1]); "
               "s=''.join(line[2:] for line in p.read_text(encoding='utf-8').splitlines(True) if line.startswith('#>')); "
               "sys.argv=[str(p),*sys.argv[2:]]; exec(compile(s,str(p),'exec'),{'__name__':'__main__','__file__':str(p)})")
    header = ('[Desktop Entry]\nType=Application\nName=WoW SteamOS Installer\n'
              'Comment=Install World of Warcraft editions on Steam Deck and Linux\n'
              'Icon=applications-games\nTerminal=true\n'
              f'Exec=python3 -c "{command}" %k\nCategories=Game;Utility;\n\n'
              '# Standalone launcher: the versioned Python application is embedded below.\n'
              '# No bootstrap download, credentials, or author identity is embedded.\n'
              '# Source and license are included in the matching release source ZIP.\n')
    return (header + ''.join('#>' + line + '\n' for line in launcher.splitlines())).encode('utf-8')


def build(root=ROOT, output=None):
    root = Path(root)
    dist = Path(output) if output is not None else root / 'dist'
    dist.mkdir(parents=True, exist_ok=True)
    selected = selected_files(root)
    stem = f'{SLUG}-{VERSION}'
    source = dist / f'{stem}.zip'
    source.write_bytes(zip_bytes([
        (stem + '/' + p.relative_to(root).as_posix(), p.read_bytes(),
         0o755 if p.name in {'start.sh', 'wow_on_steam.py', 'Run-From-Source.desktop'} else 0o644)
        for p in selected]))
    runtime = [(p.relative_to(root).as_posix(), p.read_bytes(), 0o644) for p in selected
               if p.relative_to(root).parts[0] == 'wowsteam']
    runtime += [('__main__.py', (root / 'wow_on_steam.py').read_bytes(), 0o644),
                ('LICENSE', (root / 'LICENSE').read_bytes(), 0o644),
                ('NOTICE', (root / 'NOTICE').read_bytes(), 0o644)]
    app = b'#!/usr/bin/env python3\n' + zip_bytes(runtime)
    executable = dist / 'WoW-SteamOS-Installer.pyz'
    executable.write_bytes(app)
    desktop = dist / 'WoW-SteamOS-Installer.desktop'
    desktop.write_bytes(desktop_bytes(app))
    executable.chmod(0o755)
    desktop.chmod(0o755)
    artifacts = [source, executable, desktop]
    (dist / 'SHA256SUMS').write_text(''.join(
        f'{hashlib.sha256(p.read_bytes()).hexdigest()}  {p.name}\n' for p in artifacts), encoding='utf-8')
    return artifacts


if __name__ == '__main__':
    for artifact in build():
        print(artifact)

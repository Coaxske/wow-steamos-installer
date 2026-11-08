#!/usr/bin/env python3
"""Build a portable native executable from a matching local Qt installation.
Build-time Python only; the runtime contains no Python and no old installer.
"""
import argparse, os, re, shutil, subprocess, tarfile, json, hashlib, zipfile
from pathlib import Path

p=argparse.ArgumentParser()
p.add_argument('--qt-prefix',type=Path,default=Path('/usr'))
p.add_argument('--output',type=Path)
p.add_argument('--build-mode',choices=['public','developer'],default='public')
p.add_argument('--build-dir',type=Path)
a=p.parse_args()
root=Path(__file__).resolve().parents[1]
build=(a.build_dir or root/('build-'+a.build_mode)).resolve()
checker=root/'target'/a.build_mode/'release/wow-release'
subprocess.run([str(checker),'verify-source',str(root)],check=True)
subprocess.run([str(checker),'verify-binary',str(build/'wow-steamos'),str(build/'native-build.json')],check=True)
build_info=json.loads((build/'native-build.json').read_text())
if build_info.get('build_mode')!=a.build_mode or build_info.get('developer_tools')!=(a.build_mode=='developer'):
 raise SystemExit('Build/checker mode does not match the requested package. Build the selected mode first.')
native_info=json.loads(subprocess.run([str(build/'wow-steamos'),'--build-info'],capture_output=True,text=True,check=True).stdout)
for key in ['version','source_sha256','developer_tools','build_mode']:
 if native_info.get(key)!=build_info.get(key):raise SystemExit('Native/checker build identity differs: '+key)

out=(a.output or root/('dist-'+a.build_mode)).resolve()
existing=out/'BUILD-INFO.json'
if existing.exists() and json.loads(existing.read_text()).get('build_mode')!=a.build_mode:
 raise SystemExit('Output belongs to another build mode; select a separate output directory.')
opposite=out/('WoW-SteamOS-Developer-Linux-x86_64' if a.build_mode=='public' else 'WoW-SteamOS-Rust-Linux-x86_64')
if opposite.exists():raise SystemExit('Refusing to mix public and developer executables in one output directory.')
out.mkdir(parents=True,exist_ok=True)
runtime=out/'runtime'
if runtime.exists():shutil.rmtree(runtime)
for d in ['bin','lib','qml','plugins','licenses']:(runtime/d).mkdir(parents=True)
extra_components=[]
if a.build_mode=='developer':
 native_plugins=Path('/usr/lib/x86_64-linux-gnu/gstreamer-1.0')
 gst=runtime/'plugins/gstreamer';gst.mkdir()
 for name in ['libgstpipewire.so','libgstapp.so','libgstvideoconvertscale.so','libgstcoreelements.so']:
  f=native_plugins/name
  if not f.is_file():raise SystemExit('Missing packaged controller capture plugin: '+name)
  shutil.copy2(f,gst/name);extra_components.append((name,f))
 scanner=Path('/usr/lib/x86_64-linux-gnu/gstreamer1.0/gstreamer-1.0/gst-plugin-scanner')
 if not scanner.is_file():raise SystemExit('Missing GStreamer plugin scanner')
 shutil.copy2(scanner,runtime/'bin/gst-plugin-scanner');extra_components.append(('gst-plugin-scanner',scanner))
 (runtime/'tessdata').mkdir()
 trained=Path('/usr/share/tesseract-ocr/5/tessdata/eng.traineddata')
 if not trained.is_file():raise SystemExit('Missing packaged English OCR model')
 shutil.copy2(trained,runtime/'tessdata/eng.traineddata');extra_components.append(('eng.traineddata',trained))
binary=build/'wow-steamos'
shutil.copy2(binary,runtime/'bin/wow-steamos')
subprocess.run(['strip','--strip-unneeded',str(runtime/'bin/wow-steamos')],check=True)
qt=a.qt_prefix.resolve()
lib=qt/'lib/x86_64-linux-gnu'
if not lib.exists():lib=qt/'lib'
qml=lib/'qt6/qml';plugins=lib/'qt6/plugins'
if not qml.is_dir():qml=qt/'qml'
if not plugins.is_dir():plugins=qt/'plugins'
for required in ['Qt/labs/folderlistmodel']:
 if not (qml/required/'qmldir').is_file():raise SystemExit('Missing required QML runtime module: '+required)
for module in ['QtQuick','QtQml','Qt/labs/folderlistmodel']:
 if (qml/module).is_dir():shutil.copytree(qml/module,runtime/'qml'/module,dirs_exist_ok=True)
for d in ['platforms','xcbglintegrations','wayland-shell-integration','wayland-graphics-integration-client','wayland-decoration-client','imageformats']:
 if (plugins/d).is_dir():shutil.copytree(plugins/d,runtime/'plugins'/d,dirs_exist_ok=True)
# Desktop backends only. Do not pull in embedded framebuffer / VNC stacks.
for f in (runtime/'plugins/platforms').glob('*.so'):
 if not any(x in f.name for x in ['qxcb','qwayland','qoffscreen','qminimal.so']): f.unlink()
# Keep distribution-specific graphics drivers and glibc on the host.
external=re.compile(r'^(ld-linux|libc\.|libm\.|libdl\.|libpthread\.|librt\.|libresolv\.|libutil\.|libnss_|libstdc\+\+|libgcc_s|libGL[EX]?\.|libOpenGL\.|libGLdispatch\.|libEGL\.|libGLES|libvulkan|libdrm|libgbm|libpipewire-0\.3\.|libspa-)')
env=dict(os.environ,LD_LIBRARY_PATH=str(lib)+':'+os.environ.get('LD_LIBRARY_PATH',''))
pending=[runtime/'bin/wow-steamos']+([runtime/'bin/gst-plugin-scanner'] if a.build_mode=='developer' else [])+list(runtime.glob('qml/**/*.so'))+list(runtime.glob('plugins/**/*.so'))
seen=set();system=set();bundled_sources={}
while pending:
 f=pending.pop()
 if str(f) in seen:continue
 seen.add(str(f))
 output=subprocess.run(['ldd',str(f)],env=env,text=True,capture_output=True,check=True).stdout
 for line in output.splitlines():
  if '=> not found' in line:raise SystemExit('Unresolved runtime dependency: '+line)
  match=re.search(r'^\s*(\S+)\s+=>\s+(/\S+)',line)
  if not match:continue
  name,path=match.groups()
  if external.match(name):system.add(name);continue
  target=runtime/'lib'/name
  if not target.exists():shutil.copy2(Path(path).resolve(),target);pending.append(target);bundled_sources[name]=Path(path).resolve()
(runtime/'HOST-DEPENDENCIES.txt').write_text('\n'.join(sorted(system))+'\n')
(runtime/'bin/qt.conf').write_text('[Paths]\nPrefix=..\nPlugins=plugins\nQml2Imports=qml\nLibraries=lib\n')
# Include every available distribution notice, then fail closed if a bundled
# library cannot be tied to a notice. No hardcoded library-family allowlist.
for docroot in [qt/'share/doc',Path('/usr/share/doc')]:
 if docroot.exists():
  for f in docroot.glob('*/copyright'):
   if f.is_file(): shutil.copy2(f,runtime/'licenses'/f'{f.parent.name}.txt')
index_path=os.environ.get('WOW_PACKAGE_INDEX')
index=json.loads(Path(index_path).read_text()) if index_path else {}
audit={}
for name,path in bundled_sources.items():
 try: relative='/usr/'+path.relative_to(qt).as_posix()
 except ValueError: relative=str(path)
 package=index.get(relative)
 if not package:
  found=subprocess.run(['dpkg-query','-S',str(path)],capture_output=True,text=True)
  if found.returncode==0:package=found.stdout.split(': ')[0].split(':')[0]
 if not package or not (runtime/'licenses'/f'{package}.txt').is_file():
  raise SystemExit('Missing package/license notice for '+name)
 audit[name]={'package':package,'notice':f'licenses/{package}.txt'}
for name,path in extra_components:
 found=subprocess.run(['dpkg-query','-S',str(path)],capture_output=True,text=True,check=True)
 package=found.stdout.split(': ')[0].split(':')[0]
 if not (runtime/'licenses'/f'{package}.txt').is_file():raise SystemExit('Missing controller component notice: '+name)
 audit[name]={'package':package,'notice':f'licenses/{package}.txt'}
(runtime/'LICENSE-INVENTORY.json').write_text(json.dumps(audit,indent=2)+'\n')
# Avoid redistributing unrelated host documentation.
used={entry['notice'].split('/')[-1] for entry in audit.values()}
# Qt/SDL plugins are separate from libraries: retain their package notices too.
for f in (runtime/'licenses').glob('*.txt'):
 if f.name not in used and not any(x in f.name for x in ['qt6','qml6','sdl2']):f.unlink()
for license in ['LGPL-3','GPL-3','LGPL-2.1','GPL-2','Apache-2.0']:
 f=Path('/usr/share/common-licenses')/license
 if f.exists():shutil.copy2(f,runtime/'licenses'/license)
shutil.copy2(root/'THIRD-PARTY.md',runtime/'licenses/THIRD-PARTY.md')
payload=out/'payload.tar.gz'
with tarfile.open(payload,'w:gz',compresslevel=9) as t:
 for item in sorted(runtime.iterdir()):t.add(item,arcname=item.name)
executable=out/('WoW-SteamOS-Rust-Linux-x86_64' if a.build_mode=='public' else 'WoW-SteamOS-Developer-Linux-x86_64')
subprocess.run(['rustc','--edition=2021','-C','opt-level=2','-C','strip=symbols',str(root/'packaging/launcher.rs'),'-o',str(executable)],env=dict(os.environ,WOW_PAYLOAD=str(payload)),check=True)
executable.chmod(0o755)
print(executable)

# Snapshot precisely the inputs hashed into the executable. Verify archive bytes,
# not just the working tree, so stale or unexpected Source.zip content fails.
directories=['src','ui','scripts','packaging','tests','vendor','.cargo']
files=['Cargo.toml','Cargo.lock','CMakeLists.txt','build.rs','build_support.rs','LICENSE','README.md','FEATURES.md','VALIDATION.md','TEST-ON-DEVICE.md','THIRD-PARTY.md']
paths=[root/name for name in files]
for name in directories:
 paths += [f for f in (root/name).rglob('*') if f.is_file() and '__pycache__' not in f.parts]
source=out/'Source.zip'
with zipfile.ZipFile(source,'w',compression=zipfile.ZIP_DEFLATED,compresslevel=9,strict_timestamps=False) as z:
 for f in sorted(paths):z.write(f,f.relative_to(root).as_posix())
with zipfile.ZipFile(source) as z:
 if sorted(z.namelist()) != sorted(f.relative_to(root).as_posix() for f in paths):raise SystemExit('Unexpected source archive contents')
 h=hashlib.sha256()
 for name in sorted(z.namelist()):h.update(name.encode()+b'\0');h.update(hashlib.sha256(z.read(name)).digest())
 if h.hexdigest()!=build_info['source_sha256']:raise SystemExit('Source archive fingerprint mismatch')
subprocess.run([str(checker),'verify-source',str(root)],check=True)
build_info['native_pre_strip_sha256']=build_info.pop('binary_sha256')
build_info['native_payload_sha256']=hashlib.sha256((runtime/'bin/wow-steamos').read_bytes()).hexdigest()
build_info['binary_sha256']=hashlib.sha256(executable.read_bytes()).hexdigest()
build_info['portable_executable_sha256']=build_info['binary_sha256']
build_info['source_archive_sha256']=hashlib.sha256(source.read_bytes()).hexdigest()
build_info['libc']=os.confstr('CS_GNU_LIBC_VERSION')
(out/'BUILD-INFO.json').write_text(json.dumps(build_info,indent=2)+'\n')

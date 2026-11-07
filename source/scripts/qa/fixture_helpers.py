"""VM-only real-engine integration checks; synthetic data is never a UI pass."""
from pathlib import Path
import os,sys,json,subprocess,hashlib,struct,copy,time,traceback
import shortcut_fixture as v
H=Path.home()
if os.environ.get('A10_ENGINE_CONTAINER_FIXTURES')=='1':
 assert Path('/run/.containerenv').exists() and str(H)=='/output/vmlab', 'Explicit isolated container only'
else:
 assert H.name=='vmlab' and os.getuid()==1000
 assert subprocess.check_output(['systemd-detect-virt'],text=True).strip()=='kvm'
ROOT=H/'vm-lab/alpha10-reliability';ROOT.mkdir(mode=0o700,parents=True,exist_ok=True)
CLI=Path(sys.argv[1]).resolve();RUN=sys.argv[2] if len(sys.argv)>2 else 'baseline';OUT=ROOT/('engine-'+RUN);OUT.mkdir(mode=0o700,exist_ok=False)
results=[]
def put(p,data):
 p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(data if isinstance(data,bytes) else data.encode())
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest() if p.exists() else None
def setval(e,k,val,tag=1):
 for n,(key,t,x) in enumerate(e):
  if key==k:e[n]=(key,tag,val);return
 e.append((k,tag,val))
def entry(ident,title,exe,opts):return [('appid',2,struct.pack('<I',ident)),('appname',1,title),('exe',1,'"'+str(exe)+'"'),('StartDir',1,'"'+str(Path(exe).parent)+'"'),('LaunchOptions',1,opts),('IsHidden',2,struct.pack('<I',1)),('ControllerSentinel',1,'layout-for-'+str(ident)),('tags',0,[])]
def payload(value):return bytes.fromhex(value) if isinstance(value,str) else bytes(value)
def pe():
 b=bytearray(2048);b[:2]=b'MZ';b[0x3c]=0x80;b[0x80:0x84]=b'PE\0\0';b[0x84:0x86]=struct.pack('<H',0x8664);b[0x98:0x9a]=struct.pack('<H',0x20b);return bytes(b)
class Case:
 def __init__(self,name):
  self.home=OUT/name/'home';self.home.mkdir(parents=True);self.data=self.home/'.local/share/wow-steamos-rust';self.steam=self.home/'.local/share/Steam';self.sc=self.steam/'userdata/123/config/shortcuts.vdf';self.cfg=self.steam/'config/config.vdf';self.games=self.home/'Games';self.games.mkdir();self.env=dict(os.environ,HOME=str(self.home),XDG_DATA_HOME=str(self.home/'.local/share'),XDG_CONFIG_HOME=str(self.home/'.config'),XDG_CACHE_HOME=str(self.home/'.cache'))
  put(self.sc,v.encode([('shortcuts',0,[])]));put(self.cfg,'"InstallConfigStore" { "Software" { "Valve" { "Steam" { "CompatToolMapping" { } } } } }')
  p=self.steam/'steamapps/common/Proton 11';put(p/'proton','test');put(p/'toolmanifest.vdf','"manifest" { "commandline" "/proton %verb%" }');put(p/'version','1752000000 proton-11.0-1')
 def call(self,action,ok=True,**req):
  req={'action':action,**req};p=subprocess.run([str(CLI)],input=json.dumps(req),text=True,capture_output=True,env=self.env,timeout=25)
  d=json.loads(p.stdout);(self.home.parent/'calls.jsonl').open('a').write(json.dumps({'request':req,'response':d})+'\n')
  assert d['ok']==ok,(action,d)
  return d.get('data') if ok else d['error']
 def state(self):return self.call('init')['state']
 def plan(self):return json.loads((self.data/'review.json').read_text())
 def entries(self):return [e for _,tag,e in v.lookup(v.parse(self.sc.read_bytes()),'shortcuts') if tag==0]
 def write_entries(self,es):put(self.sc,v.encode([('shortcuts',0,[(str(n),0,e) for n,e in enumerate(es)])]))
 def setup(self,download=True):
  q=self.call('review_setup',steam=str(self.steam),user='123',proton='proton_11',location=str(self.games),editions=['era']);i=self.plan()['installation']
  if download:put(Path(i['root'])/'Battle.net-Setup.exe',pe())
  self.call('apply',token=q['token']);return self.state()['installations'][0]
 def add_games(self,i,zero=False):
  prefix=Path(i['prefix']);bn=prefix/'pfx/drive_c/Program Files (x86)/Battle.net/Battle.net.exe';put(bn,b'MZ');put(bn.with_name('Battle.net Launcher.exe'),b'MZ')
  put(prefix/'pfx/drive_c/Program Files (x86)/Steam/steamclient.dll',b'MZ')
  game=prefix/'pfx/drive_c/Program Files (x86)/World of Warcraft/_classic_era_/WoWClassic.exe';put(game,b'' if zero else b'MZ');return bn,game
 def repair(self,i):
  p=self.call('review_repair',id=i['id']);self.call('apply',token=p['token'])
 def image(self):return {str(p):sha(p) for p in [self.sc,self.cfg,self.data/'state.json']}
def run(name,fn):
 try:detail=fn();status='PASS'
 except Exception as e:detail=str(e);status='FAIL';put(OUT/(name+'-trace.txt'),traceback.format_exc())
 results.append({'name':name,'status':status,'evidence':detail,'kind':'real-engine / synthetic container fixtures' if os.environ.get('A10_ENGINE_CONTAINER_FIXTURES')=='1' else 'real-engine / synthetic VM fixtures'});put(OUT/'results.json',json.dumps(results,indent=2));print(name,status,flush=True)
def repeated():
 c=Case('repeated-repair');i=c.setup();c.add_games(i);c.repair(i);es=c.entries();ids=[v.lookup(e,'appid') for e in es]
 for e in es:
  if any(k=='wowRustEdition' for k,t,x in e):setval(e,'appname','My Era custom name');setval(e,'LaunchOptions',"PROTON_LOG=1 %command% --my-custom-argument 'two words' --exec=\"launch WoWC\"")
 c.write_entries(es);c.repair(i);before=c.image();mtime=c.sc.stat().st_mtime_ns
 for _ in range(5):c.repair(i)
 assert c.image()==before,'Unnecessary target byte changes during repeated repair'
 assert c.sc.stat().st_mtime_ns==mtime,'Byte-identical shortcuts unnecessarily rewritten'
 assert [v.lookup(e,'appid') for e in c.entries()]==ids
 assert any('My Era custom name'==v.lookup(e,'appname') and '--my-custom-argument' in v.lookup(e,'LaunchOptions') for e in c.entries())
 return 'Five repeated repairs stable; IDs, custom name/options, controller sentinel and target bytes/shortcut mtime preserved.'
def python_adopt():
 c=Case('python-adoption-spaces');prefix=c.games/'Python legacy copy with spaces/prefix';i={'prefix':str(prefix)};bn,g=c.add_games(i);ret=g.parent.parent/'_retail_/Wow.exe';put(ret,b'MZ')
 ids=[0x80000001,0x80000002,0x80000003];es=[entry(ids[0],'My launcher',bn,''),entry(ids[1],'My Retail name',bn,' --exec="launch WoW"'),entry(ids[2],'My Era name',bn,' --exec="launch WoWC"')]
 for e in es:setval(e,'LaunchOptions',"STEAM_COMPAT_DATA_PATH='"+str(prefix)+"' PROTON_LOG=1 %command% --custom-preserve 'two words'"+v.lookup(e,'LaunchOptions'))
 c.write_entries(es);put(c.cfg,'"InstallConfigStore" { "Software" { "Valve" { "Steam" { "CompatToolMapping" { '+''.join('"'+str(i)+'" { "name" "proton_11" } ' for i in ids)+'} } } } }')
 layouts=c.steam/'userdata/123/config/controller-fixture.json';put(layouts,json.dumps({str(i):'selected-layout-'+str(i) for i in ids}));layout_hash=sha(layouts)
 candidates=c.call('adoption_candidates',prefix=str(prefix))['candidates'];assert len(candidates)==3
 q=c.call('review_adopt',prefix=str(prefix),steam=str(c.steam),user='123',appid=ids[0],selected=ids);before=c.sc.read_bytes();assert before==v.encode([('shortcuts',0,[(str(n),0,e) for n,e in enumerate(es)])])
 c.call('apply',token=q['token']);i=c.state()['installations'][0]
 for _ in range(3):c.repair(i)
 after=c.entries();assert len(after)==3;assert sorted(struct.unpack('<I',v.lookup(e,'appid'))[0] for e in after)==ids
 for e in after:
  ident=struct.unpack('<I',v.lookup(e,'appid'))[0];assert v.lookup(e,'appname')==['My launcher','My Retail name','My Era name'][ids.index(ident)];assert '--custom-preserve' in v.lookup(e,'LaunchOptions');assert v.lookup(e,'ControllerSentinel')=='layout-for-'+str(ident)
 assert sha(layouts)==layout_hash
 return 'Python-style untagged shortcuts adopted via engine review/apply; three original IDs, custom names/options and synthetic controller associations retained through three repairs. No physical layout claim.'
def interrupted(which,conflict=False):
 c=Case('interrupted-'+which+('-conflict' if conflict else ''));i=c.setup();c.add_games(i)
 if which=='repair':q=c.call('review_repair',id=i['id'])
 else:
  c=Case('interrupted-install-journal');q=c.call('review_setup',steam=str(c.steam),user='123',proton='proton_11',location=str(c.games),editions=['era'])
 plan=c.plan();j=plan['journal'];put(c.data/'transaction.json',json.dumps(j));change=j['changes'][0];put(Path(change['path']),payload(change['after']))
 if conflict:
  target=Path(j['changes'][1]['path']);saved=target.read_bytes();put(target,saved+b'\n// external conflicting edit\n');before=c.image();err=c.call('recover',ok=False);assert c.image()==before and (c.data/'transaction.json').exists();put(target,saved)
 c.call('recover');assert not (c.data/'transaction.json').exists()
 for change in j['changes']:assert Path(change['path']).read_bytes()==payload(change['after'])
 c.call('recover');return 'Real generated journal partially applied as an interruption fixture; recovery completed all after-bytes and repeated recovery was safe.'+(' External conflict refused without writes before recovery.' if conflict else '')
def download_failure():
 c=Case('download-failure');shim=c.home/'bin/curl';put(shim,'#!/bin/sh\nexit 22\n');shim.chmod(0o755);c.env['PATH']=str(shim.parent)+':'+os.environ['PATH']
 q=c.call('review_setup',steam=str(c.steam),user='123',proton='proton_11',location=str(c.games),editions=['era']);before=c.image();c.call('apply',ok=False,token=q['token']);assert c.image()==before;assert not(c.data/'transaction.json').exists();i=c.plan()['installation'];assert not(Path(i['root'])/'Battle.net-Setup.part').exists()
 put(Path(i['root'])/'Battle.net-Setup.exe',pe());c.call('apply',token=q['token']);assert len(c.state()['installations'])==1
 return 'Simulated curl exit 22 through real engine: no Steam/state commit or partial download; same reviewed operation succeeded with a valid cached PE fixture. Not a real Blizzard download.'
def kill_download():
 c=Case('killed-download');shim=c.home/'bin/curl';put(shim,'#!/bin/sh\nwhile [ "$1" != "--output" ]; do shift; done\nshift\nprintf partial > "$1"\nsleep 2\nexit 22\n');shim.chmod(0o755);c.env['PATH']=str(shim.parent)+':'+os.environ['PATH'];q=c.call('review_setup',steam=str(c.steam),user='123',proton='proton_11',location=str(c.games),editions=['era']);i=c.plan()['installation'];before=c.image()
 p=subprocess.Popen([str(CLI)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,env=c.env);p.stdin.write(json.dumps({'action':'apply','token':q['token']}));p.stdin.close();time.sleep(.4);p.terminate();p.wait(timeout=5);time.sleep(2.2);assert c.image()==before
 put(Path(i['root'])/'Battle.net-Setup.exe',pe());c.call('apply',token=q['token']);assert len(c.state()['installations'])==1
 return 'Actual engine process terminated during a controlled download; no config commit; retry used cached fixture and completed. Transport was a local shim.'
def removal(interrupt=False):
 c=Case('removal'+('-interrupted' if interrupt else ''));i=c.setup();bn,g=c.add_games(i);c.repair(i);other=c.home/'unrelated keep.txt';put(other,'KEEP');shared=Path(i['prefix'])/'pfx/drive_c/Program Files (x86)/Steam/steamclient.dll';root=Path(i['root']);put(root/'keep-unrelated.dat','outside removal folders');put(g.parent/'WTF/Config.wtf','settings')
 es=c.entries();es.append(entry(0x8fffffff,'Unrelated application',c.home/'not-in-selected-prefix','--keep'));c.write_entries(es);sentinel=v.fingerprint(c.entries()[-1]);keep=sha(other);shared_sha=sha(shared)
 q=c.call('review_clean',id=i['id'],keep_settings=not interrupt);before=c.image();c.call('apply',ok=False,token=q['token'],confirmation='wrong');assert c.image()==before
 if interrupt:
  plan=c.plan();j=plan['journal']
  for ch in j['changes']:put(Path(ch['path']),payload(ch['after']))
  plan['journal']['done']=True;r=plan['remove'][0];Path(r['path']).rename(r['quarantine']);put(c.data/'cleanup.json',json.dumps({'plan':plan,'settings_saved':True}));c.call('recover')
 else:c.call('apply',token=q['token'],confirmation='REMOVE THIS COPY')
 assert not g.exists() and not bn.exists();assert sha(other)==keep and sha(shared)==shared_sha and(root/'keep-unrelated.dat').exists();assert len(c.entries())==1 and v.fingerprint(c.entries()[0])==sentinel
 remains=[str(p.relative_to(root)) for p in root.rglob('*') if p.is_file()];put(c.home.parent/'remnants.json',json.dumps(remains,indent=2))
 assert not c.state()['installations'];return 'Selected launcher/game removed; unrelated file/tile, shared Steam support and environment remainder retained. '+('Interrupted cleanup resumed after quarantine rename.' if interrupt else 'Wrong confirmation refused first; settings backup retained.')
def discovery():
 c=Case('moved-missing-multiple');i=c.setup();bn,g=c.add_games(i);other=c.games/'Another installation with spaces/prefix';c.add_games({'prefix':str(other)});incomplete=c.games/'Incomplete install/_retail_';incomplete.mkdir(parents=True)
 scan=c.call('scan')['scan'];assert len(scan['launchers'])==2 and len(scan['games'])==2
 moved=c.games/'Moved folder with spaces';other.parent.rename(moved);scan=c.call('scan')['scan'];assert len(scan['games'])==2 and any('Moved folder with spaces' in x['path'] for x in scan['games']);assert not any('Incomplete' in x['path'] for x in scan['games'])
 bn.rename(bn.with_name('Battle.net.exe.missing'));before=c.image();c.call('review_repair',ok=False,id=i['id']);assert c.image()==before
 return 'Multiple and moved prefixes with spaces discovered; no-executable incomplete folder excluded; missing launcher repair refused without target changes.'
def empty_game():
 c=Case('zero-byte-game');i=c.setup();bn,g=c.add_games(i,zero=True);scan=c.call('scan')['scan'];assert not any(x['executable']==str(g) for x in scan['games']),'Zero-byte WoWClassic.exe advertised as discovered/installed game'
 return 'Zero-byte game executable is not advertised as a completed game.'
def alpha9_upgrade():
 c=Case('alpha9-saved-state-duplicate');i=c.setup();bn,g=c.add_games(i);c.repair(i);state=c.state();i=state['installations'][0];beta=g.parent.parent/'_classic_beta_/Wow.exe';put(beta,b'MZ')
 opts="STEAM_COMPAT_DATA_PATH='"+i['prefix']+"' PROTON_LOG=1 %command% --custom-keep --exec=\"launch WoWF\""
 original=entry(0x81234567,'My original Forever',bn,opts);setval(original,'wowRustEdition','forever');setval(original,'tags',[('0',1,'wow-rust:'+i['id'])],0)
 duplicate=entry(0x81234568,'WoW Classic / Forever Beta',bn,"STEAM_COMPAT_DATA_PATH='"+i['prefix']+"' %command% --game=wow_classic_beta");setval(duplicate,'wowRustEdition','classic_beta');setval(duplicate,'wowRustGame',str(beta));setval(duplicate,'tags',[('0',1,'wow-rust:'+i['id'])],0)
 c.write_entries([duplicate]+c.entries()+[original]);i['editions']=['era','forever'];i['shortcuts'].append(0x81234567);i['bindings'].append({'appid':0x81234567,'edition':'forever','game':None,'launcher':False});put(c.data/'state.json',json.dumps(state))
 import re
 cfg=c.cfg.read_text();cfg=re.sub(r'("CompatToolMapping"\s*\{)',lambda m:m.group(1)+' "2166572391" { "name" "proton_11" } "2166572392" { "name" "proton_11" } ',cfg,count=1);put(c.cfg,cfg)
 for _ in range(3):c.repair(i)
 es=c.entries();assert len(es)==3,'Expected launcher, Era and original Forever only';kept=next(e for e in es if struct.unpack('<I',v.lookup(e,'appid'))[0]==0x81234567)
 assert v.lookup(kept,'appname')=='My original Forever' and '--custom-keep' in v.lookup(kept,'LaunchOptions') and v.lookup(kept,'ControllerSentinel')=='layout-for-2166572391';assert not any(struct.unpack('<I',v.lookup(e,'appid'))[0]==0x81234568 for e in es)
 # A synthetic formatter rewrite is distinct from a real Steam process run.
 before_ids=sorted(struct.unpack('<I',v.lookup(e,'appid'))[0] for e in es);c.write_entries(list(reversed(es)));put(c.cfg,c.cfg.read_text().replace('\t','\t\t'));c.repair(i)
 assert sorted(struct.unpack('<I',v.lookup(e,'appid'))[0] for e in c.entries())==before_ids
 return 'Alpha9-compatible saved bindings with missing game association and duplicate Beta upgraded; original Forever ID/name/options/controller sentinel retained. Repaired after synthetic formatter/reorder changes; not a real Steam-run pass.'
def cancel_undo():
 c=Case('engine-cancel-undo');i=c.setup();bn,g=c.add_games(i);before=c.image();c.repair(i);assert len(c.entries())==2;c.call('undo');assert g.exists() and bn.exists() and len(c.entries())==1
 c.repair(i);q=c.call('review_clean',id=i['id'],keep_settings=False);plan=c.plan();before=c.image();put(c.data/'cleanup.json',json.dumps({'plan':plan,'settings_saved':False}));c.call('cancel_cleanup');assert c.image()==before and g.exists() and not(c.data/'cleanup.json').exists()
 return 'Real engine Undo restored pre-repair tile set without deleting games; pending cleanup cancellation before writes preserved all target bytes. No UI Cancel/Back pass inferred.'
def running_steam_guard():
 c=Case('running-steam-guard');i=c.setup();c.add_games(i);q=c.call('review_repair',id=i['id']);before=c.image()
 code="import ctypes,time;ctypes.CDLL(None).prctl(15,b'steam',0,0,0);print('READY',flush=True);time.sleep(10)"
 proc=subprocess.Popen(['python3','-c',code],stdout=subprocess.PIPE,text=True)
 try:
  assert proc.stdout.readline().strip()=='READY';err=c.call('apply',ok=False,token=q['token']);assert 'Steam is still running' in err;assert c.image()==before
 finally:proc.terminate();proc.wait(timeout=5)
 c.call('apply',token=q['token']);return 'Real process-list guard refused target writes while a controlled child named steam existed, then completed after that child exited. Not an actual Steam-client run.'
def invalid_download_body():
 c=Case('invalid-http-body');shim=c.home/'bin/curl';put(shim,'#!/bin/sh\nwhile [ "$1" != "--output" ]; do shift; done\nshift\nprintf HTML > "$1"\nprintf "200\\n"\n');shim.chmod(0o755);c.env['PATH']=str(shim.parent)+':'+os.environ['PATH'];q=c.call('review_setup',steam=str(c.steam),user='123',proton='proton_11',location=str(c.games),editions=['era']);before=c.image();c.call('apply',ok=False,token=q['token']);assert c.image()==before;assert not (Path(c.plan()['installation']['root'])/'Battle.net-Setup.part').exists()
 return 'HTTP-200 non-PE response simulated through curl shim; real engine rejected body, removed partial file, and left target configuration unchanged.'
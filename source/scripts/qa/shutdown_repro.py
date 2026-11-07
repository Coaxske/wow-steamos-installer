from pathlib import Path
import os,sys,json,subprocess,time,threading
assert Path('/run/.containerenv').exists()
os.environ.update(HOME='/output/vmlab',A10_ENGINE_CONTAINER_FIXTURES='1')
sys.path.insert(0,'/output/private')
cli=sys.argv[1];sys.argv=['fixture',cli,'shutdown-'+str(time.time_ns())]
exec(compile(Path('/output/private/fixture_helpers.py').read_text(),'fixture_helpers','exec'))
c=Case('shutdown-helper');q=c.call('review_setup',steam=str(c.steam),user='123',proton='proton_11',location=str(c.games),editions=['era']);i=c.plan()['installation'];put(Path(i['root'])/'Battle.net-Setup.exe',pe());before=c.image();flag=c.home/'exit-request'
bin=c.home/'bin';bin.mkdir()
put(bin/'steam','#!/usr/bin/python3\nimport ctypes,pathlib\nctypes.CDLL(None).prctl(15,b"steam",0,0,0)\npathlib.Path('+repr(str(flag))+').touch()\n');(bin/'steam').chmod(0o755)
put(bin/'xdg-open','#!/bin/sh\nexit 0\n');(bin/'xdg-open').chmod(0o755);c.env['PATH']=str(bin)+':'+os.environ['PATH']
code='import ctypes,time,pathlib;ctypes.CDLL(None).prctl(15,b"steam",0,0,0);print("READY",flush=True)\nwhile not pathlib.Path('+repr(str(flag))+').exists():time.sleep(.02)'
client=subprocess.Popen(['python3','-c',code],stdout=subprocess.PIPE,text=True);assert client.stdout.readline().strip()=='READY'
thread=threading.Thread(target=client.wait);thread.start()
start=time.monotonic();proc=subprocess.Popen([str(CLI)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,env=c.env);proc.stdin.write(json.dumps(dict(action='guided_apply',token=q['token'],manage_steam=True)));proc.stdin.close()
observed=[]
while proc.poll() is None and time.monotonic()-start<40:
 for p in Path('/proc').iterdir():
  try:
   if (p/'comm').read_text().strip()=='steam':
    st=(p/'status').read_text();ppid=next(x for x in st.splitlines() if x.startswith('PPid:')).split()[1]
    if ppid==str(proc.pid):observed.append(next(x for x in st.splitlines() if x.startswith('State:')))
  except OSError:pass
 time.sleep(.02)
assert proc.poll() is not None,'Engine exceeded bound'
d=json.loads(proc.stdout.read());thread.join(timeout=5)
record=dict(cli=str(CLI),seconds=round(time.monotonic()-start,3),result=d,helper_states=sorted(set(observed)),steam_state_unchanged=c.image()==before,retained_bootstrap=Path(i['root'],'Battle.net-Setup.exe').exists(),evidence=str(OUT))
name='baseline-shutdown' if 'baseline' in str(CLI) else 'candidate-shutdown';Path('/output/private/'+name+'.json').write_text(json.dumps(record,indent=2));print(json.dumps(record,indent=2))
assert not d['ok'] if 'baseline' in str(CLI) else d['ok']

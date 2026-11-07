"""Real Rust engine, synthetic disposable files; never a GUI/game/Steam-client pass."""
from pathlib import Path
import os, sys, json, subprocess, time, hashlib, itertools, fcntl

assert Path('/run/.containerenv').exists(), 'Run only inside the documented isolated container'
os.environ.update(HOME='/output/vmlab', A10_ENGINE_CONTAINER_FIXTURES='1')
BASE = Path('/output')
sys.argv = ['fixture', str(BASE/'private/interface-target/release/wow-engine-interface'), str(time.time_ns())]
exec(compile((BASE/'private/fixture_helpers.py').read_text(), 'fixture_helpers.py', 'exec'))
COMMANDS = {'init':'status','scan':'discover','apply':'apply','recover':'recover','undo':'undo',
            'cancel_cleanup':'cancel-recovery','settings_backups':'settings-backups',
            'restore_settings':'restore-settings','adoption_candidates':'adoption-candidates'}
OPERATIONS = {'review_setup':'setup','review_repair':'repair','review_clean':'remove',
              'review_reinstall':'reinstall','review_adopt':'adopt','review_addon':'addon'}
def invoke(c, action, **params):
    command = COMMANDS.get(action, 'plan')
    if action in OPERATIONS: params = dict(operation=OPERATIONS[action], **params)
    request = dict(schema_version=1, request_id='fixture', params=params)
    p = subprocess.run([str(CLI), command], input=json.dumps(request), text=True,
                       capture_output=True, env=c.env, timeout=30)
    d = json.loads(p.stdout)
    assert d['schema_version']==1 and d.get('request_id')=='fixture', d
    assert p.returncode == (0 if d['ok'] else 3), (p.returncode,d,p.stderr)
    with (c.home.parent/'calls.jsonl').open('a') as f:
        f.write(json.dumps(dict(command=command,request=request,response=d,exit=p.returncode))+'\n')
    return d
def call(self, action, ok=True, **params):
    d=invoke(self,action,**params)
    assert d['ok']==ok,(action,d)
    return d['data'] if ok else d['error']['message']
if os.environ.get("QA_FROZEN_BASELINE") == "1":
    CLI=BASE/"frozen-cli"
else:
    Case.call=call

def complete():
    c=Case('first-complete-workflow'); original=c.image()
    q=c.call('review_setup',steam=str(c.steam),user='123',proton='proton_11',location=str(c.games),editions=['era'])
    assert c.image()==original, 'Review modified transaction targets'
    i=c.plan()['installation'];put(Path(i['root'])/'Battle.net-Setup.exe',pe())
    c.call('apply',token=q['token']);i=c.state()['installations'][0]
    assert len(c.entries())==1 and '"proton_11"' in c.cfg.read_text()
    c.add_games(i)  # Explicitly synthetic game/launcher fixture; no downloaded game.
    c.repair(i);before=c.image();ids=[v.lookup(e,'appid') for e in c.entries()]
    c.repair(i);assert c.image()==before and [v.lookup(e,'appid') for e in c.entries()]==ids
    q=c.call('review_clean',id=i['id'],keep_settings=False)
    c.call('apply',token=q['token'],confirmation='REMOVE THIS COPY')
    assert not c.state()['installations'] and not c.entries()
    return 'Review target immutability → apply → inspect state/compatibility/tiles → repair twice with stable IDs/bytes → scoped removal verified. Shared prefix support retained.'

def freshness():
    c=Case('strict-freshness');i=c.setup();c.add_games(i)
    q=c.call('review_repair',id=i['id']);before=c.image()
    assert 'expired' in c.call('apply',ok=False,token='0'*32).lower();assert c.image()==before
    put(c.cfg,c.cfg.read_text()+'\n// unrelated formatting edit after review\n');before=c.image()
    c.call('apply',token=q['token']);assert len(c.entries())==2
    c.repair(i)
    return 'Expired token refused without writes; unrelated config formatting edit safely merged by the production engine; repair remained stable.'

def protocol():
    for command,raw in [('unknown','{"schema_version":1}'),('status','{'),('status','{"schema_version":2}'),('status','{"schema_version":1,"params":{"action":"apply"}}'),('apply','x'*65537)]:
        p=subprocess.run([str(CLI),command],input=raw,text=True,capture_output=True)
        d=json.loads(p.stdout);assert p.returncode==2 and not d['ok'] and d['error']['kind']=='protocol'
    p=subprocess.run([str(CLI),'interface'],capture_output=True,text=True);assert p.returncode==0
    return 'Version, invalid JSON/command, reserved action and oversized input rejected as protocol errors (exit 2); interface introspection passed.'

def locks():
    c=Case('locking');i=c.setup();c.add_games(i);q=c.call('review_repair',id=i['id']);before=c.image()
    with (c.data/'operation.lock').open('a') as f:
        fcntl.flock(f,fcntl.LOCK_EX|fcntl.LOCK_NB)
        err=c.call('apply',ok=False,token=q['token']);assert c.image()==before
    c.call('apply',token=q['token'])
    return 'Held engine operation.lock refused apply without writes; released lock allowed same reviewed apply. '+err

def combos():
    for index,editions in enumerate([['era'],['retail'],['forever'],['retail','era'],['era','forever'],['retail','forever'],['retail','forever','era']]):
        c=Case('editions-'+str(index));q=c.call('review_setup',steam=str(c.steam),user='123',proton='proton_11',location=str(c.games),editions=editions)
        assert q['choices']['editions']==editions
        i=c.plan()['installation'];put(Path(i['root'])/'Battle.net-Setup.exe',pe());c.call('apply',token=q['token']);i=c.state()['installations'][0]
        _,g=c.add_games(i)
        if 'era' not in editions: g.unlink()  # add_games is an Era helper, not an extra requested client.
        for edition in editions:
            folder={'era':'_classic_era_','retail':'_retail_','forever':'_forever_'}[edition]
            put(g.parent.parent/folder/'Wow.exe',b'MZ')
        c.repair(i);before=c.image();c.repair(i);assert c.image()==before
        assert len(c.entries())==1+len(editions)
    return 'Seven nonempty Retail/Forever/Era selection combinations reviewed/applied/repaired twice with exact tile counts. Fake PE/client data; not product entitlement/download/launch verification.'

def settings():
    c=Case('settings-roundtrip');i=c.setup();_,g=c.add_games(i);c.repair(i)
    put(g.parent/'WTF/Config.wtf','fixture-settings');put(g.parent/'Interface/AddOns/Test/data.lua','fixture-addon')
    expected={str(p.relative_to(g.parent)):sha(p) for p in g.parent.rglob('*') if p.is_file() and p!=g}
    g.unlink()  # Candidate 6 missing-executable regression exercised through CLI.
    q=c.call('review_reinstall',id=i['id'],location=str(c.games),editions=['era'],keep_settings=True,addon_manager='none',launcher_tile=False)
    replacement=c.plan()['replacement'];put(Path(replacement['root'])/'Battle.net-Setup.exe',pe())
    c.call('apply',token=q['token'],confirmation='REMOVE THIS COPY');i=c.state()['installations'][0]
    assert i['addon_manager']=='none' and i['launcher_tile'] is False
    backup=q['token'];assert (c.data/'settings-backups'/backup/'.inventory.json').exists()
    _,newg=c.add_games(i);c.call('restore_settings',id=i['id'],backup=backup)
    for relative,digest in expected.items():assert sha(newg.parent/relative)==digest
    before=c.image();assert 'already exists' in c.call('restore_settings',ok=False,id=i['id'],backup=backup);assert c.image()==before
    return 'Missing executable did not lose WTF/Interface; reinstall changed choices applied, inventory backup retained, restore matched file hashes, overwrite refused.'

def cross_root():
    c=Case('cross-root-resume');i=c.setup();bn,g=c.add_games(i);c.repair(i)
    q=c.call('review_clean',id=i['id'],keep_settings=False)
    put(c.data/'cleanup.json',json.dumps(dict(plan=c.plan(),settings_saved=False)))
    root=c.home/'.steam/steam';foreign=root/'userdata/456/config/shortcuts.vdf';put(root/'steamapps/.fixture','root')
    es=entry(0x8f001122,'Shared other root',bn,"STEAM_COMPAT_DATA_PATH='"+i['prefix']+"' %command%")
    put(foreign,v.encode([('shortcuts',0,[('0',0,es)])]));before=c.image();digest=sha(foreign)
    assert 'Another Steam root' in c.call('recover',ok=False);assert c.image()==before and sha(foreign)==digest and bn.exists() and g.exists()
    return 'Reference introduced in second discovered Steam root after synthetic pending cleanup checkpoint refused recovery before deletion; all targets retained.'

def ownership():
    c=Case('ownership-refusal');i=c.setup();c.add_games(i);c.repair(i);marker=Path(i['root'])/'.wow-rust-owner.json'
    state=json.loads(marker.read_text());state['id']='0'*32;put(marker,json.dumps(state));before=c.image()
    assert 'ownership changed' in c.call('review_clean',ok=False,id=i['id']);assert c.image()==before
    return 'Changed ownership marker refused removal review without changing Steam/state/game files.'

def choices_omitted():
    c=Case('reinstall-omitted-options');i=c.setup();c.add_games(i);c.repair(i);state=c.state()
    state['installations'][0].update(addon_manager='wowup_cf',launcher_tile=False);put(c.data/'state.json',json.dumps(state))
    q=c.call('review_reinstall',id=i['id'],location=str(c.games),editions=['era'],keep_settings=False)
    r=c.plan()['replacement'];assert q['choices']['addon_manager']=='wowup_cf' and q['choices']['launcher_tile'] is False
    put(Path(r['root'])/'Battle.net-Setup.exe',pe());c.call('apply',token=q['token'],confirmation='REMOVE THIS COPY')
    actual=c.state()['installations'][0];assert actual['addon_manager']=='wowup_cf' and actual['launcher_tile'] is False
    return 'Omitted reinstall choices retain selected addon manager and disabled launcher tile through review and execution.'


def terminated_download():
    c=Case('actual-process-interruption');marker=c.home/'transport-started';shim=c.home/'bin/curl'
    put(shim,'#!/bin/sh\nwhile [ "$1" != "--output" ]; do shift; done\nshift\nprintf partial > "$1"\ntouch "'+str(marker)+'"\nsleep 1\nexit 22\n');shim.chmod(0o755);c.env['PATH']=str(shim.parent)+':'+os.environ['PATH']
    q=c.call('review_setup',steam=str(c.steam),user='123',proton='proton_11',location=str(c.games),editions=['era']);i=c.plan()['installation'];before=c.image()
    p=subprocess.Popen([str(CLI),'apply'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,env=c.env)
    p.stdin.write(json.dumps(dict(schema_version=1,params=dict(token=q['token']))));p.stdin.close()
    deadline=time.monotonic()+5
    while not marker.exists() and p.poll() is None and time.monotonic()<deadline:time.sleep(.02)
    assert marker.exists(),'Download shim never became ready'
    p.terminate();p.wait(timeout=5);time.sleep(1.2)
    assert c.image()==before and not(c.data/'transaction.json').exists()
    put(Path(i['root'])/'Battle.net-Setup.exe',pe());c.call('apply',token=q['token'])
    assert len(c.state()['installations'])==1
    return 'Versioned CLI process terminated after observable local transport readiness; no Steam/state commit, same-plan cached-fixture retry completed. No real Blizzard transport.'

def during_download():
    import shlex
    c=Case('production-download-merge');cached=c.home/'valid-pe';put(cached,pe());shim=c.home/'bin/curl'
    script = "#!/bin/sh\nwhile [ \"$1\" != \"--output\" ]; do shift; done\nshift\n"
    script += "cp " + shlex.quote(str(cached)) + ' "$1"\n'
    script += "printf '%s\\n' '\"ExternalSentinel\" \"keep\"' >> " + shlex.quote(str(c.cfg)) + "\nprintf '200\\n'\n"
    put(shim,script);shim.chmod(0o755);c.env['PATH']=str(shim.parent)+':'+os.environ['PATH']
    q=c.call('review_setup',steam=str(c.steam),user='123',proton='proton_11',location=str(c.games),editions=['era'])
    c.call('apply',token=q['token']);assert 'ExternalSentinel' in c.cfg.read_text() and 'keep' in c.cfg.read_text()
    assert len(c.state()['installations'])==1
    return 'Production safe merge retained unrelated semantic config entry introduced during fake download; setup committed. No QA strict guard.'

ONLY=set(filter(None,os.environ.get('QA_ONLY','').split(',')))
if not ONLY or '01-complete-workflow' in ONLY: run('01-complete-workflow',complete)
if os.environ.get('QA_FROZEN_BASELINE') == '1':
    (BASE/'private/frozen-baseline-results.json').write_text(json.dumps(dict(results=results,evidence=str(OUT)),indent=2))
    sys.exit(results[-1]['status']!='PASS')
if results and results[-1]['status']!='PASS':sys.exit(1)
for name,fn in [('protocol',protocol),('production-safe-merge',freshness),('operation-lock',locks),('edition-combinations',combos),('custom-identity-repair',repeated),('legacy-adoption',python_adopt),('settings-roundtrip',settings),('reinstall-omitted',choices_omitted),('cross-root-resume-refusal',cross_root),('ownership-refusal',ownership),('scoped-removal-confirmation',removal),('repair-interruption-conflict',lambda:interrupted('repair',True)),('install-interruption',lambda:interrupted('install')),('failed-download-retry',download_failure),('invalid-download-body',invalid_download_body),('steam-running-refusal',running_steam_guard),('discovery-moved-missing',discovery),('undo-cancel',cancel_undo),('actual-cli-interruption',terminated_download),('during-download-safe-merge',during_download)]:
    if not ONLY or name in ONLY: run(name,fn)
record=dict(method='Real Rust engine through CLI v1; synthetic disposable container fixtures only',results=results,evidence=str(OUT))
(BASE/'private'/('results-'+str(time.time_ns())+'.json')).write_text(json.dumps(record,indent=2)+'\n')
(BASE/'private/latest-results.json').write_text(json.dumps(record,indent=2)+'\n')
print(json.dumps(record,indent=2))
sys.exit(any(r['status']!='PASS' for r in results))

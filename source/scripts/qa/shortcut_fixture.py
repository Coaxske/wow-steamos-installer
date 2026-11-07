#!/usr/bin/env python3
"""Guest-only unrelated shortcut sentinel; never prints profile/account information."""
import hashlib,json,os
from pathlib import Path
import struct,subprocess,sys,zlib
TITLE='VM Lab Unrelated Dummy'
HOME=Path.home()
DATA=HOME/'vm-lab/unrelated-fixture'


def parse(data):
    assert len(data)<32*1024*1024
    i=0
    def text():
        nonlocal i
        end=data.index(b'\0',i);s=data[i:end].decode();i=end+1;return s
    def obj(depth=0):
        nonlocal i
        assert depth<64
        out=[]
        while True:
            tag=data[i];i+=1
            if tag==8:return out
            key=text()
            if tag==0:value=obj(depth+1)
            elif tag==1:value=text()
            elif tag in (2,3,4,6,7,10):
                size=8 if tag in (7,10) else 4
                value=data[i:i+size];i+=size;assert len(value)==size
            else:raise ValueError('Unsupported VDF type; no writes')
            out.append((key,tag,value))
            assert len(out)<100000
    result=obj();assert i==len(data);return result


def encode(obj):
    b=bytearray()
    for key,tag,value in obj:
        b.append(tag);b.extend(key.encode()+b'\0')
        b.extend(encode(value) if tag==0 else value.encode()+b'\0' if tag==1 else value)
    b.append(8);return bytes(b)


def lookup(obj,key):
    return next(v for k,t,v in obj if k.lower()==key.lower())


def source():
    root=HOME/'.local/share/Steam'
    candidates=[p/'config/shortcuts.vdf' for p in (root/'userdata').iterdir() if p.name.isdigit() and p.name!='0' and (p/'config').is_dir()]
    assert len(candidates)==1,'Expected exactly one guest profile; no edits performed'
    return candidates[0]


def fingerprint(entry):return hashlib.sha256(encode(entry)).hexdigest()


def main():
    assert subprocess.check_output(['systemd-detect-virt'],text=True).strip()=='kvm'
    assert HOME.name=='vmlab','Guest lab account required'
    assert b'ostree=' in Path('/proc/cmdline').read_bytes()
    assert subprocess.run(['pgrep','-x','steam'],stdout=subprocess.DEVNULL).returncode!=0,'Fully exit guest Steam first'
    path=source();before=path.read_bytes() if path.exists() else None
    doc=parse(before) if before else [('shortcuts',0,[])]
    entries=lookup(doc,'shortcuts')
    mode=sys.argv[1]
    if mode=='create':
        assert not entries,'Fixture creation requires empty guest shortcuts; nothing overwritten'
        DATA.mkdir(parents=True,exist_ok=True)
        runner=DATA/'run.sh';runner.write_text('#!/bin/sh\nprintf "Disposable unrelated fixture\\n"\n');runner.chmod(0o755)
        target='"'+str(runner)+'"'
        ident=zlib.crc32((target+TITLE).encode())|0x80000000
        entry=[('appid',2,struct.pack('<I',ident)),('appname',1,TITLE),('exe',1,target),('StartDir',1,'"'+str(DATA)+'"'),('icon',1,'vm-lab-artwork-sentinel'),('ShortcutPath',1,''),('LaunchOptions',1,'--unrelated-fixture'),('IsHidden',2,struct.pack('<I',1)),('AllowDesktopConfig',2,struct.pack('<I',1)),('AllowOverlay',2,struct.pack('<I',1)),('OpenVR',2,struct.pack('<I',0)),('VmLabPreserve',1,'retain-this-field'),('tags',0,[('0',1,'VM lab unrelated fixture')])]
        entries.append(('0',0,entry))
        expected={'entry_sha256':fingerprint(entry),'file_sha256':hashlib.sha256((DATA/'keep.txt').read_bytes()).hexdigest()}
        (DATA/'expected.json').write_text(json.dumps(expected))
        if before is not None:(DATA/'original-shortcuts.vdf').write_bytes(before)
        tmp=path.with_suffix('.vm-lab-new');tmp.write_bytes(encode(doc));tmp.replace(path)
        print('UNRELATED_DUMMY_SHORTCUT_CREATED')
    elif mode=='verify':
        expected=json.loads((DATA/'expected.json').read_text())
        found=[entry for _,tag,entry in entries if tag==0 and lookup(entry,'appname')==TITLE]
        assert len(found)==1 and fingerprint(found[0])==expected['entry_sha256'],'Unrelated tile fields changed'
        assert hashlib.sha256((DATA/'keep.txt').read_bytes()).hexdigest()==expected['file_sha256'],'Unrelated file changed'
        print('UNRELATED_SHORTCUT_AND_FILE_PRESERVED')
    else:raise ValueError('Unsupported fixture operation')
    print('Shortcut entry count:',len(entries))


if __name__=='__main__':main()

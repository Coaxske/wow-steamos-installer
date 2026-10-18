import io
import json
import os
from pathlib import Path
import struct
import tempfile
import unittest
from unittest.mock import patch
from wowsteam import vdf
from wowsteam.cli import main, prompt, exclusive, register
from wowsteam.manager import Manager
from wowsteam.system import (SetupError, allowed_download, atomic_write, busy_processes,
    catalog, discover, game_exe, parse_selection, pe_file, safe_path, download_installer)

CONFIG = '"InstallConfigStore" { "Software" { "Valve" { "Steam" { "Other" "keep" } } } }\n'

def pe(path):
    path.parent.mkdir(parents=True, exist_ok=True)
    data = bytearray(1024)
    data[:2] = b'MZ'
    struct.pack_into('<I', data, 60, 128)
    data[128:132] = b'PE\0\0'
    path.write_bytes(data)
    return path

class VdfTests(unittest.TestCase):
    def test_binary_roundtrip(self):
        data = [[vdf.T_MAP, 'shortcuts', [[vdf.T_MAP, '0', [
            [vdf.T_STR, 'AppName', 'Existing 世界'], [vdf.T_INT, 'appid', -23],
            [vdf.T_FLOAT, 'float', 1.25], [vdf.T_UINT64, 'large', 2**63],
            [vdf.T_INT64, 'signed', -2**45], [vdf.T_MAP, 'tags', []]]]]]]
        encoded = vdf.bvdf_dumps(data)
        self.assertEqual(encoded, vdf.bvdf_dumps(vdf.bvdf_loads(encoded)))

    def test_unknown_truncated_and_trailing_binary_refused(self):
        for b in (b'\x99x\0', b'\x02x\0\x01', b'\x01bad', b'\x00map\0', b'\x08extra'):
            with self.subTest(b=b), self.assertRaises(vdf.VdfError): vdf.bvdf_loads(b)

    def test_compact_mapping_preserves_neighbors(self):
        text = '"InstallConfigStore" { "Software" { "Valve" { "Steam" { "CompatToolMapping" { "123" {"name" "old"} "456" {"name" "keep"} } "other" "keep2" } } } }'
        updated = vdf.set_compat_tool(text, '123', 'proton_experimental')
        self.assertEqual(vdf.get_compat_tool(updated, '456'), 'keep')
        self.assertIn('"other" "keep2"', updated)
        removed = vdf.remove_compat_tool(updated, '123')
        self.assertEqual(vdf.get_compat_tool(removed, '456'), 'keep')
        vdf.vdf_parse_tree(removed)

    def test_mapping_preserves_comments_and_idempotent(self):
        updated = vdf.set_compat_tool('// comment\n'+CONFIG, '42', 'proton_10')
        self.assertEqual(updated, vdf.set_compat_tool(updated, '42', 'proton_10'))
        self.assertTrue(updated.startswith('// comment'))

    def test_invalid_text_refused(self):
        for s in ('"key" {', '"key"', '}', '"key" "unfinished', '[UNFINISHED'):
            with self.subTest(s=s), self.assertRaises(vdf.VdfError): vdf.vdf_parse_tree(s)

class HostTests(unittest.TestCase):
    def test_catalog_selection(self):
        self.assertEqual([x['id'] for x in parse_selection('1,forever,1 era')], ['retail','forever','era'])
        self.assertEqual(len({x['id'] for x in catalog()}), len(catalog()))
        with self.assertRaises(SetupError): parse_selection('wow-not-real')

    def test_no_fictional_hardcore_download(self):
        ids = {x['id'] for x in catalog()}
        self.assertNotIn('hardcore', ids)
        self.assertNotIn('sod', ids)

    def test_shell_metacharacters_refused(self):
        for s in ('/tmp/x$(touch nope)', '/tmp/"x', '/tmp/`x`', '/tmp/%command%', '/tmp/x\ny'):
            with self.subTest(s=s), self.assertRaises(SetupError): safe_path(s)
        self.assertEqual(str(safe_path('/tmp/My Games')), '/tmp/My Games')

    def test_redirect_allowlist(self):
        self.assertTrue(allowed_download('https://download.battle.net/a'))
        for url in ('http://battle.net/a', 'https://battle.net.evil.test/a', 'file:///a', 'https://evil.test/?battle.net'):
            self.assertFalse(allowed_download(url))

    def test_pe_and_discovery(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            wanted = pe(root / 'World of Warcraft/_classic_beta_/WowB.exe')
            pe(root / 'World of Warcraft/_retail_/Wow.exe')
            pe(root / 'World of Warcraft/_classic_beta_/World of Warcraft Launcher.exe')
            pe(root / 'Data/_classic_beta_/WowB.exe')
            (root / 'fake.exe').write_text('MZ is not enough')
            self.assertFalse(pe_file(root / 'fake.exe'))
            self.assertEqual(discover([root,root], ['_classic_beta_']), [wanted])
            self.assertTrue(game_exe(wanted))

    def test_symlink_tree_not_followed(self):
        with tempfile.TemporaryDirectory() as d:
            p = Path(d)
            pe(p/'outside/_retail_/Wow.exe')
            (p/'scan').mkdir()
            (p/'scan/link').symlink_to(p/'outside', target_is_directory=True)
            self.assertEqual(discover([p/'scan'], ['_retail_']), [])

    def test_process_detection(self):
        with tempfile.TemporaryDirectory() as d:
            p = Path(d)/'123'
            p.mkdir()
            (p/'comm').write_text('steamwebhelper\n')
            (p/'cmdline').write_bytes(b'/usr/bin/steamwebhelper\0')
            self.assertIn('Steam', busy_processes(Path(d)))
            (p/'comm').write_text('wine\n')
            (p/'cmdline').write_bytes(b'wine\0C:\\Games\\_retail_\\Wow.exe\0')
            self.assertIn('WoW', busy_processes(Path(d)))

    def test_eof_not_approval(self):
        with patch('builtins.input', side_effect=EOFError), self.assertRaises(SetupError): prompt('Proceed?')

    def test_atomic_write_refuses_symlink(self):
        with tempfile.TemporaryDirectory() as d:
            p = Path(d)
            (p/'real').write_text('keep')
            (p/'link').symlink_to(p/'real')
            with self.assertRaises(SetupError): atomic_write(p/'link', b'replace')
            self.assertEqual((p/'real').read_text(), 'keep')

    def test_plan_creates_no_data(self):
        with tempfile.TemporaryDirectory() as d, patch('sys.stdout', new_callable=io.StringIO):
            path = Path(d)/'not-created'
            self.assertEqual(main(['plan','--root',str(path),'--editions','forever,retail']),0)
            self.assertFalse(path.exists())

    def test_second_instance_refused(self):
        with tempfile.TemporaryDirectory() as d:
            with exclusive(Path(d)), self.assertRaises(SetupError), exclusive(Path(d)): pass

    def test_bad_download_preserves_existing(self):
        class Response(io.BytesIO):
            def geturl(self): return 'https://download.battle.net/file'
        with tempfile.TemporaryDirectory() as d:
            p=Path(d)/'installer.exe'
            p.write_bytes(b'original')
            with patch('urllib.request.build_opener') as op:
                op.return_value.open.return_value=Response(b'<html>bad</html>')
                with self.assertRaises(SetupError): download_installer(p)
            self.assertEqual(p.read_bytes(),b'original')
            self.assertEqual([x.name for x in p.parent.iterdir()],['installer.exe'])

class ManagerTests(unittest.TestCase):
    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory()
        self.base=Path(self.tmp.name)
        self.steam=self.base/'Steam'
        (self.steam/'config').mkdir(parents=True)
        (self.steam/'config/config.vdf').write_text(CONFIG)
        self.m=Manager(self.base/'managed',self.steam,'123')
        self.exe=pe(self.base/'games/_retail_/Wow.exe')
        self.spec=('retail','WoW Retail',self.exe)
        idle=patch('wowsteam.manager.require_idle'); idle.start()
        self.addCleanup(idle.stop); self.addCleanup(self.tmp.cleanup)

    def apply(self): return self.m.update([self.spec])

    def test_repeat_keeps_id_artwork_and_rename(self):
        self.apply()
        appid=self.m.load()['tiles']['retail']['appid']
        sc=vdf.Shortcuts(self.m.shortcuts_file); fields=sc.find_by_appid(appid)
        vdf.kv_set(fields,vdf.T_STR,'icon','/artwork/custom.png')
        vdf.kv_set(fields,vdf.T_STR,'AppName','My renamed WoW'); sc.save()
        self.apply()
        self.assertEqual(appid,self.m.load()['tiles']['retail']['appid'])
        sc=vdf.Shortcuts(self.m.shortcuts_file)
        self.assertEqual(len(sc.entries),1)
        self.assertEqual(vdf.kv_get(sc.find_by_appid(appid),'icon')[2],'/artwork/custom.png')
        self.assertEqual(vdf.kv_get(sc.find_by_appid(appid),'AppName')[2],'My renamed WoW')

    def test_unrelated_same_name_not_adopted(self):
        sc=vdf.Shortcuts(self.m.shortcuts_file)
        fields=vdf.make_shortcut_fields(2147483650,'WoW Retail','"/other.exe"','"/"','keep')
        sc.entries.append([vdf.T_MAP,'0',fields]); sc.save(); self.apply()
        result=vdf.Shortcuts(self.m.shortcuts_file)
        self.assertEqual(len(result.entries),2)
        self.assertEqual(result.find_by_appid(2147483650),fields)

    def test_external_edit_not_overwritten(self):
        self.apply(); sc=vdf.Shortcuts(self.m.shortcuts_file)
        fields=sc.find_by_appid(self.m.load()['tiles']['retail']['appid'])
        vdf.kv_set(fields,vdf.T_STR,'LaunchOptions','user changes'); sc.save()
        before=self.m.shortcuts_file.read_bytes()
        with self.assertRaises(SetupError): self.apply()
        with self.assertRaises(SetupError): self.m.update([],remove=True)
        self.assertEqual(self.m.shortcuts_file.read_bytes(),before)

    def test_shared_prefix_consistent(self):
        bnet=pe(self.base/'bnet/Battle.net.exe')
        self.m.update([self.spec,('bnet','Battle.net',bnet)])
        self.assertEqual(len({r['signature']['LaunchOptions'] for r in self.m.load()['tiles'].values()}),1)

    def test_unknown_vdf_leaves_files_untouched(self):
        self.m.shortcuts_file.parent.mkdir(parents=True)
        self.m.shortcuts_file.write_bytes(b'\x99bad\0')
        before=self.m.config_file.read_bytes()
        with self.assertRaises(vdf.VdfError): self.apply()
        self.assertEqual(self.m.config_file.read_bytes(),before)
        self.assertFalse(self.m.state_file.exists())

    def test_corrupt_state_not_replaced(self):
        self.m.root.mkdir(); self.m.state_file.write_text('{')
        with self.assertRaises(SetupError): self.apply()
        self.assertEqual(self.m.state_file.read_text(),'{')

    def test_remove_keeps_games_and_other_mapping(self):
        self.apply()
        self.m.config_file.write_text(vdf.set_compat_tool(self.m.config_file.read_text(),'17','proton_10'))
        self.m.update([],remove=True)
        self.assertTrue(self.exe.exists())
        self.assertEqual(len(vdf.Shortcuts(self.m.shortcuts_file).entries),0)
        self.assertEqual(vdf.get_compat_tool(self.m.config_file.read_text(),'17'),'proton_10')

    def interrupted(self):
        real=atomic_write
        def fail(path,data):
            if path==self.m.config_file: raise OSError('simulated disk failure')
            real(path,data)
        with patch('wowsteam.manager.atomic_write',side_effect=fail), self.assertRaises(OSError): self.apply()

    def test_interrupted_commit_recovers(self):
        original=self.m.config_file.read_bytes(); self.interrupted()
        self.assertTrue(self.m.pending.exists())
        with self.assertRaises(SetupError): self.apply()
        self.assertTrue(self.m.recover())
        self.assertFalse(self.m.shortcuts_file.exists()); self.assertFalse(self.m.state_file.exists())
        self.assertEqual(self.m.config_file.read_bytes(),original)

    def test_recovery_refuses_later_edits(self):
        self.interrupted(); self.m.config_file.write_text(CONFIG+'// later edits')
        with self.assertRaises(SetupError): self.m.recover()
        self.assertIn('later edits',self.m.config_file.read_text())

    def test_running_steam_prevents_write(self):
        with patch('wowsteam.manager.require_idle',side_effect=SetupError('Steam running')), self.assertRaises(SetupError): self.apply()
        self.assertFalse(self.m.state_file.exists())

    def test_backup_original_bytes(self):
        original=self.m.config_file.read_bytes(); backup=self.apply()
        self.assertEqual((backup/'1.bin').read_bytes(),original)
        self.assertFalse(self.m.pending.exists())

    def test_proton_not_changed_silently(self):
        self.apply()
        with self.assertRaises(SetupError): self.m.update([self.spec],{'proton':'different'})

    def test_missing_target_not_registered(self):
        with self.assertRaises(SetupError): self.m.update([('x','Missing',self.base/'missing.exe')])
        self.assertFalse(self.m.state_file.exists())

    def test_missing_owned_tile_recreated_with_same_id(self):
        self.apply(); appid=self.m.load()['tiles']['retail']['appid']
        sc=vdf.Shortcuts(self.m.shortcuts_file); sc.remove(appid); sc.save(); self.apply()
        self.assertEqual(self.m.load()['tiles']['retail']['appid'],appid)

    def test_duplicate_client_selection_registers_once(self):
        items=[catalog()[0],catalog()[1]]
        with patch('wowsteam.cli.select_exe',return_value=self.exe), patch('wowsteam.cli.yes',return_value=True), patch('wowsteam.cli.wait_for_exit'), patch('sys.stdout',new_callable=io.StringIO):
            register(self.m,items)
        self.assertEqual(len(self.m.load()['tiles']),1)


class WizardIntegrationTests(unittest.TestCase):
    def test_bootstrap_launcher_retarget_and_two_clients(self):
        from types import SimpleNamespace
        from wowsteam.cli import setup
        with tempfile.TemporaryDirectory() as d:
            base=Path(d)
            steam=base/'Steam'
            (steam/'config').mkdir(parents=True)
            (steam/'config/config.vdf').write_text(CONFIG)
            m=Manager(base/'Managed Games',steam,'123')
            installer=pe(base/'Battle.net-Setup.exe')
            args=SimpleNamespace(prefix=None,games=None,proton='proton_experimental',installer=str(installer))
            selected=[catalog()[0],catalog()[1]]
            launched=[]
            clients=[]
            def open_fake(manager,key):
                record=manager.load()['tiles'][key]
                launched.append((record['appid'],record['exe']))
                if len(launched)==1:
                    pe(m.root/'prefix/pfx/drive_c/Program Files (x86)/Battle.net/Battle.net Launcher.exe')
                else:
                    clients.extend([pe(m.root/'games/World of Warcraft/_retail_/Wow.exe'),pe(m.root/'games/World of Warcraft/_classic_beta_/WowB.exe')])
            def pick(item,roots):
                return discover(roots,item['folders'])[0]
            with patch('wowsteam.cli.yes',return_value=True), patch('wowsteam.cli.wait_for_exit'), patch('wowsteam.cli.open_tile',side_effect=open_fake), patch('wowsteam.cli.select_exe',side_effect=pick), patch('wowsteam.manager.require_idle'), patch('sys.stdout',new_callable=io.StringIO):
                setup(m,args,selected)
            state=m.load()
            self.assertEqual(set(state['tiles']),{'bnet','retail','forever'})
            self.assertEqual(launched[0][0],launched[1][0])
            self.assertEqual(launched[0][1],str(installer))
            self.assertTrue(launched[1][1].endswith('Battle.net Launcher.exe'))
            self.assertEqual(state['tiles']['retail']['exe'],str(clients[0]))
            self.assertEqual(state['tiles']['forever']['exe'],str(clients[1]))
            before=state
            with patch('wowsteam.cli.yes',side_effect=[True,False,True]), patch('wowsteam.cli.wait_for_exit'), patch('wowsteam.cli.select_exe',side_effect=pick), patch('wowsteam.manager.require_idle'), patch('sys.stdout',new_callable=io.StringIO):
                setup(m,args,selected)
            self.assertEqual({k:r['appid'] for k,r in before['tiles'].items()},{k:r['appid'] for k,r in m.load()['tiles'].items()})

if __name__=='__main__': unittest.main()

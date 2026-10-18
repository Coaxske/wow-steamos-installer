import hashlib
import os
from pathlib import Path
import shlex
import subprocess
import sys
import tempfile
import unittest
import zipfile

from scripts.package import ROOT, VERSION, build


class PackageTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.workspace = tempfile.TemporaryDirectory(prefix='installer package ')
        cls.directory = Path(cls.workspace.name)
        cls.artifacts = build(output=cls.directory / "download folder's files")
        cls.source, cls.app, cls.desktop = cls.artifacts

    @classmethod
    def tearDownClass(cls):
        cls.workspace.cleanup()

    def run_app(self, arguments):
        env = dict(os.environ, HOME=str(self.directory), PYTHONPATH='')
        return subprocess.run(arguments, cwd=self.directory, env=env,
                              text=True, capture_output=True, timeout=20)

    def desktop_command(self, target):
        line = next(s for s in self.desktop.read_text().splitlines() if s.startswith('Exec='))
        # The generated entry uses no desktop escapes. shlex models the quoted
        # argument grouping; %k is substituted as a single argument, never a shell.
        return [target if value == '%k' else value for value in shlex.split(line[5:])]

    def test_zipapp_runs_away_from_source_and_reads_catalog(self):
        result = self.run_app([sys.executable, str(self.app), 'plan', '--editions', 'retail,era'])
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('retail:', result.stdout)
        self.assertIn('era:', result.stdout)
        self.assertFalse((self.directory / 'Games').exists())
        self.assertTrue(self.app.stat().st_mode & 0o111)

    def test_actual_desktop_exec_accepts_file_uri_and_space_paths(self):
        for target in (str(self.desktop), self.desktop.as_uri()):
            with self.subTest(target=target):
                result = self.run_app(self.desktop_command(target) + ['list-editions'])
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertIn('forever:', result.stdout)
                self.assertIn('retail:', result.stdout)

    def test_embedded_payload_corruption_stops_before_running(self):
        original = self.desktop.read_text()
        changed = original.replace("#>    'IyEv", "#>    'AyEv", 1)
        self.assertNotEqual(original, changed)
        damaged = self.desktop.with_name('damaged.desktop')
        damaged.write_text(changed)
        result = self.run_app(self.desktop_command(str(damaged)) + ['plan'])
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('Damaged embedded installer', result.stderr)

    def test_artifacts_are_reproducible_and_contain_only_source(self):
        again = build(output=self.directory / 'second build')
        for original, second in zip(self.artifacts, again):
            self.assertEqual(original.read_bytes(), second.read_bytes())
        sums = (self.app.parent / 'SHA256SUMS').read_text()
        for artifact in self.artifacts:
            self.assertIn(hashlib.sha256(artifact.read_bytes()).hexdigest() + '  ' + artifact.name, sums)
        with zipfile.ZipFile(self.source) as archive:
            names = archive.namelist()
            self.assertFalse(any('/.git/' in n or 'privacy-reset' in n or '__pycache__' in n for n in names))
        with zipfile.ZipFile(self.app) as archive:
            self.assertIn('LICENSE', archive.namelist())
            self.assertIn('wowsteam/catalog.json', archive.namelist())
            self.assertFalse(any(n.startswith(('tests/', 'docs/', '.github/')) for n in archive.namelist()))


if __name__ == '__main__':
    unittest.main()

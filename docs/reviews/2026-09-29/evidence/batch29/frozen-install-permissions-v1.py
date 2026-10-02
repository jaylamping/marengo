"""Actual installer permission contract, restricted to a disposable container."""

import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


class InstallPermissions(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        if os.geteuid() != 0 or not Path('/.dockerenv').is_file():
            raise RuntimeError('Run as root only inside a disposable Docker container')
        for executable in ['useradd', 'runuser', 'visudo', 'rsync']:
            if not shutil.which(executable):
                raise RuntimeError(f'required test prerequisite missing: {executable}')
        cls.workspace = tempfile.TemporaryDirectory(prefix='marengo-install-permissions-')
        cls.root = Path(cls.workspace.name)
        cls.root.chmod(0o755)
        source = Path(__file__).resolve().parent.parent
        bundle = cls.root / 'bundle'
        shutil.copytree(source / 'scripts', bundle / 'scripts')
        shutil.copytree(source / 'config', bundle / 'config')
        (bundle / 'assets').mkdir()
        binaries = bundle / 'target/release'
        binaries.mkdir(parents=True)
        shutil.copyfile('/bin/true', binaries / 'marengo-pi')
        (bundle / '.deploy-rev').write_text('adef857 fixture\n')
        cls.runtime = 'marengo-install-fixture'
        cls.deploy = 'marengo-deploy-fixture'
        subprocess.run(['useradd', '--system', cls.deploy], check=True)
        fake_bin = cls.root / 'fake-bin'
        fake_bin.mkdir()
        # Process/service controls are the substituted boundary. Filesystem,
        # ownership, accounts, rsync and sudoers validation remain real.
        for name in ['systemctl', 'pkill']:
            path = fake_bin / name
            path.write_text('#!/bin/sh\nexit 0\n')
            path.chmod(0o755)
        cls.install = cls.root / 'installed'
        environment = dict(os.environ, MARENGO_INSTALL_ROOT=str(cls.install),
                           MARENGO_USER=cls.runtime, MARENGO_DEPLOY_USER=cls.deploy,
                           PATH=f'{fake_bin}:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin')
        result = subprocess.run(['bash', str(bundle / 'scripts/install-pi.sh')],
                                env=environment, text=True, capture_output=True, timeout=60)
        if result.returncode:
            raise AssertionError(f'actual installer failed: {result.stdout}\n{result.stderr}')
        cls.install_stdout = result.stdout
        policy = Path(f'/etc/sudoers.d/marengo-{cls.runtime}-restart').read_text()
        cls.helpers = []
        for line in policy.splitlines():
            if 'NOPASSWD:' in line:
                helper = Path(line.split('NOPASSWD:', 1)[1].strip().split()[0])
                if helper not in cls.helpers:
                    cls.helpers.append(helper)
        if len(cls.helpers) != 2:
            raise AssertionError(f'expected actual restart/enqueue sudo targets, got {policy}')

    @classmethod
    def tearDownClass(cls):
        cls.workspace.cleanup()

    def as_runtime(self, script, *arguments):
        return subprocess.run(['runuser', '-u', self.runtime, '--', 'python3', '-c', script,
                               *map(str, arguments)], text=True, capture_output=True, timeout=10)

    def test_runtime_cannot_replace_actual_sudo_targets(self):
        for helper in self.helpers:
            with self.subTest(helper=helper):
                # Preserve any vulnerable renamed inode for diagnosis; no
                # replacement is executed, and no physical sudo helper runs.
                result = self.as_runtime(
                    'import os,sys; os.rename(sys.argv[1],sys.argv[1]+".test-renamed")', helper)
                self.assertNotEqual(result.returncode, 0,
                                    f'runtime replaced actual sudo target {helper}')
                self.assertTrue(helper.is_file())

    def test_runtime_cannot_replace_release_code_or_redirect_ancestors(self):
        for path in [self.install / 'bin/marengo-pi', self.install / 'scripts', self.install]:
            with self.subTest(path=path):
                result = self.as_runtime(
                    'import os,sys; os.rename(sys.argv[1],sys.argv[1]+".test-renamed")', path)
                self.assertNotEqual(result.returncode, 0,
                                    f'runtime replaced installed code/ancestor {path}')

    def test_runtime_can_write_only_declared_config_and_state(self):
        for directory in ['config', 'var/calibration', 'var/log']:
            with self.subTest(directory=directory):
                path = self.install / directory / 'permission-test.txt'
                result = self.as_runtime(
                    'import pathlib,sys; pathlib.Path(sys.argv[1]).write_text("allowed state")', path)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(path.read_text(), 'allowed state')
        denied = self.as_runtime(
            'import pathlib,sys; pathlib.Path(sys.argv[1]).write_text("replacement")',
            self.install / 'bin/extra-code')
        self.assertNotEqual(denied.returncode, 0, 'runtime wrote new installed executable')


if __name__ == '__main__':
    unittest.main(verbosity=2)

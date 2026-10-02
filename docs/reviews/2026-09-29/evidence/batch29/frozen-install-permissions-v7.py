"""Actual installer permission contract, restricted to a disposable container."""

import os
import json
from pathlib import Path
import pwd
import shutil
import subprocess
import tempfile
import time
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
        if baseline := os.environ.get('MARENGO_INSTALL_TEST_BASELINE'):
            shutil.copyfile(baseline, bundle / 'scripts/install-pi.sh')
        if baseline := os.environ.get('MARENGO_ENQUEUE_TEST_BASELINE'):
            shutil.copyfile(baseline, bundle / 'scripts/pi-enqueue-self-update.sh')
        shutil.copytree(source / 'config', bundle / 'config')
        (bundle / 'assets').mkdir()
        binaries = bundle / 'target/release'
        binaries.mkdir(parents=True)
        shutil.copyfile('/bin/true', binaries / 'marengo-pi')
        (bundle / '.deploy-rev').write_text('adef857 fixture\n')
        # Match the production runtime group used by the old enqueue helper.
        cls.runtime = 'marengo'
        cls.deploy = 'marengo-deploy-fixture'
        subprocess.run(['useradd', '--system', cls.deploy], check=True)
        fake_bin = cls.root / 'fake-bin'
        cls.fake_bin = fake_bin
        cls.bundle = bundle
        fake_bin.mkdir()
        # Process/service controls are the substituted boundary. Filesystem,
        # ownership, accounts, rsync and sudoers validation remain real.
        for name in ['systemctl', 'pkill']:
            path = fake_bin / name
            path.write_text('#!/bin/sh\nexit 0\n')
            path.chmod(0o755)
        (fake_bin / 'systemctl').write_text(
            '#!/bin/sh\ncase "$1" in is-active) exit 3;; esac\nexit 0\n')
        cls.install = cls.root / 'installed'
        environment = dict(os.environ, MARENGO_INSTALL_ROOT=str(cls.install),
                           MARENGO_USER=cls.runtime, MARENGO_DEPLOY_USER=cls.deploy,
                           PATH=f'{fake_bin}:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin')
        cls.environment = environment
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
        changed = self.as_runtime('import os,sys; os.chmod(sys.argv[1],0o777)', self.install)
        self.assertNotEqual(changed.returncode, 0, 'runtime can reopen release ancestor permissions')

    def test_actual_sudo_helper_ancestors_are_immutable(self):
        for helper in self.helpers:
            for ancestor in helper.parents:
                with self.subTest(helper=helper, ancestor=ancestor):
                    metadata = ancestor.stat()
                    self.assertEqual(metadata.st_uid, 0, 'privileged ancestor is not root-owned')
                    self.assertEqual(metadata.st_mode & 0o022, 0,
                                     'privileged ancestor is group/world writable')

    def test_enqueue_state_paths_cannot_redirect_root_writes(self):
        helper = next(path for path in self.helpers if 'enqueue' in path.name)
        (self.bundle / '.git').mkdir()
        state = self.install / 'var'
        lock_victim = self.root / 'root-lock-sentinel'
        temporary_victim = self.root / 'root-temporary-sentinel'
        for victim in [lock_victim, temporary_victim]:
            victim.write_bytes(b'privileged bytes must remain intact\n')
            victim.chmod(0o600)
        attacked_lock = state / 'deploy-job.lock'
        attack = self.as_runtime('import os,sys; os.symlink(sys.argv[1],sys.argv[2])',
                                 lock_victim, attacked_lock)
        self.assertEqual(attack.returncode, 0, attack.stderr)
        marker = self.root / 'enqueue-paused'
        resume = self.root / 'enqueue-resume'
        recorder = self.root / 'systemd-run-arguments'
        (self.fake_bin / 'systemctl').write_text(
            '#!/bin/sh\n'
            'case "$1" in\n'
            'is-active) exit 3;;\n'
            f'reset-failed) touch "{marker}"; '
            f'for unused in $(seq 1 500); do test -f "{resume}" && exit 0; sleep 0.01; done; exit 1;;\n'
            'esac\nexit 0\n')
        launcher = self.fake_bin / 'systemd-run'
        launcher.write_text(f'#!/bin/sh\nprintf "%s\\n" "$@" > "{recorder}"\n')
        launcher.chmod(0o755)
        job_file = state / 'enqueue-contract.json'
        environment = dict(os.environ, MARENGO_ROOT=str(self.install),
                           MARENGO_STAGING_ROOT=str(self.bundle), MARENGO_DEPLOY_USER=self.deploy,
                           PATH=f'{self.fake_bin}:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin',
                           MARENGO_DEPLOY_JOB_FILE=str(job_file))
        process = subprocess.Popen([str(helper), 'a' * 40, 'contract-job'], env=environment,
                                   text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        try:
            deadline = time.monotonic() + 5
            while not marker.exists() and process.poll() is None and time.monotonic() < deadline:
                time.sleep(0.01)
            self.assertTrue(marker.exists(), 'real helper did not reach controlled pre-write barrier')
            predictable = Path(f'{job_file}.tmp.{process.pid}')
            attack = self.as_runtime('import os,sys; os.symlink(sys.argv[1],sys.argv[2])',
                                     temporary_victim, predictable)
            self.assertEqual(attack.returncode, 0, attack.stderr)
            resume.touch()
            stdout, stderr = process.communicate(timeout=10)
            self.assertEqual(process.returncode, 0, stdout + stderr)
            self.assertTrue(recorder.is_file(), 'valid request must reach recorded systemd-run')
            for victim in [lock_victim, temporary_victim]:
                with self.subTest(victim=victim.name):
                    self.assertEqual(victim.read_bytes(), b'privileged bytes must remain intact\n')
                    self.assertEqual(victim.stat().st_mode & 0o777, 0o600)
            self.assertNotEqual(job_file.stat().st_uid, 0, 'runtime state written with root ownership')
        finally:
            resume.touch()
            if process.poll() is None:
                process.kill()
                process.communicate()

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

    def test_installer_refuses_redirected_code_before_writing(self):
        for redirected in ['www', 'scripts/nested-helper', 'var/log', 'var/calibration']:
            with self.subTest(redirected=redirected):
                destination = self.root / ('redirect-' + redirected.replace('/', '-'))
                destination.mkdir()
                external = self.root / ('external-' + redirected.replace('/', '-'))
                external.mkdir()
                sentinel = external / 'keep.txt'
                sentinel.write_text('must remain intact')
                sentinel.chmod(0o600)
                link = destination / redirected
                link.parent.mkdir(parents=True, exist_ok=True)
                link.symlink_to(external, target_is_directory=True)
                environment = dict(self.environment, MARENGO_INSTALL_ROOT=str(destination))
                result = subprocess.run(['bash', str(self.bundle / 'scripts/install-pi.sh')],
                                        env=environment, text=True, capture_output=True, timeout=60)
                self.assertNotEqual(result.returncode, 0, 'installer accepted redirected code')
                self.assertIn('symlink', result.stderr)
                self.assertFalse((destination / 'bin/marengo-pi').exists())
                self.assertEqual(sentinel.read_text(), 'must remain intact')
                self.assertEqual(sentinel.stat().st_mode & 0o777, 0o600)

    def test_staged_code_stays_immutable_before_final_seal(self):
        staging = self.root / 'writable-mode-bundle'
        shutil.copytree(self.bundle, staging)
        runtime = pwd.getpwnam(self.runtime)
        for directory in [staging / 'scripts', staging / 'www']:
            directory.mkdir(exist_ok=True)
            (directory / 'staged-code-marker').write_text('never executed')
            os.chown(directory, runtime.pw_uid, runtime.pw_gid)
            directory.chmod(0o775)
        (staging / 'www/index.html').write_text('fixture')
        observations = self.root / 'copy-window-observations.jsonl'
        wrapper = self.fake_bin / 'rsync'
        # Observe immediately after the real rsync returns, before the installer
        # can apply its final seal. No copy behavior is replaced by the wrapper.
        wrapper.write_text(
            '#!/usr/bin/python3\n'
            'import json, pathlib, subprocess, sys\n'
            'result = subprocess.run(["/usr/bin/rsync", *sys.argv[1:]])\n'
            'if result.returncode: sys.exit(result.returncode)\n'
            'marker = pathlib.Path(sys.argv[-1]) / "staged-code-marker"\n'
            'if marker.is_file():\n'
            f'    probe = subprocess.run(["runuser", "-u", "{self.runtime}", "--", '
            '"python3", "-c", "import os,sys; os.rename(sys.argv[1],sys.argv[1]+\'.renamed\')", '
            'str(marker)], capture_output=True)\n'
            f'    with open("{observations}", "a") as output:\n'
            '        output.write(json.dumps(dict(path=str(marker), status=probe.returncode)) + "\\n")\n')
        wrapper.chmod(0o755)
        destination = self.root / 'staged-mode-install'
        environment = dict(self.environment, MARENGO_INSTALL_ROOT=str(destination))
        try:
            result = subprocess.run(['bash', str(staging / 'scripts/install-pi.sh')],
                                    env=environment, text=True, capture_output=True, timeout=60)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            attempts = [json.loads(line) for line in observations.read_text().splitlines()]
            self.assertEqual(len(attempts), 2, 'both scripts and web copies must be observed')
            for attempt in attempts:
                with self.subTest(path=attempt['path']):
                    self.assertNotEqual(attempt['status'], 0, 'runtime replaced code after rsync')
        finally:
            wrapper.unlink()


if __name__ == '__main__':
    unittest.main(verbosity=2)

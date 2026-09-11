"""Installer integration tests; only isolated, installer-owned paths are changed."""
import hashlib
import json
import os
import re
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
import zipfile

ROOT = Path(__file__).resolve().parents[1]
ASSET = 'remover-macos-arm64.zip'
VERSION = re.search(r'^version = "([^"]+)"', (ROOT / 'Cargo.toml').read_text(), re.M).group(1)

class InstallerTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='remover-install-test-')
        self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name)
        self.install = self.base / 'prefix with spaces' / 'remover'
        self.bin = self.base / 'bin with spaces'
        self.env = dict(os.environ, HOME=str(self.base), X_BOT_FOLLOWER_REMOVER_INSTALL_DIR=str(self.install), X_BOT_FOLLOWER_REMOVER_BIN_DIR=str(self.bin))

    def run_installer(self, *args, ok=True):
        result = subprocess.run(['sh', str(ROOT / 'install.sh'), '--no-modify-path', *args], env=self.env, text=True, capture_output=True)
        self.assertEqual(result.returncode == 0, ok, result.stdout + result.stderr)
        return result

    def install_local(self):
        self.run_installer('--from', str(ROOT / 'dist'))

    def altered_release(self, unsafe=False):
        release = self.base / 'altered'
        release.mkdir()
        if unsafe:
            with zipfile.ZipFile(release / ASSET, 'w') as z:
                z.writestr('../escaped', 'must never be extracted')
        else:
            shutil.copy2(ROOT / 'dist' / ASSET, release / ASSET)
        digest = hashlib.sha256((release / ASSET).read_bytes()).hexdigest()
        (release / 'SHA256SUMS').write_text(f'{digest}  {ASSET}\n')
        return release

    def test_fresh_install_upgrade_and_uninstall(self):
        state = self.base / 'cleanup-data'
        state.write_text('preserve me')
        self.install_local()
        executable = self.bin / 'remover'
        self.assertTrue(executable.is_symlink())
        self.assertEqual(subprocess.check_output([str(executable), '--version'], text=True).strip(), f'remover {VERSION}')
        extension = self.install / 'bundle/remover-extension'
        self.assertTrue((extension / 'manifest.json').is_file())
        (extension / 'obsolete.js').write_text('obsolete')
        self.install_local()
        self.assertFalse((extension / 'obsolete.js').exists())
        self.assertEqual(executable.resolve(), (self.install / 'bundle/remover').resolve())
        host = self.base / 'Library/Application Support/Google/Chrome/NativeMessagingHosts/com.x_bot_follower_remover.pairing.json'
        host.parent.mkdir(parents=True)
        host.write_text(json.dumps({'path': '/another/installation/remover-native-host.sh'}))
        subprocess.run([str(executable), 'unregister-host'], env=self.env, check=True)
        self.assertTrue(host.exists(), 'Must preserve another installation\'s host')
        host.write_text(json.dumps({'path': str((self.install / 'bundle/remover-native-host.sh').resolve())}))
        self.run_installer('--uninstall')
        self.assertFalse(host.exists(), 'Must remove its own host registration')
        self.assertFalse(executable.is_symlink())
        self.assertFalse(self.install.exists())
        self.assertEqual(state.read_text(), 'preserve me')

    def test_legacy_override_and_managed_marker_upgrade(self):
        self.install_local()
        (self.install / '.remover-install').rename(self.install / '.forgive-me-install')
        (self.bin / 'remover').unlink()
        (self.install / 'bundle/remover').rename(self.install / 'bundle/forgive-me')
        (self.bin / 'forgive-me').symlink_to(self.install / 'bundle/forgive-me')
        self.env['FORGIVE_ME_INSTALL_DIR'] = self.env.pop('X_BOT_FOLLOWER_REMOVER_INSTALL_DIR')
        self.env['FORGIVE_ME_BIN_DIR'] = self.env.pop('X_BOT_FOLLOWER_REMOVER_BIN_DIR')
        self.install_local()
        self.assertEqual(subprocess.check_output([str(self.bin / 'remover'), '--version'], text=True).strip(), f'remover {VERSION}')
        self.assertFalse((self.bin / 'forgive-me').is_symlink())
        self.assertTrue((self.install / '.remover-install').is_file())

    def test_public_downloads_latest_and_pinned_release_without_github_cli(self):
        tools = self.base / 'tools'
        tools.mkdir()
        curl = tools / 'curl'
        curl.write_text("#!/usr/bin/env python3\nimport sys, shutil, os\nfrom pathlib import Path\na=sys.argv[1:]\nurl=a[-1]\nwith open(os.environ['DOWNLOAD_LOG'], 'a') as log: log.write(url+'\\n')\nshutil.copyfile(Path(os.environ['RELEASE_FIXTURE'])/url.rsplit('/',1)[1], a[a.index('--output')+1])\n")
        curl.chmod(0o755)
        self.env.update(PATH=str(tools)+os.pathsep+self.env['PATH'], DOWNLOAD_LOG=str(self.base/'downloads'), RELEASE_FIXTURE=str(ROOT/'dist'))
        self.run_installer()
        self.run_installer('--version', f'v{VERSION}')
        urls = (self.base/'downloads').read_text().splitlines()
        self.assertEqual(urls, [f'https://github.com/altonwells/x-bot-follower-remover/releases/{release}/{asset}' for release in ['latest/download', f'download/v{VERSION}'] for asset in [ASSET, 'SHA256SUMS']])

    def test_bad_checksum_does_not_change_existing_install(self):
        self.install_local()
        before = (self.install / 'bundle/remover').read_bytes()
        release = self.altered_release()
        with (release / ASSET).open('ab') as f:
            f.write(b'tamper')
        result = self.run_installer('--from', str(release), ok=False)
        self.assertIn('Checksum mismatch', result.stderr)
        self.assertEqual((self.install / 'bundle/remover').read_bytes(), before)
        self.assertFalse((self.install / '.install-lock').exists())

    def test_unsafe_archive_rejected_before_extraction(self):
        release = self.altered_release(unsafe=True)
        result = self.run_installer('--from', str(release), ok=False)
        self.assertIn('Unsafe archive', result.stderr)
        self.assertFalse(self.install.exists())

    def test_version_mismatch_rejected_before_install(self):
        result = self.run_installer('--from', str(ROOT / 'dist'), '--version', 'v99.0.0', ok=False)
        self.assertIn('version disagree', result.stderr)
        self.assertFalse(self.install.exists())

    def test_unmanaged_paths_are_preserved(self):
        self.install.mkdir(parents=True)
        sentinel = self.install / 'important.txt'
        sentinel.write_text('do not replace')
        self.run_installer('--from', str(ROOT / 'dist'), ok=False)
        self.assertEqual(sentinel.read_text(), 'do not replace')
        self.run_installer('--uninstall', ok=False)
        self.assertTrue(sentinel.exists())

    def test_unmanaged_executable_is_preserved(self):
        self.bin.mkdir()
        executable = self.bin / 'remover'
        executable.write_text('other install')
        self.run_installer('--from', str(ROOT / 'dist'), ok=False)
        self.assertEqual(executable.read_text(), 'other install')

    def test_install_lock_preserves_current_version(self):
        self.install_local()
        lock = self.install / '.install-lock'
        lock.mkdir()
        self.run_installer('--from', str(ROOT / 'dist'), ok=False)
        self.assertTrue(lock.exists())
        self.assertTrue((self.install / 'bundle/remover').is_file())

if __name__ == '__main__':
    unittest.main()

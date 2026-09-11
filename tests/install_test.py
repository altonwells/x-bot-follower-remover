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
ASSET = 'forgive-me-macos-arm64.zip'
VERSION = re.search(r'^version = "([^"]+)"', (ROOT / 'Cargo.toml').read_text(), re.M).group(1)

class InstallerTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='forgive-me-install-test-')
        self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name)
        self.install = self.base / 'prefix with spaces' / 'forgive-me'
        self.bin = self.base / 'bin with spaces'
        self.env = dict(os.environ, HOME=str(self.base), FORGIVE_ME_INSTALL_DIR=str(self.install), FORGIVE_ME_BIN_DIR=str(self.bin))

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
        executable = self.bin / 'forgive-me'
        self.assertTrue(executable.is_symlink())
        self.assertEqual(subprocess.check_output([str(executable), '--version'], text=True).strip(), f'forgive-me {VERSION}')
        extension = self.install / 'bundle/forgive-me-extension'
        self.assertTrue((extension / 'manifest.json').is_file())
        (extension / 'obsolete.js').write_text('obsolete')
        self.install_local()
        self.assertFalse((extension / 'obsolete.js').exists())
        self.assertEqual(executable.resolve(), (self.install / 'bundle/forgive-me').resolve())
        host = self.base / 'Library/Application Support/Google/Chrome/NativeMessagingHosts/com.forgive_me.pairing.json'
        host.parent.mkdir(parents=True)
        host.write_text(json.dumps({'path': '/another/installation/forgive-me-native-host.sh'}))
        subprocess.run([str(executable), 'unregister-host'], env=self.env, check=True)
        self.assertTrue(host.exists(), 'Must preserve another installation\'s host')
        host.write_text(json.dumps({'path': str((self.install / 'bundle/forgive-me-native-host.sh').resolve())}))
        self.run_installer('--uninstall')
        self.assertFalse(host.exists(), 'Must remove its own host registration')
        self.assertFalse(executable.is_symlink())
        self.assertFalse(self.install.exists())
        self.assertEqual(state.read_text(), 'preserve me')

    def test_bad_checksum_does_not_change_existing_install(self):
        self.install_local()
        before = (self.install / 'bundle/forgive-me').read_bytes()
        release = self.altered_release()
        with (release / ASSET).open('ab') as f:
            f.write(b'tamper')
        result = self.run_installer('--from', str(release), ok=False)
        self.assertIn('Checksum mismatch', result.stderr)
        self.assertEqual((self.install / 'bundle/forgive-me').read_bytes(), before)
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
        executable = self.bin / 'forgive-me'
        executable.write_text('other install')
        self.run_installer('--from', str(ROOT / 'dist'), ok=False)
        self.assertEqual(executable.read_text(), 'other install')

    def test_install_lock_preserves_current_version(self):
        self.install_local()
        lock = self.install / '.install-lock'
        lock.mkdir()
        self.run_installer('--from', str(ROOT / 'dist'), ok=False)
        self.assertTrue(lock.exists())
        self.assertTrue((self.install / 'bundle/forgive-me').is_file())

if __name__ == '__main__':
    unittest.main()

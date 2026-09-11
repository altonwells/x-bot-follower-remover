"""Returning-user startup through a real PTY. Isolated Chrome/data; no X access."""
import fcntl
import hashlib
import json
import os
from pathlib import Path
import pty
import select
import shutil
import sqlite3
import struct
import subprocess
import sys
import tempfile
import termios
import time
import unittest

BINARY = Path(sys.argv.pop(1)).resolve()

def extension_id(path):
    return ''.join(chr(ord('a') + nibble) for byte in hashlib.sha256(str(path).encode()).digest()[:16] for nibble in [byte >> 4, byte & 15])

class Onboarding(unittest.TestCase):
    def test_saved_owner_and_removed_extension_enter_setup_and_repair_binding(self):
        with tempfile.TemporaryDirectory(prefix='remover-onboarding-') as tmp:
            root = Path(tmp)
            data = root / 'data'
            data.mkdir()
            legacy = root / '.local/share/forgive-me/bundle/forgive-me-extension'
            old_id = extension_id(legacy)
            original_token = 'a' * 64
            (data / 'config.json').write_text(json.dumps(dict(token=original_token, port=0, extension_id=old_id)))
            with sqlite3.connect(data / 'cleanup.sqlite') as db:
                db.execute('CREATE TABLE settings(key TEXT PRIMARY KEY,value TEXT NOT NULL)')
                db.execute('INSERT INTO settings VALUES (?,?)', ('last_owner', json.dumps('saved-owner')))
            profile = root / 'Library/Application Support/Google/Chrome/Profile 1'
            profile.mkdir(parents=True)
            (profile / 'Secure Preferences').write_text(json.dumps({'extensions': {'settings': {old_id: {'path': str(legacy)}}}}))
            bundle = root / 'bundle'
            extension = bundle / 'remover-extension'
            extension.mkdir(parents=True)
            (extension / 'manifest.json').write_text('{}')
            executable = bundle / 'remover'
            shutil.copyfile(BINARY, executable)
            executable.chmod(0o755)
            master, slave = pty.openpty()
            fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 34, 120, 0, 0))
            proc = subprocess.Popen([str(executable), '--data-dir', str(data)], stdin=slave, stdout=slave, stderr=slave, env=dict(os.environ, HOME=tmp, TERM='xterm-256color'), start_new_session=True)
            os.close(slave)
            output = bytearray()
            try:
                deadline = time.monotonic() + 5
                while time.monotonic() < deadline:
                    if select.select([master], [], [], .1)[0]:
                        try: chunk = os.read(master, 65536)
                        except OSError: break
                        output.extend(chunk)
                        if b'\x1b[6n' in chunk: os.write(master, b'\x1b[1;1R')
                        if b'Set up Chrome' in output: break
                text = output.decode(errors='replace')
                self.assertIn('SETUP', text)
                self.assertIn('Only the old extension', text)
                self.assertIn('Set up Chrome', text)
                self.assertNotIn('Enter Start cleanup', text)
                self.assertNotIn('press s to scan', text)
                os.write(master, b'q')
                proc.wait(timeout=5)
                self.assertEqual(proc.returncode, 0)
                repaired = json.loads((data / 'config.json').read_text())
                self.assertNotEqual(repaired['token'], original_token)
                self.assertEqual(repaired['extension_id'], extension_id(extension.resolve()))
                with sqlite3.connect(data / 'cleanup.sqlite') as db:
                    self.assertEqual(db.execute("SELECT value FROM settings WHERE key='last_owner'").fetchone()[0], json.dumps('saved-owner'))
            finally:
                if proc.poll() is None: proc.terminate(); proc.wait(timeout=5)
                os.close(master)

if __name__ == '__main__':
    unittest.main()

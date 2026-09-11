"""Exercise an isolated background process, its control socket, and clean exit. No X access."""
import json
from pathlib import Path
import sqlite3
import subprocess
import sys
import tempfile
import time
import unittest

BINARY = str(Path(sys.argv.pop(1)).resolve())

class BackgroundCLI(unittest.TestCase):
    def test_background_status_pause_stop(self):
        with tempfile.TemporaryDirectory(prefix='fm-bg-', dir='/private/tmp') as folder:
            root = Path(folder)
            (root / 'config.json').write_text(json.dumps({'token': 'a' * 64, 'port': 0, 'extension_id': None}))
            db = sqlite3.connect(root / 'cleanup.sqlite')
            db.execute('CREATE TABLE settings(key TEXT PRIMARY KEY,value TEXT NOT NULL)')
            policy = dict(inactive_days=90, skip_verified=True, skip_following=True, include_zero_posts=True, delay_seconds=60, batch_limit=50)
            for key, value in [('last_owner', '1'), ('batch:1', dict(id='test', ids=['2'], policy=policy))]:
                db.execute('INSERT INTO settings VALUES (?,?)', (key, json.dumps(value)))
            db.commit()
            db.close()
            worker = subprocess.Popen([BINARY, '--data-dir', folder, 'worker'], stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True)
            try:
                for _ in range(100):
                    if (root / 'worker.sock').exists(): break
                    if worker.poll() is not None: self.fail(worker.communicate()[1].decode())
                    time.sleep(.05)
                def control(action):
                    return subprocess.check_output([BINARY, '--data-dir', folder, action], text=True, timeout=5)
                self.assertIn('1 queued', control('status'))
                self.assertIn('Waiting for Chrome', control('pause'))
                self.assertIsNone(worker.poll())
                control('stop')
                self.assertEqual(worker.wait(timeout=5), 0)
                self.assertFalse((root / 'worker.sock').exists())
            finally:
                if worker.poll() is None:
                    worker.terminate()
                    worker.wait(timeout=5)
                worker.communicate()

if __name__ == '__main__': unittest.main()

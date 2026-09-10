"""Real PTY smoke test: alternate screen, mouse capture, resize and shell restoration.
Only --demo is run. No browser, saved account state or X action is accessed.
"""
import errno
import fcntl
import os
import pty
import select
import signal
import struct
import sys
import termios
import time
import unittest
from pathlib import Path

BINARY = str(Path(sys.argv.pop(1) if len(sys.argv) > 1 else "target/debug/forgive-me").resolve())

class Fullscreen(unittest.TestCase):
    def test_fullscreen_lifecycle_and_resize(self):
        pid, master = pty.fork()
        if pid == 0:
            os.environ["TERM"] = "xterm-256color"
            os.execl(BINARY, BINARY, "--demo", "--no-animation")
        output = bytearray()
        exited = False
        cursor_queries = 0

        def read_for(seconds):
            nonlocal cursor_queries
            chunk = bytearray()
            end = time.monotonic() + seconds
            while time.monotonic() < end:
                if select.select([master], [], [], max(0, end - time.monotonic()))[0]:
                    try:
                        data = os.read(master, 65536)
                    except OSError as error:
                        if error.errno == errno.EIO:
                            break
                        raise
                    if not data:
                        break
                    output.extend(data)
                    chunk.extend(data)
                    # Emulate the terminal's cursor-position response during initialization.
                    queries = output.count(b"\x1b[6n")
                    while cursor_queries < queries:
                        os.write(master, b"\x1b[1;1R")
                        cursor_queries += 1
            return chunk

        def resize(width, height):
            fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", height, width, 0, 0))
            os.kill(pid, signal.SIGWINCH)

        try:
            resize(120, 34)
            read_for(1)
            self.assertIn(b"\x1b[?1049h", output, "must enter alternate screen")
            self.assertIn(b"\x1b[?1006h", output, "must capture SGR mouse input")
            self.assertIn(b"FOLLOWER INVENTORY", output)
            os.write(master, b"\x1b[<65;20;15M")  # wheel down; never selects or approves
            self.assertTrue(read_for(0.2), "wheel should move the focused row")
            resize(52, 12)
            self.assertTrue(read_for(0.2), "resize should redraw immediately")
            os.write(master, b"?")
            read_for(0.1)
            os.write(master, b"\x1b[<65;20;5M")
            self.assertTrue(read_for(0.1), "wheel should scroll help inside its panel")
            resize(140, 42)
            read_for(0.2)
            os.write(master, b"\x1b")
            read_for(0.1)
            os.write(master, b"q")
            read_for(0.5)
            found, status = os.waitpid(pid, os.WNOHANG)
            self.assertEqual(found, pid, "q must exit promptly")
            exited = True
            self.assertEqual(os.waitstatus_to_exitcode(status), 0)
            self.assertIn(b"\x1b[?1006l", output, "must release mouse capture")
            self.assertIn(b"\x1b[?1049l", output, "must restore the shell screen")
            self.assertNotIn(b"\n", output, "frames must address cells, not append scrolling lines")
        finally:
            if not exited:
                os.kill(pid, signal.SIGTERM)
                os.waitpid(pid, 0)
            os.close(master)

if __name__ == "__main__":
    unittest.main()

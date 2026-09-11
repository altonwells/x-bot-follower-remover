#!/usr/bin/env python3
"""Serve the built manager with fictional data; no Chrome profile or X access."""
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import shutil
import tempfile

root = Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix="remover-manager-preview-") as folder:
    page = Path(folder)
    shutil.copytree(root / "extension/dist", page, dirs_exist_ok=True)
    shutil.copyfile(root / "examples/manager-fixture.js", page / "fixture.js")
    html = page / "options.html"
    html.write_text(html.read_text().replace('<script type="module" src="options.js"', '<script src="fixture.js"></script><script type="module" src="options.js"'))
    print("Fictional preview: http://127.0.0.1:8793/options.html", flush=True)
    ThreadingHTTPServer(("127.0.0.1", 8793), partial(SimpleHTTPRequestHandler, directory=folder)).serve_forever()

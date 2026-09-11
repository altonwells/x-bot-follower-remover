#!/usr/bin/env python3
"""Build archives in a fresh temp directory, then publish complete products to dist."""
from pathlib import Path
import hashlib
import json
import platform
import shutil
import subprocess
import tempfile
import zipfile

root = Path(__file__).resolve().parents[1]
dist = root / 'dist'
dist.mkdir(exist_ok=True)
host = next(line.split(': ', 1)[1] for line in subprocess.check_output(['rustc', '-vV'], text=True).splitlines() if line.startswith('host: '))
metadata = json.loads(subprocess.check_output(['cargo', 'metadata', '--locked', '--offline', '--filter-platform', host, '--format-version', '1'], cwd=root))

with tempfile.TemporaryDirectory(prefix='remover-package-') as temporary:
    stage = Path(temporary)
    bundle = stage / 'remover'
    bundle.mkdir()
    shutil.copyfile(root / 'target/release/remover', bundle / 'remover')
    (bundle / 'remover').chmod(0o755)
    shutil.copytree(root / 'extension/dist', bundle / 'remover-extension', copy_function=shutil.copyfile)
    for name in ['README.md', 'LICENSE', 'install.sh']:
        shutil.copyfile(root / name, bundle / name)
    for name in ['docs', 'protocol']:
        shutil.copytree(root / name, bundle / name, copy_function=shutil.copyfile)
    licenses = bundle / 'licenses'
    licenses.mkdir()
    lines = ['# Dependency license inventory', '', 'Generated from Cargo.lock and host package metadata, including development dependencies.', '']
    for package in sorted(metadata['packages'], key=lambda p: (p['name'], p['version'])):
        if package['name'] == 'x-bot-follower-remover':
            continue
        lines.append(f"- {package['name']} {package['version']}: {package.get('license') or 'See package license file'}")
        for source in Path(package['manifest_path']).parent.iterdir():
            if source.is_file() and source.name.upper().startswith(('LICENSE', 'COPYING', 'NOTICE', 'UNLICENSE')):
                folder = licenses / f"{package['name']}-{package['version']}"
                folder.mkdir(exist_ok=True)
                # Use current timestamps; registry epoch timestamps can confuse synced folders.
                shutil.copyfile(source, folder / source.name)
    (bundle / 'DEPENDENCY_LICENSES.md').write_text('\n'.join(lines) + '\n')
    extension_zip = stage / 'remover-extension.zip'
    with zipfile.ZipFile(extension_zip, 'w', zipfile.ZIP_DEFLATED) as archive:
        for path in sorted((bundle / 'remover-extension').rglob('*')):
            if path.is_file():
                archive.write(path, path.relative_to(bundle))
    full_zip = stage / f'remover-macos-{platform.machine()}.zip'
    with zipfile.ZipFile(full_zip, 'w', zipfile.ZIP_DEFLATED) as archive:
        for path in sorted(bundle.rglob('*')):
            if path.is_file():
                archive.write(path, path.relative_to(stage))
    products = [bundle / 'remover', extension_zip, full_zip]
    checksums = ''.join(hashlib.sha256(path.read_bytes()).hexdigest() + '  ' + path.name + '\n' for path in products)
    for path in products:
        pending = dist / ('.' + path.name + '.pending')
        shutil.copyfile(path, pending)
        if path.name == 'remover':
            pending.chmod(0o755)
        pending.replace(dist / path.name)
    (dist / 'SHA256SUMS').write_text(checksums)
    for name in ['README.md', 'LICENSE', 'install.sh', 'DEPENDENCY_LICENSES.md']:
        shutil.copyfile(bundle / name, dist / name)
    for name in ['docs', 'protocol', 'remover-extension']:
        shutil.copytree(bundle / name, dist / name, dirs_exist_ok=True, copy_function=shutil.copyfile)
    print(f'Binary: {dist / "remover"}\nExtension: {dist / extension_zip.name}\nBundle: {dist / full_zip.name}')

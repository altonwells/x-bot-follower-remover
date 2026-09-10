#!/usr/bin/env python3
"""Package only build products and public documentation; never user data."""
from pathlib import Path
import json, shutil, subprocess, zipfile, platform, hashlib
root=Path(__file__).resolve().parents[1]
dist=root/'dist';dist.mkdir(exist_ok=True)
shutil.copy2(root/'target/release/forgive-me',dist/'forgive-me')
shutil.copytree(root/'extension/dist',dist/'forgive-me-extension',dirs_exist_ok=True)
for name in ['README.md','LICENSE','THIRD_PARTY_NOTICES.md']:
    shutil.copy2(root/name,dist/name)
shutil.copytree(root/'docs',dist/'docs',dirs_exist_ok=True)
shutil.copytree(root/'protocol',dist/'protocol',dirs_exist_ok=True)
host=next(line.split(': ',1)[1] for line in subprocess.check_output(['rustc','-vV'],text=True).splitlines() if line.startswith('host: '))
metadata=json.loads(subprocess.check_output(['cargo','metadata','--locked','--offline','--filter-platform',host,'--format-version','1'],cwd=root))
licenses=dist/'licenses';licenses.mkdir(exist_ok=True)
lines=['# Dependency license inventory','','Generated from Cargo.lock and Cargo package metadata. Includes development and target-specific packages.','']
for p in sorted(metadata['packages'],key=lambda p:(p['name'],p['version'])):
    if p['name']=='forgive-me':continue
    lines.append(f"- {p['name']} {p['version']}: {p.get('license') or 'See package license file'}")
    package=Path(p['manifest_path']).parent
    for f in package.iterdir():
        if f.is_file() and f.name.upper().startswith(('LICENSE','COPYING','NOTICE','UNLICENSE')):
            out=licenses/f"{p['name']}-{p['version']}";out.mkdir(exist_ok=True);shutil.copy2(f,out/f.name)
(dist/'DEPENDENCY_LICENSES.md').write_text('\n'.join(lines)+'\n')
with zipfile.ZipFile(dist/'forgive-me-extension.zip','w',zipfile.ZIP_DEFLATED,strict_timestamps=False)as z:
    for p in sorted((dist/'forgive-me-extension').rglob('*')):
        if p.is_file():z.write(p,p.relative_to(dist))
archive=dist/f'forgive-me-macos-{platform.machine()}.zip'
with zipfile.ZipFile(archive,'w',zipfile.ZIP_DEFLATED,strict_timestamps=False)as z:
    for p in sorted(dist.rglob('*')):
        if p.is_file() and p.suffix!='.zip' and p.name!='SHA256SUMS':z.write(p,Path('forgive-me')/p.relative_to(dist))
print(f'Binary: {dist / "forgive-me"}\nExtension: {dist / "forgive-me-extension.zip"}\nBundle: {archive}')

products=[dist/'forgive-me',dist/'forgive-me-extension.zip',archive]
(dist/'SHA256SUMS').write_text(''.join(hashlib.sha256(p.read_bytes()).hexdigest()+'  '+p.name+'\n' for p in products))

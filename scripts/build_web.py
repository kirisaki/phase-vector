#!/usr/bin/env python3
"""Build a self-contained static site with relative URLs for GitHub Pages."""

from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def publish_site(site, dist):
    # Keep imported paths present while Vite serves the previous build.
    files = [path for path in site.rglob('*') if path.is_file()]
    # Publish the WASM before its JS entry point triggers a development reload.
    files.sort(key=lambda path: path.name == 'phase_vector.js')
    for source in files:
        destination = dist / source.relative_to(site)
        destination.parent.mkdir(parents=True, exist_ok=True)
        with tempfile.NamedTemporaryFile(dir=destination.parent, delete=False) as output:
            temporary = Path(output.name)
        try:
            shutil.copy2(source, temporary)
            temporary.replace(destination)
        finally:
            temporary.unlink(missing_ok=True)
    expected = {path.relative_to(site) for path in files}
    for path in dist.rglob('*'):
        if path.is_file() and path.relative_to(dist) not in expected:
            path.unlink()


def main():
    version = re.search(r'name = "wasm-bindgen"\nversion = "([^"]+)"',
                        (ROOT / 'Cargo.lock').read_text()).group(1)
    if sys.argv[1:] == ['--bindgen-version']:
        print(version)
        return
    try:
        installed = subprocess.check_output(['wasm-bindgen', '--version'], text=True).strip()
    except FileNotFoundError:
        raise SystemExit(f'Install the matching CLI: cargo install wasm-bindgen-cli --version {version} --locked')
    if installed != f'wasm-bindgen {version}':
        raise SystemExit(f'Expected wasm-bindgen {version}, found {installed}')
    subprocess.run(['cargo', 'build', '--locked', '--release', '--target',
                    'wasm32-unknown-unknown'], cwd=ROOT, check=True)
    dist = ROOT / 'dist'
    with tempfile.TemporaryDirectory(prefix='phase-vector-web-') as temporary:
        site = Path(temporary) / 'site'
        shutil.copytree(ROOT / 'web', site, ignore=shutil.ignore_patterns('*.ts'))
        subprocess.run(['wasm-bindgen', '--target', 'web', '--remove-name-section',
                        '--out-dir', str(site / 'pkg'),
                        str(ROOT / 'target/wasm32-unknown-unknown/release/phase_vector.wasm')],
                       cwd=ROOT, check=True)
        # Generated declarations and bindings share the Vite/TypeScript virtual root.
        publish_site(site / 'pkg', dist / 'pkg')
        subprocess.run(['npm', 'run', 'check'], cwd=ROOT, check=True)
        subprocess.run(['node', str(ROOT / 'node_modules/typescript/bin/tsc'),
                        '--project', str(ROOT / 'tsconfig.json'), '--noEmit', 'false',
                        '--outDir', str(site)], cwd=ROOT, check=True)
        entry = site / 'index.html'
        entry.write_text(entry.read_text().replace('./app.ts', './app.js'))
        publish_site(site, dist)
    print(f'Static site written to {dist}')


if __name__ == '__main__':
    main()

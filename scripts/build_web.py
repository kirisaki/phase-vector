#!/usr/bin/env python3
"""Build a self-contained static site with relative URLs for GitHub Pages."""

from pathlib import Path
import re
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]


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
    if dist.exists():
        shutil.rmtree(dist)
    shutil.copytree(ROOT / 'web', dist)
    subprocess.run(['wasm-bindgen', '--target', 'web', '--no-typescript', '--remove-name-section',
                    '--out-dir', str(dist / 'pkg'),
                    str(ROOT / 'target/wasm32-unknown-unknown/release/phase_vector.wasm')],
                   cwd=ROOT, check=True)
    print(f'Static site written to {dist}')


if __name__ == '__main__':
    main()

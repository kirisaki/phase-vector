#!/usr/bin/env python3
"""Render favicon variants and capture a real, paused mission-start social image."""

from pathlib import Path
from playwright.sync_api import sync_playwright, expect
from PIL import Image
from browser_site import RELEASE_URL, route_release_site
from browser_controls import connect_engine

ROOT = Path(__file__).resolve().parents[1]
ASSETS = ROOT / 'web' / 'assets'
VIEWPORT = {'width': 1600, 'height': 900}
CARD = {'width': 1200, 'height': 630}


def main():
    ASSETS.mkdir(exist_ok=True)
    with sync_playwright() as playwright:
        browser = playwright.chromium.launch(headless=True, args=[
            '--no-sandbox', '--use-angle=swiftshader', '--enable-unsafe-swiftshader',
        ])
        page = browser.new_page(viewport=VIEWPORT, device_scale_factor=1)
        route_release_site(page)
        page.goto(RELEASE_URL)
        expect(page.locator('body')).to_have_attribute('data-ready', 'true', timeout=90000)
        connect_engine(page)
        page.locator('[data-view="friendly"]').click()
        page.locator('#zoom-in').click()
        # Change only the capture viewport and camera, never the simulated world.
        page.add_style_tag(content='''
            .canvas-wrap { position: fixed !important; inset: 0 !important;
                width: 100vw !important; height: 100vh !important; z-index: 1000;
                background: #070f16; }
        ''')
        frames = page.evaluate('wasm.rendered_frames()')
        page.wait_for_function('n => wasm.rendered_frames() > n + 5', arg=frames)
        page.wait_for_function("document.querySelector('#tactical-map').width === innerWidth && document.querySelector('#tactical-map').height === innerHeight")
        page.wait_for_timeout(250)
        state = page.evaluate('readState()')
        assert state['world']['tick'] == 0 and state['paused']
        assert state['map']['focus'] == 'friendly'
        own = next(ship for ship in state['world']['ships'] if ship['id'] == state['player_id'])
        assert state['map']['center'] == own['position']
        page.mouse.move(0, 0)
        # Crop around the ship, leaving edge controls outside the social image.
        page.screenshot(path=str(ASSETS / 'ogp-start.png'), clip={
            'x': (VIEWPORT['width'] - CARD['width']) / 2,
            'y': (VIEWPORT['height'] - CARD['height']) / 2, **CARD,
        })
        assert Image.open(ASSETS / 'ogp-start.png').size == (1200, 630)
        page.close()

        # Raster fallbacks use the same SVG arrow as the scalable favicon.
        svg = (ASSETS / 'favicon.svg').read_text()
        for size, name in [(32, 'favicon-32.png'), (180, 'apple-touch-icon.png')]:
            icon = browser.new_page(viewport={'width': size, 'height': size}, device_scale_factor=1)
            icon.set_content('<style>html,body{margin:0;background:transparent}svg{display:block;width:100vw;height:100vh}</style>' + svg)
            icon.screenshot(path=str(ASSETS / name), omit_background=True)
            icon.close()
        browser.close()
    print('Captured 1200×630 mission-start OGP image and SVG-derived favicon variants.')


if __name__ == '__main__':
    main()

#!/usr/bin/env python3
"""Exercise the built site in Chromium under the GitHub Pages project subpath."""

import argparse
import mimetypes
from pathlib import Path
from urllib.parse import unquote, urlparse

from playwright.sync_api import sync_playwright, expect

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--url', help='Test an actual deployment instead of the local dist directory')
    parser.add_argument('--executable-path', help='Use an existing Chromium executable')
    args = parser.parse_args()
    artifacts = ROOT / 'artifacts'
    artifacts.mkdir(exist_ok=True)
    errors = []
    failed_requests = []

    with sync_playwright() as playwright:
        browser = playwright.chromium.launch(
            headless=True, executable_path=args.executable_path,
            args=['--no-sandbox', '--use-angle=swiftshader', '--enable-unsafe-swiftshader'],
        )
        page = browser.new_page(viewport={'width': 1360, 'height': 1000}, device_scale_factor=1)
        page.on('pageerror', lambda error: errors.append(str(error)))
        page.on('requestfailed', lambda request: failed_requests.append(request.url))

        if not args.url:
            def serve(route):
                path = unquote(urlparse(route.request.url).path)
                prefix = '/phase-vector/'
                if not path.startswith(prefix):
                    route.fulfill(status=404, body='Wrong project base path')
                    return
                relative = path[len(prefix):] or 'index.html'
                file = (ROOT / 'dist' / relative).resolve()
                if not file.is_relative_to(ROOT / 'dist') or not file.is_file():
                    route.fulfill(status=404, body='Not found')
                    return
                mime = 'application/wasm' if file.suffix == '.wasm' else mimetypes.guess_type(file)[0]
                route.fulfill(path=str(file), content_type=mime or 'application/octet-stream')

            # Real browser requests are fulfilled from the release artifact. No
            # local listening port is required in restricted development sessions.
            page.route('https://phase-vector.test/**', serve)

        try:
            response = page.goto(args.url or 'https://phase-vector.test/phase-vector/')
            assert response.ok, response.status
            expect(page.locator('body')).to_have_attribute('data-ready', 'true', timeout=90000)
            expect(page.locator('#ticks')).to_have_text('0')
            expect(page.locator('#mode')).to_have_text('PLANNING')
            expect(page.locator('#tactical-map')).to_be_visible()

            def lua(source, expected=None, error=False):
                page.locator('#lua-input').fill(source)
                page.locator('#run').click()
                result = page.locator('#console-output p').last
                expect(result).to_have_class('error-line' if error else 'result')
                if expected is not None:
                    expect(result).to_contain_text(expected)
                return result.inner_text()

            lua('return 1 + 2', '3')
            lua('answer = 6 * 7; print("Answer:", answer)', '42')
            lua('answer', '42')
            expect(page.locator('#step')).to_be_disabled()
            expect(page.locator('#toggle')).to_be_disabled()
            lua("game.command('step')", 'Commit orders', error=True)
            lua('game.burn(0/0, 0)', 'finite', error=True)
            lua('game.speed(1.5)', 'Playback speed', error=True)
            page.locator('#burn-x').fill('0.02')
            expect(page.locator('#commit')).to_be_disabled()
            page.locator('#apply-burn').click()
            lua('game.state().planned_burn.x', '0.02')
            page.wait_for_timeout(250)
            expect(page.locator('#ticks')).to_have_text('0')
            before = page.locator('#tactical-map').screenshot()
            page.locator('#commit').click()
            expect(page.locator('#mode')).to_have_text('EXECUTING')
            page.locator('#toggle').click()
            expect(page.locator('#mode')).to_have_text('PAUSED')
            expect(page.locator('#burn-x')).to_be_disabled()
            expect(page.locator('#commit')).to_be_disabled()
            lua('game.burn(0, 0)', 'only change during planning', error=True)
            lua("game.command('commit')", 'only change during planning', error=True)
            paused_ticks = page.locator('#ticks').inner_text()
            page.wait_for_timeout(250)
            expect(page.locator('#ticks')).to_have_text(paused_ticks)
            page.locator('#step').click()
            expect(page.locator('#ticks')).to_have_text(str(int(paused_ticks) + 1))
            lua('game.state().ship.vx > 1', 'true')
            page.locator('#speed').select_option('64')
            lua('game.state().speed', '64')
            page.locator('#toggle').click()
            expect(page.locator('#mode')).to_have_text('PLANNING', timeout=15000)
            expect(page.locator('#time')).to_have_text('120.00')
            expect(page.locator('#ticks')).to_have_text('480')
            expect(page.locator('#dv')).to_have_text('17.600')
            expect(page.locator('#spent-dv')).to_have_text('2.400')
            page.wait_for_timeout(250)
            expect(page.locator('#ticks')).to_have_text('480')
            after = page.locator('#tactical-map').screenshot()
            assert before != after, 'Physical movement was not rendered'
            sample = 'return game.state().ship.x, game.state().ship.vx, game.state().ship.remaining_dv'
            played_result = lua(sample)
            page.locator('#reset').click()
            expect(page.locator('#ticks')).to_have_text('0')
            expect(page.locator('#dv')).to_have_text('20.000')
            lua('answer', '42')
            lua("game.burn(0.02, 0); game.command('commit'); game.command('pause'); for i=1,480 do game.command('step') end")
            assert lua(sample) == played_result, 'Playback and manual stepping diverged'
            expect(page.locator('#time')).to_have_text('120.00')
            lua('game.state().command_count', '1')
            expect(page.locator('#burn-x')).to_have_value('0')
            page.locator('#speed').select_option('64')
            page.locator('#commit').click()
            expect(page.locator('#mode')).to_have_text('PLANNING', timeout=15000)
            expect(page.locator('#time')).to_have_text('240.00')
            expect(page.locator('#dv')).to_have_text('17.600')
            lua('game.state().command_count', '2')
            lua('function (', error=True)
            lua("error('probe error')", 'probe error', error=True)
            lua('while true do end', 'budget', error=True)
            lua('return 2 + 3', '5')
            lua('answer', '42')
            page.screenshot(path=str(artifacts / 'desktop.png'), full_page=True)
            page.set_viewport_size({'width': 390, 'height': 844})
            page.wait_for_timeout(300)
            assert page.evaluate('document.documentElement.scrollWidth <= innerWidth'), 'Horizontal overflow'
            expect(page.locator('#tactical-map')).to_be_visible()
            page.screenshot(path=str(artifacts / 'mobile.png'), full_page=True)
            assert not errors, errors
            assert not failed_requests, failed_requests
            print('PASS: physical motion, delta-v, WEGO boundaries, order locks, replay, Lua, and responsive layout')
        finally:
            page.screenshot(path=str(artifacts / 'last-page.png'), full_page=True)
            (artifacts / 'browser-errors.txt').write_text('\n'.join(errors + failed_requests))
            browser.close()


if __name__ == '__main__':
    main()

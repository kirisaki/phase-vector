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
        page.on('requestfailed', lambda request: failed_requests.append(f'{request.url}: {request.failure}'))

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
            expect(page.locator('#velocity')).to_have_text('0.000, 29979.246')
            expect(page.locator('#dv')).to_have_text('1200.000')
            expect(page.locator('#speed [aria-pressed="true"]')).to_have_text('32×')
            expect(page.locator('#next-turn')).to_be_disabled()
            expect(page.locator('#tactical-map')).to_be_visible()
            expect(page.locator('#console-panel')).to_be_hidden()
            expect(page.locator('#console-toggle')).to_have_attribute('aria-expanded', 'false')
            page.screenshot(path=str(artifacts / 'desktop-map.png'), full_page=True)
            page.locator('#console-toggle').click()
            expect(page.locator('#lua-input')).to_be_focused()

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
            for speed in [32, 64, 128, 1]:
                page.locator(f'[data-speed="{speed}"]').click()
                expect(page.locator('#speed [aria-pressed="true"]')).to_have_text(f'{speed}×')
                lua('game.state().speed', str(speed))
                expect(page.locator('#ticks')).to_have_text('0')
            lua('game.speed(32)')
            expect(page.locator('[data-speed="32"]')).to_have_attribute('aria-pressed', 'true')
            lua('game.speed(1)')
            lua('answer', '42')
            page.locator('#console-close').click()
            expect(page.locator('#console-panel')).to_be_hidden()
            expect(page.locator('#console-toggle')).to_be_focused()
            page.locator('#console-toggle').click()
            expect(page.locator('#lua-input')).to_have_value('answer')
            expect(page.locator('#console-output p').last).to_have_text('42')
            page.locator('#lua-input').press('Escape')
            expect(page.locator('#console-panel')).to_be_hidden()
            page.locator('#console-toggle').click()
            expect(page.locator('#step')).to_be_disabled()
            expect(page.locator('#toggle')).to_be_disabled()
            lua("game.command('step')", 'Commit orders', error=True)
            lua('game.burn(0/0, 0)', 'finite', error=True)
            lua('game.speed(1.5)', 'Playback speed', error=True)
            page.locator('#burn-x').fill('1000')
            expect(page.locator('#commit')).to_be_disabled()
            page.locator('#apply-burn').click()
            lua('game.state().planned_burn.x', '1000')
            page.wait_for_timeout(250)
            expect(page.locator('#ticks')).to_have_text('0')
            expect(page.locator('#railgun-label')).to_contain_text('0.90c')
            lua('game.state().scale.light_speed_mps', '299792458')
            page.locator('[data-view="friendly"]').click()
            expect(page.locator('#grid-label')).to_have_text('GRID / 10,000 km')
            page.locator('#zoom-in').click()
            expect(page.locator('#grid-label')).to_have_text('GRID / 5,000 km')
            page.locator('#zoom-out').click()
            expect(page.locator('#grid-label')).to_have_text('GRID / 10,000 km')
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
            expect(page.locator('#ticks')).to_have_text(str(int(paused_ticks) + 40))
            expect(page.locator('#mode')).to_have_text('PAUSED')
            lua('game.state().ship.vx > 0', 'true')
            page.locator('[data-speed="64"]').click()
            lua('game.state().speed', '64')
            expect(page.locator('#mode')).to_have_text('PAUSED')
            expect(page.locator('#speed [aria-pressed="true"]')).to_have_text('64×')
            page.locator('#toggle').click()
            expect(page.locator('#mode')).to_have_text('PLANNING', timeout=15000)
            expect(page.locator('#time')).to_have_text('120.00')
            expect(page.locator('#ticks')).to_have_text('480')
            expect(page.locator('#dv')).to_have_text('1080.000')
            expect(page.locator('#spent-dv')).to_have_text('120.000')
            page.wait_for_timeout(250)
            expect(page.locator('#ticks')).to_have_text('480')
            after = page.locator('#tactical-map').screenshot()
            assert before != after, 'Physical movement was not rendered'
            sample = 'return game.state().ship.x, game.state().ship.vx, game.state().ship.remaining_dv'
            played_result = lua(sample)
            lua("game.command('reset')")
            expect(page.locator('#ticks')).to_have_text('0')
            expect(page.locator('#dv')).to_have_text('1200.000')
            expect(page.locator('#speed [aria-pressed="true"]')).to_have_text('32×')
            lua('answer', '42')
            lua("game.burn(1000, 0); game.command('commit'); game.command('pause'); for i=1,480 do game.command('step') end")
            assert lua(sample) == played_result, 'Playback and manual stepping diverged'
            expect(page.locator('#time')).to_have_text('120.00')
            lua('game.state().command_count', '1')
            expect(page.locator('#burn-x')).to_have_value('0')
            lua("game.command('reset'); game.burn(1000, 0); game.command('commit'); game.command('pause'); for i=1,479 do game.command('step') end")
            expect(page.locator('#time')).to_have_text('119.75')
            page.locator('#step').click()
            expect(page.locator('#time')).to_have_text('120.00')
            expect(page.locator('#mode')).to_have_text('PLANNING')
            expect(page.locator('#step')).to_be_disabled()
            assert lua(sample) == played_result, '10-second step crossed the planning boundary'
            lua("game.command('reset'); game.burn(1000, 0); game.command('commit'); game.command('pause')")
            expect(page.locator('#next-turn')).to_be_enabled()
            page.locator('#next-turn').click()
            expect(page.locator('#mode')).to_have_text('PLANNING')
            expect(page.locator('#next-turn')).to_be_disabled()
            assert lua(sample) == played_result, 'Next turn and playback diverged'
            expect(page.locator('#speed [aria-pressed="true"]')).to_have_text('32×')
            page.locator('[data-speed="1"]').click()
            page.locator('#commit').click()
            page.locator('#next-turn').click()
            expect(page.locator('#mode')).to_have_text('PLANNING', timeout=15000)
            expect(page.locator('#time')).to_have_text('240.00')
            expect(page.locator('#speed [aria-pressed="true"]')).to_have_text('1×')
            expect(page.locator('#dv')).to_have_text('1080.000')
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
            expect(page.locator('#tactical-map')).to_be_hidden()
            page.screenshot(path=str(artifacts / 'mobile.png'), full_page=True)
            page.locator('#console-close').click()
            expect(page.locator('#console-panel')).to_be_hidden()
            assert page.evaluate('document.documentElement.scrollWidth <= innerWidth'), 'Horizontal overflow'
            page.screenshot(path=str(artifacts / 'mobile-map.png'), full_page=True)
            for width, height in [(1360, 768), (1024, 600), (390, 844), (320, 568), (844, 390)]:
                page.set_viewport_size({'width': width, 'height': height})
                # Wait for visualViewport's resize event to update the app height.
                page.wait_for_function('Math.abs(document.body.getBoundingClientRect().height - innerHeight) < 1')
                expect(page.locator('#tactical-map')).to_be_visible()
                assert page.evaluate('document.documentElement.scrollWidth <= innerWidth && document.documentElement.scrollHeight <= innerHeight'), 'Page overflow'
                for selector in ['#console-toggle', '[data-view="overview"]', '[data-view="friendly"]', '[data-view="contact"]', '#zoom-in', '#zoom-out', '#commit', '#toggle', '#step', '#next-turn', '[data-speed="1"]', '[data-speed="128"]', '[data-speed="32"]', '[data-speed="64"]', '#burn-x', '#burn-y', '#apply-burn']:
                    box = page.locator(selector).bounding_box()
                    assert box and box['x'] >= 0 and box['y'] >= 0 and box['x'] + box['width'] <= width + 1 and box['y'] + box['height'] <= height + 1, (width, height, selector, box)
                page.mouse.wheel(0, 1000)
                assert page.evaluate('scrollX === 0 && scrollY === 0'), 'Page scrolled'
                page.screenshot(path=str(artifacts / f'viewport-{width}x{height}.png'))
                page.locator('#console-toggle').click()
                expect(page.locator('#lua-input')).to_be_focused()
                for selector in ['#console-close', '#lua-input', '#run']:
                    box = page.locator(selector).bounding_box()
                    assert box and box['y'] >= 0 and box['y'] + box['height'] <= height + 1, (width, height, selector, box)
                lua('answer', '42')
                page.locator('#console-close').click()
                expect(page.locator('#console-toggle')).to_be_focused()
            assert not errors, errors
            assert not failed_requests, failed_requests
            print('PASS: physical motion, delta-v, WEGO boundaries, order locks, replay, Lua, and responsive layout')
        finally:
            page.screenshot(path=str(artifacts / 'last-page.png'), full_page=True)
            (artifacts / 'browser-errors.txt').write_text('\n'.join(errors + failed_requests))
            browser.close()


if __name__ == '__main__':
    main()

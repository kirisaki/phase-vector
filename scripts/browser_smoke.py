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
        page = browser.new_page(viewport={'width': 1360, 'height': 1000}, device_scale_factor=1, has_touch=True)
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
            expect(page.locator('#velocity')).to_have_text('0.000, 75.000')
            expect(page.locator('#dv')).to_have_text('60.000')
            expect(page.locator('#range-label')).to_have_text('RANGE / 240,000 km')
            expect(page.locator('#speed [aria-pressed="true"]')).to_have_text('32×')
            expect(page.locator('#next-turn')).to_be_enabled()
            expect(page.locator('#tactical-map')).to_be_visible()
            expect(page.locator('#console-panel')).to_be_hidden()
            expect(page.locator('#console-toggle')).to_have_attribute('aria-expanded', 'false')
            page.screenshot(path=str(artifacts / 'desktop-map.png'), full_page=True)
            def read_state():
                return page.evaluate("""async () => {
                    const url = new URL('./app.js', location.href);
                    const source = await (await fetch(url)).text();
                    const module = source.match(/from ['"]([^'"]+)['"]/)[1];
                    return JSON.parse((await import(new URL(module, url).href)).state_json());
                }""")

            def route_point(x, y):
                return page.locator('#route-editor').evaluate('''(svg, p) => {
                    const circle = svg.querySelector('.reach-area');
                    const cx = Number(circle.getAttribute('cx')), cy = Number(circle.getAttribute('cy'));
                    const radius = Number(circle.getAttribute('r'));
                    const result = new DOMPoint(cx + (p[0] - 200) / 140 * radius,
                        cy + (p[1] - 200) / 140 * radius).matrixTransform(svg.getScreenCTM());
                    return {x: result.x, y: result.y};
                }''', [x, y])

            def handle_point():
                return page.locator('#arrival-handle').evaluate("handle => { const p = new DOMPoint(0, 0).matrixTransform(handle.getScreenCTM()); return {x: p.x, y: p.y}; }")

            def start_drag():
                point = handle_point()
                page.mouse.move(point['x'], point['y'])
                page.mouse.down()

            expect(page.locator('[data-view="overview"]')).to_have_text('Overview · Auto')
            expect(page.locator('#burn-x')).to_be_hidden()
            expect(page.locator('#route-editor')).to_be_visible()
            expect(page.locator('#plan-route, #route-apply, #route-cancel')).to_have_count(0)
            expect(page.locator('#commit')).to_be_enabled()
            original_grid = page.locator('#grid-label').inner_text()
            original_map = page.locator('#tactical-map').bounding_box()
            expect(page.locator('#burn-label')).to_have_text('BURN / 120 s')
            expect(page.locator('#actual-arrival circle')).to_have_count(0)
            current = read_state()
            own = current['world']['ships'][0]['position']
            box = page.locator('#route-editor').bounding_box()
            mpp = max(1000, 600 * box['width'] / box['height']) * 2 * current['map']['metres_per_unit'] / box['width']
            center = route_point(200, 200)
            assert abs(center['x'] - (box['x'] + box['width'] / 2 + (own['x'] - current['map']['center']['x']) / mpp)) < 1
            assert abs(center['y'] - (box['y'] + box['height'] / 2 - (own['y'] - current['map']['center']['y']) / mpp)) < 1
            start_drag()
            frozen_zoom = read_state()['map']['metres_per_unit']
            destination = route_point(270, 165)
            page.mouse.move(destination['x'], destination['y'], steps=5)
            expect(page.locator('#commit')).to_be_disabled()
            expect(page.locator('#next-turn')).to_be_disabled()
            assert float(page.locator('#route-cost').inner_text().replace(',', '')) > 0
            expect(page.locator('#burn-x')).to_have_value('0')
            assert read_state()['map']['metres_per_unit'] == frozen_zoom
            assert page.locator('#map-planned-route').get_attribute('d')
            assert page.locator('#tactical-map').bounding_box() == original_map
            page.mouse.up()
            expect(page.locator('#commit')).to_be_enabled()
            assert float(page.locator('#burn-x').input_value()) > 0
            expect(page.locator('#ticks')).to_have_text('0')
            expect(page.locator('[data-view="overview"]')).to_have_attribute('aria-pressed', 'true')
            expect(page.locator('#grid-label')).to_have_text(original_grid)
            assert page.locator('#tactical-map').bounding_box() == original_map
            page.screenshot(path=str(artifacts / 'route-desktop.png'))
            cost_before = page.locator('#route-cost').inner_text()
            page.locator('#zoom-out').click()
            assert page.locator('#grid-label').inner_text() != original_grid
            expect(page.locator('#route-cost')).to_have_text(cost_before)
            page.locator('[data-view="overview"]').click()

            # Pointer capture preserves direction outside the disk and canvas.
            start_drag()
            for point in [(650, 200), (650, -250)]:
                outside = route_point(*point)
                page.mouse.move(outside['x'], outside['y'], steps=5)
                edge = handle_point()
                center, right = route_point(200, 200), route_point(340, 200)
                radius = right['x'] - center['x']
                dx, dy = edge['x'] - center['x'], edge['y'] - center['y']
                assert abs((dx * dx + dy * dy) ** 0.5 - radius) < 1
                if point[1] != 200:
                    assert abs(dx + dy) < 1, (dx, dy)
                expect(page.locator('#route-editor')).to_have_attribute('data-invalid', 'false')
                assert abs(float(page.locator('#route-cost').inner_text().replace(',', '')) - 5.88399) < 0.001
            page.mouse.up()
            expect(page.locator('#commit')).to_be_enabled()
            original_burn = [page.locator(f'#burn-{axis}').input_value() for axis in ['x', 'y']]
            start_drag()
            center = route_point(200, 200)
            page.mouse.move(center['x'], center['y'], steps=5)
            page.keyboard.press('Escape')
            page.mouse.up()
            assert [page.locator(f'#burn-{axis}').input_value() for axis in ['x', 'y']] == original_burn
            expect(page.locator('#commit')).to_be_enabled()
            page.locator('#arrival-handle').press('Home')
            expect(page.locator('#route-cost')).to_have_text('0')
            expect(page.locator('#burn-x')).to_have_value('0')
            page.locator('#arrival-handle').press('ArrowRight')
            graphical_cost = float(page.locator('#route-cost').inner_text().replace(',', ''))
            assert graphical_cost > 0
            assert float(page.locator('#burn-x').input_value()) > 0
            expect(page.locator('[data-view="overview"]')).to_have_attribute('aria-pressed', 'true')
            expect(page.locator('#grid-label')).to_have_text(original_grid)
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

            # Execute a pointer-authored plan through the real Commit button.
            page.locator('#console-close').click()
            start_drag()
            destination = route_point(270, 165)
            page.mouse.move(destination['x'], destination['y'], steps=5)
            page.mouse.up()
            planned = read_state()
            planned_map_box = page.locator('#tactical-map').bounding_box()
            page.locator('[data-speed="1"]').click()
            page.locator('#commit').click()
            page.locator('#toggle').click()
            executing = read_state()
            assert page.locator('#tactical-map').bounding_box() == planned_map_box
            assert executing['active_burn'] == planned['planned_burn']
            assert executing['maneuver']['endpoint'] == planned['maneuver']['endpoint']
            expect(page.locator('[data-view="overview"]')).to_have_attribute('aria-pressed', 'true')
            expect(page.locator('#route-editor')).to_be_visible()
            expect(page.locator('#arrival-handle')).to_be_hidden()
            expect(page.locator('#burn-label')).to_have_text('BURN / PAUSED')
            assert page.locator('#burn-vector').get_attribute('d')
            page.screenshot(path=str(artifacts / 'executing-burn.png'))
            page.locator('#console-toggle').click()
            lua('game.arrive(0, 0)', 'only change during planning', error=True)
            page.locator('#next-turn').click()
            actual = read_state()['world']['ships'][0]
            assert actual['position'] == planned['maneuver']['endpoint']
            assert actual['velocity'] == planned['maneuver']['arrival_velocity']
            assert abs(actual['spent_dv'] - planned['maneuver']['delta_v']) < 1e-6
            lua("game.command('reset')")
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
            page.locator('#numeric-burn summary').click()
            numeric_map_box = page.locator('#tactical-map').bounding_box()
            page.locator('#burn-x').fill('49.03325')
            expect(page.locator('#route-feedback')).to_contain_text('Apply edited numeric values')
            assert page.locator('#tactical-map').bounding_box() == numeric_map_box
            expect(page.locator('#commit')).to_be_disabled()
            expect(page.locator('#next-turn')).to_be_disabled()
            page.locator('#apply-burn').click()
            lua('math.abs(game.state().planned_burn.x - 49.03325) < 1e-9', 'true')
            page.wait_for_timeout(250)
            expect(page.locator('#ticks')).to_have_text('0')
            expect(page.locator('#railgun-label')).to_contain_text('660 km/s · reference flight 363.6 s')
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
            expect(page.locator('#dv')).to_have_text('54.116')
            expect(page.locator('#spent-dv')).to_have_text('5.884')
            page.wait_for_timeout(250)
            expect(page.locator('#ticks')).to_have_text('480')
            after = page.locator('#tactical-map').screenshot()
            assert before != after, 'Physical movement was not rendered'
            sample = 'return game.state().ship.x, game.state().ship.vx, game.state().ship.remaining_dv'
            played_result = lua(sample)
            lua("game.command('reset')")
            expect(page.locator('#ticks')).to_have_text('0')
            expect(page.locator('#dv')).to_have_text('60.000')
            expect(page.locator('#speed [aria-pressed="true"]')).to_have_text('32×')
            lua('answer', '42')
            lua("game.burn(49.03325, 0); game.command('commit'); game.command('pause'); for i=1,480 do game.command('step') end")
            assert lua(sample) == played_result, 'Playback and manual stepping diverged'
            expect(page.locator('#time')).to_have_text('120.00')
            lua('game.state().command_count', '1')
            expect(page.locator('#burn-x')).to_have_value('0')
            lua("game.command('reset'); game.burn(49.03325, 0); game.command('commit'); game.command('pause'); for i=1,479 do game.command('step') end")
            expect(page.locator('#time')).to_have_text('119.75')
            page.locator('#step').click()
            expect(page.locator('#time')).to_have_text('120.00')
            expect(page.locator('#mode')).to_have_text('PLANNING')
            expect(page.locator('#step')).to_be_disabled()
            assert lua(sample) == played_result, '10-second step crossed the planning boundary'
            lua("game.command('reset'); game.burn(49.03325, 0)")
            page.locator('#next-turn').click()
            expect(page.locator('#mode')).to_have_text('PLANNING')
            expect(page.locator('#time')).to_have_text('120.00')
            assert lua(sample) == played_result, 'Planning Next turn and playback diverged'
            lua('game.state().command_count', '1')
            page.locator('#next-turn').click()
            expect(page.locator('#time')).to_have_text('240.00')
            expect(page.locator('#dv')).to_have_text('54.116')
            lua('game.state().command_count', '2')
            lua("game.command('reset'); game.burn(49.03325, 0); game.command('commit'); game.command('pause')")
            expect(page.locator('#next-turn')).to_be_enabled()
            page.locator('#next-turn').click()
            expect(page.locator('#mode')).to_have_text('PLANNING')
            expect(page.locator('#next-turn')).to_be_enabled()
            assert lua(sample) == played_result, 'Next turn and playback diverged'
            expect(page.locator('#speed [aria-pressed="true"]')).to_have_text('32×')
            page.locator('[data-speed="1"]').click()
            page.locator('#commit').click()
            page.locator('#next-turn').click()
            expect(page.locator('#mode')).to_have_text('PLANNING', timeout=15000)
            expect(page.locator('#time')).to_have_text('240.00')
            expect(page.locator('#speed [aria-pressed="true"]')).to_have_text('1×')
            expect(page.locator('#dv')).to_have_text('54.116')
            lua('game.state().command_count', '2')
            lua('function (', error=True)
            lua("error('probe error')", 'probe error', error=True)
            lua('while true do end', 'budget', error=True)
            lua('return 2 + 3', '5')
            lua('answer', '42')
            # Follow motion in Overview, allow manual override, then restore auto fit.
            lua("game.command('reset'); game.speed(128)")
            page.locator('[data-view="overview"]').click()
            expect(page.locator('[data-view="overview"]')).to_have_text('Overview · Auto')
            page.wait_for_timeout(250)
            initial_zoom = read_state()['map']['metres_per_unit']
            page.locator('#commit').click()
            expect(page.locator('#mode')).to_have_text('PLANNING', timeout=15000)
            expect(page.locator('#time')).to_have_text('120.00')
            assert read_state()['map']['metres_per_unit'] > initial_zoom
            page.locator('#zoom-in').click()
            expect(page.locator('[data-view="overview"]')).to_have_text('Overview')
            manual_zoom = read_state()['map']['metres_per_unit']
            page.locator('#next-turn').click()
            assert read_state()['map']['metres_per_unit'] == manual_zoom
            page.locator('[data-view="overview"]').click()
            expect(page.locator('[data-view="overview"]')).to_have_text('Overview · Auto')
            page.wait_for_timeout(250)
            assert read_state()['map']['metres_per_unit'] > manual_zoom
            page.locator('#numeric-burn summary').click()
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
                for selector in ['#console-toggle', '[data-view="overview"]', '[data-view="friendly"]', '[data-view="contact"]', '#zoom-in', '#zoom-out', '#commit', '#toggle', '#step', '#next-turn', '[data-speed="1"]', '[data-speed="128"]', '[data-speed="32"]', '[data-speed="64"]']:
                    box = page.locator(selector).bounding_box()
                    assert box and box['x'] >= 0 and box['y'] >= 0 and box['x'] + box['width'] <= width + 1 and box['y'] + box['height'] <= height + 1, (width, height, selector, box)
                page.mouse.wheel(0, 1000)
                assert page.evaluate('scrollX === 0 && scrollY === 0'), 'Page scrolled'
                page.screenshot(path=str(artifacts / f'viewport-{width}x{height}.png'))
                page.locator('[data-view="overview"]').click()
                expect(page.locator('#route-editor')).to_be_visible()
                expect(page.locator('[data-view="overview"]')).to_have_attribute('aria-pressed', 'true')
                page.wait_for_timeout(250)
                fitted = read_state()
                box = page.locator('#route-editor').bounding_box()
                half_width = max(1000, 600 * box['width'] / box['height'])
                half_height = max(600, 1000 * box['height'] / box['width'])
                assert fitted['map']['auto_zoom']
                for ship in fitted['world']['ships']:
                    assert abs(ship['position']['x'] - fitted['map']['center']['x']) <= half_width * fitted['map']['metres_per_unit'] * 0.91
                    assert abs(ship['position']['y'] - fitted['map']['center']['y']) <= half_height * fitted['map']['metres_per_unit'] * 0.91
                for selector in ['#route-editor', '#arrival-handle']:
                    box = page.locator(selector).bounding_box()
                    assert box and box['x'] >= 0 and box['y'] >= 0 and box['x'] + box['width'] <= width + 1 and box['y'] + box['height'] <= height + 1, (width, height, selector, box)
                if width < 500:
                    start, end = handle_point(), route_point(250, 180)
                    cdp = page.context.new_cdp_session(page)
                    cdp.send('Input.dispatchTouchEvent', {'type': 'touchStart', 'touchPoints': [start]})
                    cdp.send('Input.dispatchTouchEvent', {'type': 'touchMove', 'touchPoints': [end]})
                    cdp.send('Input.dispatchTouchEvent', {'type': 'touchEnd', 'touchPoints': []})
                    cdp.detach()
                    expect(page.locator('#commit')).to_be_enabled()
                    assert float(page.locator('#burn-x').input_value()) > 0
                    assert float(page.locator('#route-cost').inner_text().replace(',', '')) > 0
                page.screenshot(path=str(artifacts / f'route-{width}x{height}.png'))
                if width < 500:
                    touch_plan = read_state()
                    page.locator('[data-speed="1"]').click()
                    page.locator('#next-turn').click()
                    expect(page.locator('#mode')).to_have_text('PLANNING')
                    actual = read_state()['world']['ships'][0]
                    assert actual['position'] == touch_plan['maneuver']['endpoint']
                    assert actual['velocity'] == touch_plan['maneuver']['arrival_velocity']
                    assert actual['remaining_dv'] == touch_plan['maneuver']['remaining_dv']
                page.locator('#arrival-handle').press('Home')
                # Measure every rendered frame, not just the settled end states.
                stable_box = page.locator('#tactical-map').bounding_box()
                page.evaluate("""() => {
                    window.layoutSamples = [];
                    window.sampleLayout = true;
                    const sample = () => {
                        if (!window.sampleLayout) return;
                        const {x, y, width, height} = document.querySelector('#tactical-map').getBoundingClientRect();
                        window.layoutSamples.push({x, y, width, height});
                        requestAnimationFrame(sample);
                    };
                    sample();
                }""")
                page.locator('[data-speed="1"]').click()
                page.locator('#commit').click()
                expect(page.locator('#mode')).to_have_text('EXECUTING')
                page.wait_for_timeout(150)
                page.locator('#toggle').click()
                expect(page.locator('#mode')).to_have_text('PAUSED')
                page.locator('#step').click()
                page.locator('#toggle').click()
                expect(page.locator('#mode')).to_have_text('EXECUTING')
                page.wait_for_timeout(150)
                page.locator('#toggle').click()
                page.locator('#next-turn').click()
                expect(page.locator('#mode')).to_have_text('PLANNING')
                page.wait_for_timeout(150)
                samples = page.evaluate('() => { window.sampleLayout = false; return window.layoutSamples; }')
                assert samples
                for sample_box in samples:
                    assert all(abs(sample_box[key] - stable_box[key]) < 0.5 for key in stable_box), (width, height, stable_box, sample_box)
                assert page.locator('#tactical-map').bounding_box() == stable_box
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
            print('PASS: stable canvas across playback phases, automatic camera zoom, manual override, direct map dragging, radial limits, touch, physical motion, delta-v, WEGO boundaries, order locks, replay, Lua, and responsive layout')
        finally:
            page.screenshot(path=str(artifacts / 'last-page.png'), full_page=True)
            (artifacts / 'browser-errors.txt').write_text('\n'.join(errors + failed_requests))
            browser.close()


if __name__ == '__main__':
    main()

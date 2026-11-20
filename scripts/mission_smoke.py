#!/usr/bin/env python3
"""Play the blockade through its real browser controls, including touch aiming."""
import argparse
import mimetypes
from pathlib import Path
from urllib.parse import unquote, urlparse
from playwright.sync_api import sync_playwright, expect
from map_navigation_checks import check_map_navigation

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--url')
    parser.add_argument('--executable-path')
    args = parser.parse_args()
    artifacts = ROOT / 'artifacts'
    artifacts.mkdir(exist_ok=True)
    errors = []
    with sync_playwright() as p:
        browser = p.chromium.launch(headless=True, executable_path=args.executable_path,
            args=['--no-sandbox', '--use-angle=swiftshader', '--enable-unsafe-swiftshader'])
        page = browser.new_page(viewport={'width': 1360, 'height': 1000}, has_touch=True)
        page.on('pageerror', lambda e: errors.append(str(e)))
        page.on('requestfailed', lambda r: errors.append(f'{r.url}: {r.failure}'))
        if not args.url:
            def serve(route):
                path = unquote(urlparse(route.request.url).path)
                prefix = '/phase-vector/'
                file = (ROOT / 'dist' / (path.removeprefix(prefix) or 'index.html')).resolve()
                if not path.startswith(prefix) or not file.is_relative_to(ROOT / 'dist') or not file.is_file():
                    route.fulfill(status=404, body='Not found')
                    return
                mime = 'application/wasm' if file.suffix == '.wasm' else mimetypes.guess_type(file)[0]
                route.fulfill(path=str(file), content_type=mime or 'application/octet-stream')
            page.route('https://phase-vector.test/**', serve)
        try:
            page.goto(args.url or 'https://phase-vector.test/phase-vector/')
            expect(page.locator('body')).to_have_attribute('data-ready', 'true', timeout=90000)
            page.evaluate("""async () => {
                const url = new URL('./app.js', location.href);
                const source = await (await fetch(url)).text();
                const module = source.match(/from ['"]([^'"]+)['"]/)[1];
                window.wasm = await import(new URL(module, url).href);
                window.readState = () => JSON.parse(wasm.state_json());
            }""")
            def state():
                return page.evaluate('readState()')

            def next_turn():
                before = state()['world']['tick']
                page.locator('#next-turn').click()
                page.wait_for_function('(tick) => readState().world.tick > tick', arg=before)
                page.wait_for_timeout(120)

            def retry():
                page.locator('#retry').click()
                expect(page.locator('#ticks')).to_have_text('0')
                assert state()['mission']['shots'] == []
                assert state()['world']['projectiles'] == []
                assert state()['mission']['queued'] == []

            def handle_point():
                return page.locator('#aim-handle').evaluate('''h => {
                    const p = new DOMPoint(0, 0).matrixTransform(h.getScreenCTM());
                    return {x: p.x, y: p.y};
                }''')

            check_map_navigation(page, artifacts)
            current = state()
            timing = current['timing']
            reload_ticks = round(current['mission']['reload_seconds'] / timing['step_seconds'])
            assert current['mission']['reload_seconds'] == 30
            assert current['mission']['outcome'] == 'running'
            assert 'remaining_dv' not in current['world']['ships'][1]
            assert 'decision' not in current['mission'] and 'ai_reason' not in current['mission']
            assert page.locator('#enemy-prediction').get_attribute('d')
            assert page.locator('#mission-goal').get_attribute('r')
            expect(page.locator('#queue-shot')).to_be_enabled()
            expect(page.locator('#retry')).to_be_disabled()
            canvas = page.locator('#tactical-map').bounding_box()
            # Pointer placement and cancellation change a reservation, never launch immediately.
            point = handle_point()
            original = current['mission']['aim']['target']
            page.mouse.move(point['x'], point['y'])
            page.mouse.down()
            expect(page.locator('#next-turn')).to_be_disabled()
            expect(page.locator('#commit')).to_be_disabled()
            page.mouse.move(point['x'] - 30, point['y'] + 20, steps=4)
            page.mouse.up()
            page.locator('#queue-shot').click()
            placed = state()['mission']['queued'][0]['target']
            assert placed != original and state()['mission']['shots'] == []
            page.locator('#cancel-shot').click()
            assert state()['mission']['queued'] == []
            page.locator('#track-aim').click()
            page.locator('#queue-shot').click()
            assert state()['mission']['queued'][0]['target'] == original
            page.locator('#cancel-shot').click()
            # Queue four rounds with the existing button, cancel one, and refill its slot.
            for _ in range(4):
                page.locator('#queue-shot').click()
            orders = state()['mission']['queued']
            assert [q['launch_tick'] for q in orders] == [i * reload_ticks for i in range(4)]
            expect(page.locator('#shot-queue .queued-shot')).to_have_count(4)
            expect(page.locator('#queued-shot-markers text')).to_have_count(4)
            assert page.locator('#tactical-map').bounding_box() == canvas
            page.get_by_role('button', name=f"Cancel shot {orders[1]['id']}", exact=True).click()
            assert state()['mission']['queued'][1] == orders[2]
            page.locator('#queue-shot').click()
            assert [q['launch_tick'] for q in state()['mission']['queued']] == [q['launch_tick'] for q in orders]
            page.screenshot(path=str(artifacts / 'mission-multiple-queue.png'))
            page.locator('#step').click()
            assert len(state()['mission']['shots']) == 1
            locked = state()['mission']['queued']
            assert all(q['locked'] for q in locked)
            expect(page.locator('#cancel-shot')).to_be_disabled()
            for order in locked:
                expect(page.get_by_role('button', name=f"Cancel shot {order['id']}", exact=True)).to_be_disabled()
            # Even when paused mid-interval, a new shot belongs to the next command interval.
            page.locator('#queue-shot').click()
            future = state()['mission']['queued'][-1]
            assert not future['locked'] and future['launch_tick'] == timing['interval_ticks']
            page.locator('#cancel-shot').click()
            assert state()['mission']['queued'] == locked
            for count in range(1, 4):
                while state()['world']['tick'] <= count * reload_ticks:
                    page.locator('#step').click()
                assert len(state()['mission']['shots']) == count + 1
            page.locator('#commit').click()
            page.wait_for_function('readState().paused', timeout=30000)
            assert state()['world']['tick'] == timing['interval_ticks']
            assert [shot['launch_tick'] for shot in state()['mission']['shots']] == [i * reload_ticks for i in range(4)]
            remaining_slots = round((state()['mission']['deadline_seconds'] - state()['time_seconds']) / state()['mission']['reload_seconds'])
            for _ in range(remaining_slots):
                page.locator('#queue-shot').click()
            expect(page.locator('#queue-shot')).to_be_disabled()
            expect(page.locator('#gun-feedback')).to_contain_text('No free firing slot')
            expect(page.locator('#shot-queue .queued-shot')).to_have_count(remaining_slots)
            first_id = state()['mission']['queued'][0]['id']
            page.get_by_role('button', name=f'Cancel shot {first_id}', exact=True).click()
            expect(page.locator('#queue-shot')).to_be_enabled()
            page.evaluate("wasm.command('reset')")
            page.wait_for_timeout(200)
            # No attack must lose; the terminal state freezes all gameplay commands.
            for _ in range(9):
                next_turn()
            page.locator('[data-speed="128"]').click()
            page.locator('#commit').click()
            page.wait_for_function('readState().mission.outcome !== "running"', timeout=30000)
            assert not state()['stop_at_boundary']
            assert state()['mission']['outcome'] == 'breakthrough'
            assert abs(state()['mission']['ended_seconds'] - 1173.333333) < 0.001
            expect(page.locator('#mission-status')).to_contain_text('Defeat')
            for control in ['toggle', 'step', 'commit', 'next-turn', 'queue-shot']:
                expect(page.locator('#' + control)).to_be_disabled()
            expect(page.locator('#burn-fields')).to_have_attribute('disabled', '')
            end = state()['world']
            page.wait_for_timeout(300)
            assert state()['world'] == end
            assert page.locator('#tactical-map').bounding_box() == canvas
            page.screenshot(path=str(artifacts / 'mission-defeat.png'))
            retry()
            # Seven observed-lead shots, no friendly burn: blockade without a hit.
            for turn in range(10):
                if turn < 7:
                    page.locator('#queue-shot').click()
                    assert len(state()['mission']['shots']) == turn
                next_turn()
                if turn == 0:
                    assert len(state()['world']['projectiles']) == 1
                assert len(state()['mission']['shots']) == min(turn + 1, 7)
            current = state()
            assert current['mission']['outcome'] == 'timeout', current['mission']
            assert current['world']['impacts'] == []
            expect(page.locator('#mission-status')).to_contain_text('Victory')
            expect(page.locator('#mission-metrics')).to_contain_text('7 shots')
            assert page.locator('#tactical-map').bounding_box() == canvas
            page.screenshot(path=str(artifacts / 'mission-victory.png'))
            # The diagnostic REPL exposes decision reasons and observed tracks separately.
            page.locator('#console-toggle').click()
            page.locator('#lua-input').fill('return game.state().mission.ai_reason, game.state().mission.observed_attacks')
            page.locator('#run').click()
            expect(page.locator('#console-output .result').last).not_to_have_text('nil\tnil')
            page.locator('#console-close').click()
            retry()
            # Running edits queue for a boundary. Already flying rounds retain their velocity.
            page.locator('#queue-shot').click()
            page.locator('[data-speed="1"]').click()
            page.locator('#toggle').click()
            page.wait_for_function('readState().world.tick >= 4')
            expect(page.locator('#queue-shot')).to_be_enabled()
            projectile = state()['world']['projectiles'][0]
            page.locator('#queue-shot').click()
            assert state()['mission']['queued'][0]['launch_tick'] == timing['interval_ticks']
            assert len(state()['mission']['shots']) == 1
            page.locator('#toggle').click()
            next_turn()
            assert state()['mission']['queued'] != []
            page.locator('#step').click()
            expect(page.locator('#ticks')).to_have_text(str(timing['interval_ticks'] + timing['step_forward_ticks']))
            assert len(state()['mission']['shots']) == 2
            assert state()['world']['projectiles'][0]['velocity'] == projectile['velocity']
            # Retry through Lua is the same command API; responsive/touch controls remain usable.
            page.evaluate("wasm.command('reset')")
            page.wait_for_timeout(200)
            for width, height in [(390, 844), (320, 568), (844, 390)]:
                page.set_viewport_size({'width': width, 'height': height})
                page.wait_for_timeout(300)
                assert page.evaluate('document.documentElement.scrollWidth <= innerWidth && document.documentElement.scrollHeight <= innerHeight')
                box = page.locator('#tactical-map').bounding_box()
                assert box['height'] >= 110 and box['width'] >= 200, box
                for control in ['queue-shot', 'track-aim', 'commit', 'next-turn']:
                    page.locator('#' + control).scroll_into_view_if_needed()
                    expect(page.locator('#' + control)).to_be_in_viewport()
                if width == 390:
                    touch_aim = state()['mission']['aim']['target']
                    point = handle_point()
                    client = page.context.new_cdp_session(page)
                    client.send('Input.dispatchTouchEvent', {'type': 'touchStart', 'touchPoints': [point]})
                    client.send('Input.dispatchTouchEvent', {'type': 'touchMove', 'touchPoints': [{'x': point['x'] - 25, 'y': point['y'] - 10}]})
                    client.send('Input.dispatchTouchEvent', {'type': 'touchEnd', 'touchPoints': []})
                    page.locator('#queue-shot').click()
                    assert state()['mission']['queued'][0]['target'] != touch_aim
                    page.locator('#queue-shot').click()
                    expect(page.locator('#shot-queue .queued-shot')).to_have_count(2)
                    page.screenshot(path=str(artifacts / 'mission-touch-queue.png'))
                    first_id = state()['mission']['queued'][0]['id']
                    page.get_by_role('button', name=f'Cancel shot {first_id}', exact=True).click()
                    assert len(state()['mission']['queued']) == 1
                    page.locator('#cancel-shot').click()
                    client.detach()
                page.screenshot(path=str(artifacts / f'mission-{width}x{height}.png'))
            assert not errors, errors
            print('PASS: observed prediction, pointer/touch aiming, four-round 30-second salvo/locked orders/individual cancel/full queue, boundary firing, persistence, no-attack defeat, seven-shot non-hit victory, frozen results, retry, and responsive layout')
        except Exception:
            page.screenshot(path=str(artifacts / 'mission-failure.png'), full_page=True)
            raise
        finally:
            (artifacts / 'mission-browser-errors.txt').write_text('\n'.join(errors) or 'No browser errors.\n')
            browser.close()


if __name__ == '__main__':
    main()

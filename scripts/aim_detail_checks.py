"""Check the passive fine-aim inset against the observation-only shot preview."""

import re
from pathlib import Path
from playwright.sync_api import expect
from browser_controls import read_ui_settings


def check_aim_detail(page):
    settings = read_ui_settings(page)
    detail = page.locator('#aim-detail')
    gain = settings['AIM']['fineGain']
    zoom = settings['AIM_DETAIL']['magnification']
    linger = settings['AIM_DETAIL']['lingerMs']

    def point(selector='#aim-grip .grip-control'):
        return page.locator(selector).evaluate('''e => {
            const p = new DOMPoint(0, 0).matrixTransform(e.getScreenCTM());
            return {x: p.x, y: p.y};
        }''')

    def begin():
        p = point()
        page.mouse.move(p['x'], p['y'])
        page.mouse.down()
        expect(detail).to_be_visible()
        return p

    def projection():
        return page.evaluate('''async () => {
            const {mapProjection} = await import(browserModule('map-overlay'));
            return mapProjection(document.querySelector('#mission-overlay'), readState().map).metresPerPixel;
        }''')

    def verify_estimate(target, metres_per_pixel, firing_time=None):
        expected = page.evaluate('''({target, firingTime}) => {
            const s = readState(), m = s.mission;
            const shot = JSON.parse(firingTime === null ? wasm.preview_shot(target.x, target.y)
                : wasm.preview_launch_shot(target.x, target.y, firingTime));
            const dt = shot.arrival_seconds - s.time_seconds;
            return {x: m.prediction[0].x + m.contact.velocity.x * dt,
                y: m.prediction[0].y + m.contact.velocity.y * dt};
        }''', {'target': target, 'firingTime': firing_time})
        box = page.locator('#aim-detail-map').bounding_box()
        cx = box['width'] / 2
        cy = (box['height'] - settings['AIM_DETAIL']['scaleAreaPixels']) / 2
        dx = (expected['x'] - target['x']) / metres_per_pixel
        dy = (target['y'] - expected['y']) / metres_per_pixel
        margin = settings['AIM_DETAIL']['markerInsetPixels']
        ratio = max(1, abs(dx) / max(1, cx - margin), abs(dy) / max(1, cy - margin))
        transform = page.locator('.detail-enemy').get_attribute('transform')
        x, y = [float(v) for v in re.search(r'translate\(([^)]+)\)', transform)[1].split()]
        assert abs(x - cx - dx / ratio) < 0.1
        assert abs(y - cy - dy / ratio) < 0.1
        expect(page.locator('#aim-detail-note')).to_have_text('Off view' if ratio > 1 else 'At impact')
        offset = ((expected['x'] - target['x']) ** 2 + (expected['y'] - target['y']) ** 2) ** 0.5
        displayed = float(re.search(r'Offset ([\d,.]+) km', page.locator('#aim-detail-offset').inner_text())[1].replace(',', ''))
        assert abs(displayed - offset / 1000) <= 0.051, (displayed, offset / 1000)
        endpoint = page.locator('.detail-shot').get_attribute('d').split('L')[-1]
        ex, ey = map(float, endpoint.split())
        assert abs(ex - cx) < 0.1 and abs(ey - cy) < 0.1
        track = page.evaluate('readState().mission')
        now = track['prediction'][0]
        route_start = page.locator('.detail-route').get_attribute('d').split('L')[0][1:]
        rx, ry = map(float, route_start.split())
        assert abs(rx - cx - (now['x'] - target['x']) / metres_per_pixel) < 0.1
        assert abs(ry - cy - (target['y'] - now['y']) / metres_per_pixel) < 0.1
        assert 'rotate(' in page.locator('.detail-enemy').get_attribute('transform')

    # Freeze the overview camera before sampling an independent world/pixel scale.
    # Earlier mission trials may leave automatic framing still easing toward its fit.
    page.locator('#zoom-out').click()
    initial = page.evaluate('readState().world')
    expect(detail).to_be_hidden()
    p = point('#aim-handle')
    page.mouse.move(p['x'], p['y'])
    page.mouse.down()
    expect(detail).to_be_hidden()
    page.keyboard.press('Escape')
    page.mouse.up()

    # Mouse fine aiming, including a future firing slot, uses the same arrival estimate.
    for timed in [False, True]:
        if timed:
            page.locator('#forecast-time').focus()
            page.locator('#forecast-time').press('PageUp')
        page.locator('#track-aim').click()
        data = page.evaluate('''() => {
            const s = readState(), live = document.querySelector('#forecast-live').getAttribute('aria-pressed') === 'true';
            const time = live ? null : Number(document.querySelector('#forecast-time').getAttribute('aria-valuenow'));
            const target = live ? s.mission.aim.target : JSON.parse(wasm.forecast_at(time)).aim.target;
            return {target, time};
        }''')
        mpp = projection()
        p = begin()
        panel_before = detail.bounding_box()
        scale = page.locator('.detail-scale').get_attribute('d')
        page.mouse.move(p['x'] - 4, p['y'] - 2, steps=2)
        target = {'x': data['target']['x'] - 4 * mpp * gain, 'y': data['target']['y'] + 2 * mpp * gain}
        verify_estimate(target, mpp / zoom, data['time'])
        assert detail.bounding_box() != panel_before
        assert page.locator('.detail-scale').get_attribute('d') == scale, (scale, page.locator('.detail-scale').get_attribute('d'))
        assert page.evaluate('readState().world') == initial
        assert page.evaluate('readState().mission.queued.length') == 0
        assert detail.evaluate('e => getComputedStyle(e).pointerEvents') == 'none'
        page.mouse.up()
        expect(detail).to_have_attribute('data-phase', 'lingering')
        expect(detail).to_be_hidden(timeout=linger + 2000)

    page.locator('#forecast-live').click()
    page.locator('#track-aim').click()
    # Bring the estimated current contact into the local view as well as its course.
    mpp = projection()
    track = page.evaluate('readState().mission')
    p = begin()
    lead, now = track['aim']['target'], track['prediction'][0]
    page.mouse.move(p['x'] + (now['x'] - lead['x']) / mpp / gain,
                    p['y'] - (now['y'] - lead['y']) / mpp / gain, steps=3)
    assert page.locator('.detail-current').get_attribute('d')
    expect(page.locator('.detail-current-label')).to_be_visible()
    page.keyboard.press('Escape')
    page.mouse.up()

    p = begin()
    page.mouse.move(p['x'] - 70, p['y'] - 10)
    expect(page.locator('#aim-detail-note')).to_have_text('Off view')
    page.keyboard.press('Escape')
    expect(detail).to_be_hidden()
    page.mouse.up()

    # A full queue must clear the old prediction instead of showing a stale marker.
    p = begin()
    page.mouse.move(p['x'] - 2, p['y'])
    page.mouse.up()
    for _ in range(page.evaluate('readState().mission.queue_capacity')):
        page.locator('#queue-shot').click()
    begin()
    expect(page.locator('#aim-detail-note')).to_have_text('No solution')
    assert page.locator('.detail-enemy').get_attribute('d') == ''
    page.keyboard.press('Escape')
    page.mouse.up()
    page.locator('#cancel-shot').click()

    artifacts = Path(__file__).resolve().parents[1] / 'artifacts'
    artifacts.mkdir(exist_ok=True)
    for width, height in [(1360, 1000), (390, 844), (320, 568), (844, 390)]:
        page.set_viewport_size({'width': width, 'height': height})
        page.locator('#track-aim').click()
        p = point()
        client = page.context.new_cdp_session(page)
        client.send('Input.dispatchTouchEvent', {'type': 'touchStart', 'touchPoints': [p]})
        expect(detail).to_be_visible()
        client.send('Input.dispatchTouchEvent', {'type': 'touchMove', 'touchPoints': [{'x': p['x'] - 5, 'y': p['y'] - 3}]})
        panel = detail.bounding_box()
        canvas = page.locator('.canvas-wrap').bounding_box()
        anchor = point()
        near = ((max(panel['x'] - anchor['x'], 0, anchor['x'] - panel['x'] - panel['width'])) ** 2
                + (max(panel['y'] - anchor['y'], 0, anchor['y'] - panel['y'] - panel['height'])) ** 2) ** 0.5
        assert near <= settings['AIM_DETAIL']['handleGapPixels'] + 1, near
        for spot in [p, point('#aim-handle')]:
            assert not (panel['x'] <= spot['x'] <= panel['x'] + panel['width']
                        and panel['y'] <= spot['y'] <= panel['y'] + panel['height'])
        assert canvas['x'] <= panel['x'] and panel['x'] + panel['width'] <= canvas['x'] + canvas['width']
        assert canvas['y'] <= panel['y'] and panel['y'] + panel['height'] <= canvas['y'] + canvas['height']
        assert page.locator('#aim-detail-map').bounding_box()['height'] > 25
        page.screenshot(path=str(artifacts / f'aim-detail-{width}x{height}.png'))
        # A second finger hands control to navigation and removes the inset immediately.
        other = {'x': canvas['x'] + 15, 'y': canvas['y'] + canvas['height'] - 15}
        client.send('Input.dispatchTouchEvent', {'type': 'touchStart', 'touchPoints': [{'x': p['x'] - 5, 'y': p['y'] - 3}, other]})
        expect(detail).to_be_hidden()
        client.send('Input.dispatchTouchEvent', {'type': 'touchEnd', 'touchPoints': []})
        client.detach()

    page.set_viewport_size({'width': 1360, 'height': 1000})
    page.locator('[data-view="overview"]').click()
    page.locator('[data-speed="1"]').click()
    page.locator('#toggle').click()
    expect(page.locator('#mode')).to_have_text('EXECUTING')
    begin()
    assert not page.evaluate('readState().paused')
    page.mouse.up()
    assert not page.evaluate('readState().paused')
    page.locator('#help-open').click()
    expect(detail).to_be_hidden()
    page.keyboard.press('Escape')
    page.evaluate('wasm.command("mission")')
    expect(page.locator('#ticks')).to_have_text('0')
    expect(page.locator('#speed [aria-pressed="true"]')).to_have_text('32×')
    expect(detail).to_be_hidden()
    print('PASS: fine-aim detail matches observed arrival for live/timed shots, fixed scale, linger/cancel, offscreen/unavailable states, pointer/touch/second-finger handoff, running edits, and responsive layout')

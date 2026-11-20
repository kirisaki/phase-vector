"""Real pointer/touch camera checks for the mission browser smoke test."""
from math import isclose

from playwright.sync_api import expect


def check_map_navigation(page, artifacts):
    def state():
        return page.evaluate('readState()')

    def unchanged(before):
        after = state()
        for key in ['world', 'planned_burn', 'mission']:
            assert after[key] == before[key], key

    def overview():
        page.locator('[data-view="overview"]').click()
        page.wait_for_timeout(150)
        assert state()['map']['auto_zoom']

    def box():
        return page.locator('#route-editor').bounding_box()

    def world_point(camera, point):
        rect = box()
        scale = max(2000 / rect['width'], 1200 / rect['height']) * camera['metres_per_unit']
        return (camera['center']['x'] + (point['x'] - rect['x'] - rect['width'] / 2) * scale,
                camera['center']['y'] - (point['y'] - rect['y'] - rect['height'] / 2) * scale)

    def near(a, b):
        assert all(isclose(x, y, rel_tol=1e-7, abs_tol=10) for x, y in zip(a, b)), (a, b)

    def released():
        expect(page.locator('.canvas-wrap')).not_to_have_attribute('data-navigating', 'true')
        expect(page.locator('[data-view="overview"]')).to_be_enabled()
        expect(page.locator('#commit')).to_be_enabled()

    page.set_viewport_size({'width': 1360, 'height': 1000})
    page.wait_for_timeout(200)
    overview()
    before = state()
    rect = box()
    start = {'x': rect['x'] + rect['width'] / 2, 'y': rect['y'] + 30}
    page.mouse.click(**start)
    assert state()['map']['auto_zoom']  # A click alone does not detach the camera.
    page.mouse.move(**start)
    page.evaluate('''document.addEventListener('pointerdown', e => {
        window.dragStart = {camera: readState().map, point: {x: e.clientX, y: e.clientY}};
    }, {capture: true, once: true})''')
    page.mouse.down()
    initial = page.evaluate('dragStart')
    end = {'x': start['x'] + 80, 'y': start['y'] + 45}
    page.mouse.move(**end, steps=5)
    page.mouse.up()
    camera = state()['map']
    assert camera['focus'] == 'free' and not camera['auto_zoom']
    near(world_point(initial['camera'], initial['point']), world_point(camera, end))
    unchanged(before)
    released()
    # Moving the ship against an edge clips the Burn ring without shrinking it.
    ring = page.locator('.reach-area')
    radius = float(ring.get_attribute('r'))
    cx, cy = float(ring.get_attribute('cx')), float(ring.get_attribute('cy'))
    start = {'x': rect['x'] + rect['width'] / 2, 'y': rect['y'] + rect['height'] - 25}
    page.mouse.move(**start)
    page.mouse.down()
    page.mouse.move(start['x'] + 8 - cx, start['y'] + 8 - cy, steps=5)
    page.mouse.up()
    assert float(ring.get_attribute('r')) == radius
    assert all(isclose(float(ring.get_attribute(axis)), 8, abs_tol=0.001) for axis in ['cx', 'cy'])
    unchanged(before)
    camera = state()['map']
    # Manual framing stays fixed while the simulation advances.
    page.locator('[data-speed="32"]').click()
    page.locator('#toggle').click()
    page.wait_for_function('readState().world.tick >= 8')
    page.locator('#toggle').click()
    assert state()['map'] == camera
    page.evaluate("wasm.command('reset')")
    page.wait_for_timeout(150)

    page.set_viewport_size({'width': 390, 'height': 844})
    page.wait_for_timeout(250)
    overview()
    client = page.context.new_cdp_session(page)

    def touch(kind, *points):
        client.send('Input.dispatchTouchEvent', {'type': kind, 'touchPoints': list(points)})

    def finger(x, y, ident):
        return {'x': x, 'y': y, 'id': ident}

    try:
        before = state()
        rect = box()
        x, y = rect['x'] + rect['width'] / 2, rect['y'] + 25
        a, b = finger(x - 30, y, 1), finger(x + 30, y, 2)
        touch('touchStart', a, b)
        initial = state()['map']
        a, b = finger(x - 50, y + 12, 1), finger(x + 70, y + 12, 2)
        touch('touchMove', a, b)
        camera = state()['map']
        assert isclose(camera['metres_per_unit'], initial['metres_per_unit'] / 2, rel_tol=1e-6)
        near(world_point(initial, {'x': x, 'y': y}),
             world_point(camera, {'x': x + 10, 'y': y + 12}))
        # Lifting one finger rebases the remaining drag without a jump.
        touch('touchEnd', b)
        near(world_point(camera, a), world_point(state()['map'], a))
        moved = finger(a['x'] + 20, a['y'] + 8, 1)
        touch('touchMove', moved)
        near(world_point(camera, a), world_point(state()['map'], moved))
        touch('touchEnd')
        unchanged(before)
        released()
        page.screenshot(path=str(artifacts / 'map-touch-navigation.png'))
        assert page.evaluate('visualViewport.scale === 1 && document.documentElement.scrollWidth <= innerWidth && document.documentElement.scrollHeight <= innerHeight')

        # A second finger takes over an in-progress order edit and discards its draft.
        for selector in ['#arrival-handle', '#aim-handle']:
            overview()
            before = state()
            point = page.locator(selector).evaluate('''h => {
                const p = new DOMPoint(0, 0).matrixTransform(h.getScreenCTM());
                return {x: p.x, y: p.y};
            }''')
            a = finger(point['x'], point['y'], 1)
            touch('touchStart', a)
            expect(page.locator('#commit')).to_be_disabled()
            a = finger(point['x'] + 10, point['y'] - 5, 1)
            touch('touchMove', a)
            b = finger(rect['x'] + rect['width'] / 2, rect['y'] + 20, 2)
            touch('touchStart', a, b)
            touch('touchMove', a, finger(b['x'] + 20, b['y'] + 5, 2))
            touch('touchEnd')
            released()
            assert state()['map']['focus'] == 'free'
            unchanged(before)

        # Cancellation leaves no captured gesture or disabled controls behind.
        overview()
        before = state()
        a = finger(x, y, 1)
        touch('touchStart', a)
        touch('touchMove', finger(x + 25, y + 10, 1))
        touch('touchCancel')
        released()
        unchanged(before)
        overview()
        page.mouse.move(x, y)
        page.mouse.down()
        page.mouse.move(x + 20, y + 10)
        page.keyboard.press('Escape')
        page.mouse.up()
        released()
        overview()
    finally:
        client.detach()

    page.set_viewport_size({'width': 1360, 'height': 1000})
    page.wait_for_timeout(250)
    overview()

    print('PASS: mouse pan, anchored mobile pinch/pan, one-finger continuation, editor draft cancellation, pointer cancellation, Escape, auto-fit recovery, and unchanged commands')

"""Exercise firing-time planning through the actual range, weapon, and maneuver controls."""
from playwright.sync_api import expect
from browser_controls import read_ui_settings, timeline_point


def check_planning(page, artifacts):
    check_reload_markers(page, artifacts)
    check_queue_while_scrubbing(page)
    def state():
        return page.evaluate('readState()')

    near_share = read_ui_settings(page)['TIMELINE']['nearShare']

    def select_firing(fraction=near_share):
        page.mouse.click(**timeline_point(page, fraction))
        expect(page.locator('#forecast-label')).to_contain_text('Fire')
        return float(page.locator('#forecast-time').get_attribute('aria-valuenow'))

    before = state()['world']
    # The imminent interval is expanded; pointer input snaps gently, keys remain precise.
    slider = page.locator('#forecast-time')
    deadline = state()['mission']['deadline_seconds']
    for position, seconds in [(near_share, 120), (near_share + (1 - near_share) * (480 - 120) / (deadline - 120), 480)]:
        page.mouse.click(**timeline_point(page, position, offset_pixels=0.2))
        expect(slider).to_have_attribute('aria-valuenow', str(seconds))
        slider.focus()
        page.keyboard.press('ArrowRight')
        expect(slider).to_have_attribute('aria-valuenow', str(seconds + state()['timing']['step_seconds']))
    expect(page.locator('#forecast-boundary')).to_have_text('120 s')
    assert page.locator('#forecast-axis').evaluate("e => getComputedStyle(e).backgroundImage.includes('77, 223, 217')")
    assert state()['world'] == before
    page.locator('#forecast-live').click()
    firing = select_firing()
    assert firing == 120
    expect(page.locator('.forecast-own')).to_have_count(1)
    expect(page.locator('.forecast-enemy')).to_have_count(1)
    expect(page.locator('#queue-shot')).to_be_enabled()
    assert state()['world'] == before
    page.locator('#queue-shot').click()
    order = state()['mission']['queued'][0]
    assert order['launch_tick'] * state()['timing']['step_seconds'] == firing
    assert order['requested_arrival_seconds'] is None
    assert order['solution']['arrival_seconds'] > firing + 300
    forecast = page.evaluate('(time) => JSON.parse(wasm.forecast_at(time))', firing)
    assert order['target'] == forecast['aim']['target']
    assert abs(order['target']['y'] - forecast['enemy_position']['y']) > 10_000_000
    # The rendered AIM must be beyond the enemy's position at the firing instant.
    assert page.evaluate('''() => {
        const a = document.querySelector('#aim-handle').transform.baseVal.consolidate().matrix;
        const e = document.querySelector('.forecast-enemy').transform.baseVal.consolidate().matrix;
        return Math.hypot(a.e - e.e, a.f - e.f) > 5;
    }''')
    expect(page.locator('.forecast-round')).to_have_count(1)
    expect(page.locator('#queue-shot')).to_be_disabled()
    expect(page.locator('#gun-feedback')).to_contain_text('reload')
    # A numeric maneuver is still the normal shared order API, and recomputes ballistics at the reserved firing tick.
    page.locator('#numeric-burn summary').click()
    page.locator('#burn-x').fill(str(state()['world']['ships'][0]['max_acceleration']))
    page.locator('#apply-burn').click()
    revised = state()['mission']['queued'][0]
    assert revised['id'] == order['id'] and revised['target'] == order['target']
    assert revised['requested_arrival_seconds'] is None
    assert revised['launch_tick'] == order['launch_tick']
    assert revised['solution']['arrival_seconds'] != order['solution']['arrival_seconds']
    assert not revised['error']
    page.locator('#numeric-burn summary').click()
    page.screenshot(path=str(artifacts / 'planning-desktop.png'))
    # Time scrubbing changes no world state; it never rewrites the stored order.
    page.locator('#forecast-time').focus()
    page.keyboard.press('ArrowRight')
    assert state()['mission']['queued'][0] == revised
    assert state()['world'] == before
    page.get_by_role('button', name=f'Cancel shot {order["id"]}', exact=True).click()
    expect(page.locator('.forecast-round')).to_have_count(0)
    page.locator('#forecast-time').focus()
    page.keyboard.press('Home')
    page.keyboard.press('ArrowRight')
    expect(page.locator('#queue-shot')).to_be_enabled()
    # New orders cannot modify the active interval, even if it is paused.
    page.locator('#step').click()
    page.locator('#forecast-time').focus()
    page.keyboard.press('Home')
    page.keyboard.press('ArrowRight')
    expect(page.locator('#queue-shot')).to_be_disabled()
    expect(page.locator('#gun-feedback')).to_contain_text('locked')
    page.locator('#forecast-live').click()
    expect(page.locator('#time-forecast > *')).to_have_count(0)
    expect(page.locator('#queue-shot')).to_be_enabled()
    page.evaluate("wasm.command('reset')")
    page.wait_for_timeout(200)
    # A fixed firing reservation survives a boundary and fires at its scheduled tick in a later turn.
    firing = select_firing(0.45)
    page.locator('#queue-shot').click()
    queued = state()['mission']['queued'][0]
    page.locator('#next-turn').click()
    assert state()['mission']['queued'][0]['id'] == queued['id']
    assert not state()['mission']['shots']
    while state()['world']['tick'] <= queued['launch_tick']:
        page.locator('#next-turn').click()
    shot = state()['mission']['shots'][0]
    assert shot['order_id'] == queued['id']
    assert shot['launch_tick'] == queued['launch_tick']
    assert shot['launch_tick'] * state()['timing']['step_seconds'] == firing
    assert abs(shot['arrival_seconds'] - queued['solution']['arrival_seconds']) < 1e-6
    # Passing the selected preview time returns to Live without pausing the simulation.
    page.locator('#forecast-time').focus()
    page.keyboard.press('Home')
    page.keyboard.press('ArrowRight')
    page.locator('#step').click()
    expect(page.locator('#forecast-live')).to_have_attribute('aria-pressed', 'true')
    expect(page.locator('#time-forecast > *')).to_have_count(0)
    page.evaluate("wasm.command('reset')")
    page.wait_for_timeout(200)
    # Freeze the scale under a held pointer while Auto continues advancing.
    page.locator('#toggle').click()
    page.wait_for_function('readState().world.tick > 0')
    box = slider.bounding_box()
    page.mouse.move(box['x'] + box['width'] * 0.6, box['y'] + box['height'] / 2)
    page.mouse.down()
    held_min = slider.get_attribute('aria-valuemin')
    tick = state()['world']['tick']
    page.wait_for_function('(tick) => readState().world.tick > tick + 40', arg=tick)
    assert slider.get_attribute('aria-valuemin') == held_min
    page.mouse.up()
    page.locator('#toggle').click()
    assert float(slider.get_attribute('aria-valuemin')) > float(held_min)
    page.evaluate("wasm.command('reset')")
    page.wait_for_timeout(200)
    for width, height in [(390, 844), (320, 568), (844, 390)]:
        page.set_viewport_size({'width': width, 'height': height})
        page.wait_for_timeout(250)
        # Flush the resized compositor frame before CDP sends absolute touch coordinates.
        page.screenshot()
        box = page.locator('#forecast-time').bounding_box()
        client = page.context.new_cdp_session(page)
        point = {'x': box['x'] + box['width'] / 2, 'y': box['y'] + box['height'] / 2}
        client.send('Input.dispatchTouchEvent', {'type': 'touchStart', 'touchPoints': [point]})
        client.send('Input.dispatchTouchEvent', {'type': 'touchMove', 'touchPoints': [{'x': point['x'] + 8, 'y': point['y']}]})
        client.send('Input.dispatchTouchEvent', {'type': 'touchEnd', 'touchPoints': []})
        expect(page.locator('#forecast-label')).to_contain_text('Fire')
        assert state()['world'] == before
        expect(page.locator('#forecast-live')).to_be_in_viewport()
        assert page.evaluate('document.documentElement.scrollWidth <= innerWidth && document.documentElement.scrollHeight <= innerHeight')
        page.screenshot(path=str(artifacts / f'planning-{width}.png'))
        page.locator('#forecast-live').click()
        client.detach()
    page.set_viewport_size({'width': 1360, 'height': 1000})
    page.wait_for_timeout(250)
    print('PASS: expanded colored timeline, boundary snapping, precise keys, stable running drag, selected firing time and lead beyond the forecast, firing across turns, reload feedback, maneuver ballistics, immutable reservations, active-interval lock, Live, and touch layouts')


def check_queue_while_scrubbing(page):
    def state():
        return page.evaluate('readState()')

    def reset():
        page.evaluate("wasm.command('reset')")
        page.wait_for_timeout(200)

    slider = page.locator('#forecast-time')
    queue = page.locator('#queue-shot')
    def move(fraction):
        page.mouse.move(**timeline_point(page, fraction))

    move(0.6)
    page.mouse.down()
    arrival = float(slider.get_attribute('aria-valuenow'))
    page.keyboard.down('q')
    first = state()['mission']['queued'][0]
    assert first['launch_tick'] * state()['timing']['step_seconds'] == arrival
    assert first['requested_arrival_seconds'] is None
    move(0.66)
    page.keyboard.down('q')  # Repeated keydown while held must not queue again.
    assert state()['mission']['queued'] == [first]
    page.keyboard.up('q')
    page.keyboard.press('q')
    assert len(state()['mission']['queued']) == 2
    assert state()['mission']['queued'][0] == first
    for fraction in [0.72, 0.78]:
        move(fraction)
        page.keyboard.press('q')
    assert len(state()['mission']['queued']) == 4
    move(0.84)
    expect(queue).to_be_disabled()
    page.keyboard.press('q')
    assert len(state()['mission']['queued']) == 4
    page.mouse.up()
    reset()
    move(0.3)
    page.mouse.down()
    page.keyboard.press('q')
    assert len(state()['mission']['queued']) == 1
    assert state()['mission']['queued'][0]['launch_tick'] * state()['timing']['step_seconds'] == 90
    page.mouse.up()
    reset()
    move(0.6)
    page.mouse.click(**timeline_point(page, 0.6))
    expect(queue).to_be_enabled()
    page.locator('#console-toggle').click()
    page.locator('#lua-input').fill('')
    page.locator('#lua-input').press('q')
    expect(page.locator('#lua-input')).to_have_value('q')
    assert not state()['mission']['queued']
    page.locator('#console-close').click()
    reset()

    # Two real touch pointers, including release/cancel while Auto advances.
    page.set_viewport_size({'width': 390, 'height': 844})
    page.wait_for_timeout(250)
    page.locator('[data-speed="1"]').click()
    page.locator('#toggle').click()
    page.wait_for_function('readState().world.tick > 0')
    client = page.context.new_cdp_session(page)

    def finger(fraction):
        return {'id': 1, **timeline_point(page, fraction)}

    def queue_finger():
        rect = queue.bounding_box()
        return {'id': 2, 'x': rect['x'] + rect['width'] / 2, 'y': rect['y'] + rect['height'] / 2}

    def touch(kind, points):
        client.send('Input.dispatchTouchEvent', {'type': kind, 'touchPoints': points})

    held = finger(0.6)
    touch('touchStart', [held])
    arrival = float(slider.get_attribute('aria-valuenow'))
    frozen_min = slider.get_attribute('aria-valuemin')
    touch('touchStart', [held, queue_finger()])
    touch('touchEnd', [queue_finger()])
    assert len(state()['mission']['queued']) == 1
    first = state()['mission']['queued'][0]
    assert first['launch_tick'] * state()['timing']['step_seconds'] == arrival
    assert first['requested_arrival_seconds'] is None
    tick = state()['world']['tick']
    page.wait_for_function('(tick) => readState().world.tick > tick + 4', arg=tick)
    assert slider.get_attribute('aria-valuemin') == frozen_min
    held = finger(0.68)
    touch('touchMove', [held])
    assert float(slider.get_attribute('aria-valuenow')) > arrival + 30
    expect(queue).to_be_enabled()
    touch('touchStart', [held, queue_finger()])
    touch('touchEnd', [queue_finger()])
    assert len(state()['mission']['queued']) == 2
    assert state()['mission']['queued'][0] == first
    assert slider.get_attribute('aria-valuemin') == frozen_min
    held = finger(0.76)
    touch('touchMove', [held])
    expect(queue).to_be_enabled()
    other = queue_finger()
    touch('touchStart', [held, other])
    outside = {**other, 'y': other['y'] - 60}
    touch('touchMove', [held, outside])
    touch('touchEnd', [outside])
    assert len(state()['mission']['queued']) == 2
    assert slider.get_attribute('aria-valuemin') == frozen_min
    touch('touchStart', [held, queue_finger()])
    touch('touchCancel', [])
    assert len(state()['mission']['queued']) == 2
    assert float(slider.get_attribute('aria-valuemin')) > float(frozen_min)
    client.detach()
    page.locator('#toggle').click()
    reset()
    page.locator('[data-speed="32"]').click()
    page.set_viewport_size({'width': 1360, 'height': 1000})
    page.wait_for_timeout(250)
    print('PASS: Q during drag, key-repeat/full guards and early firing, Lua typing, two-finger Queue during Auto, immutable orders, retained scale, outside release, and touch cancellation')


def check_reload_markers(page, artifacts):
    slider = page.locator('#forecast-time')
    queue = page.locator('#queue-shot')
    marks = page.locator('#forecast-ready-marks i')
    near_share = read_ui_settings(page)['TIMELINE']['nearShare']

    def reset():
        page.evaluate("wasm.command('reset')")
        page.wait_for_timeout(200)
        page.locator('#forecast-live').click()

    def times():
        return marks.evaluate_all('nodes => nodes.map(n => Number(n.dataset.seconds))')

    def choose(seconds, offset_pixels=0):
        deadline = page.evaluate('readState().mission.deadline_seconds')
        fraction = near_share * seconds / 120 if seconds <= 120 else near_share + (1 - near_share) * (seconds - 120) / (deadline - 120)
        page.mouse.click(**timeline_point(page, fraction, offset_pixels))

    assert times() == [0]
    queue.click()
    assert times() == [30]
    choose(30, -2)
    expect(slider).to_have_attribute('aria-valuenow', '30')
    expect(queue).to_be_enabled()
    slider.focus()
    page.keyboard.press('ArrowLeft')
    expect(slider).to_have_attribute('aria-valuenow', '29.75')
    expect(queue).to_be_disabled()
    choose(60)  # Move off the thumb before approaching the readiness marker again.
    choose(30, -2)
    page.keyboard.press('q')
    assert times() == [60]
    choose(120)
    queue.click()
    assert times() == [60, 150]
    orders = page.evaluate('readState().mission.queued')
    page.get_by_role('button', name=f'Cancel shot {orders[1]["id"]}', exact=True).click()
    assert times() == [30, 150]
    choose(30, -2)
    queue.click()
    choose(60, -2)
    queue.click()
    expect(marks).to_have_count(0)
    expect(page.locator('#forecast-ready-label')).to_be_hidden()
    page.locator('#cancel-shot').click()
    assert times() == [0]
    # Reload at 122 seconds must win over the blocked 120-second command tick.
    choose(92)
    expect(slider).to_have_attribute('aria-valuenow', '92')
    queue.click()
    assert times() == [0, 122]
    choose(120)
    expect(slider).to_have_attribute('aria-valuenow', '122')
    expect(queue).to_be_enabled()
    expect(page.locator('#forecast-ready-label')).to_have_text('Ready 122 s')
    page.screenshot(path=str(artifacts / 'reload-marker-desktop.png'))
    page.set_viewport_size({'width': 390, 'height': 844})
    page.wait_for_timeout(250)
    client = page.context.new_cdp_session(page)
    point = timeline_point(page, near_share)
    client.send('Input.dispatchTouchEvent', {'type': 'touchStart', 'touchPoints': [point]})
    client.send('Input.dispatchTouchEvent', {'type': 'touchEnd', 'touchPoints': []})
    expect(slider).to_have_attribute('aria-valuenow', '122')
    expect(queue).to_be_enabled()
    page.screenshot(path=str(artifacts / 'reload-marker-phone.png'))
    client.detach()
    reset()
    queue.click()
    page.locator('#step').click()
    assert times() == [120]  # The current interval is already committed.
    reset()
    page.set_viewport_size({'width': 1360, 'height': 1000})
    page.wait_for_timeout(250)
    print('PASS: firing-window markers, readiness snap over command ticks, keyboard bypass, cancel/gaps/full queue, touch, and committed interval')

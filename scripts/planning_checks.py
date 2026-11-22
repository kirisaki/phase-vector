"""Exercise future planning through the actual range, weapon, and maneuver controls."""
from playwright.sync_api import expect


def check_planning(page, artifacts):
    def state():
        return page.evaluate('readState()')

    def select_middle():
        box = page.locator('#forecast-time').bounding_box()
        page.mouse.click(box['x'] + box['width'] / 2, box['y'] + box['height'] / 2)
        expect(page.locator('#forecast-label')).to_contain_text('Preview')
        return float(page.locator('#forecast-time').input_value())

    before = state()['world']
    arrival = select_middle()
    assert 400 < arrival < 800
    expect(page.locator('.forecast-own')).to_have_count(1)
    expect(page.locator('.forecast-enemy')).to_have_count(1)
    expect(page.locator('#queue-shot')).to_be_enabled()
    assert state()['world'] == before
    page.locator('#queue-shot').click()
    order = state()['mission']['queued'][0]
    assert order['requested_arrival_seconds'] == arrival
    assert abs(order['solution']['arrival_seconds'] - arrival) <= state()['mission']['arrival_tolerance_seconds']
    expect(page.locator('.forecast-round')).to_have_count(1)
    expect(page.locator('#queue-shot')).to_be_disabled()
    expect(page.locator('#gun-feedback')).to_contain_text('reload')
    # A numeric maneuver is still the normal shared order API, and replans timed shots.
    page.locator('#numeric-burn summary').click()
    page.locator('#burn-x').fill(str(state()['world']['ships'][0]['max_acceleration']))
    page.locator('#apply-burn').click()
    revised = state()['mission']['queued'][0]
    assert revised['id'] == order['id'] and revised['target'] == order['target']
    assert revised['requested_arrival_seconds'] == arrival
    assert revised['launch_tick'] != order['launch_tick']
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
    expect(page.locator('#queue-shot')).to_be_disabled()
    expect(page.locator('#gun-feedback')).to_contain_text('Unreachable arrival')
    page.locator('#forecast-live').click()
    expect(page.locator('#time-forecast > *')).to_have_count(0)
    expect(page.locator('#queue-shot')).to_be_enabled()
    page.evaluate("wasm.command('reset')")
    page.wait_for_timeout(200)
    # A timed reservation survives a boundary and fires at its scheduled tick in a later turn.
    arrival = select_middle()
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
    assert abs(shot['arrival_seconds'] - arrival) <= state()['mission']['arrival_tolerance_seconds']
    # Passing the selected preview time returns to Live without pausing the simulation.
    page.locator('#forecast-time').focus()
    page.keyboard.press('Home')
    page.keyboard.press('ArrowRight')
    page.locator('#step').click()
    expect(page.locator('#forecast-live')).to_have_attribute('aria-pressed', 'true')
    expect(page.locator('#time-forecast > *')).to_have_count(0)
    page.evaluate("wasm.command('reset')")
    page.wait_for_timeout(200)
    for width, height in [(390, 844), (320, 568), (844, 390)]:
        page.set_viewport_size({'width': width, 'height': height})
        page.wait_for_timeout(250)
        box = page.locator('#forecast-time').bounding_box()
        client = page.context.new_cdp_session(page)
        point = {'x': box['x'] + box['width'] / 2, 'y': box['y'] + box['height'] / 2}
        client.send('Input.dispatchTouchEvent', {'type': 'touchStart', 'touchPoints': [point]})
        client.send('Input.dispatchTouchEvent', {'type': 'touchMove', 'touchPoints': [{'x': point['x'] + 8, 'y': point['y']}]})
        client.send('Input.dispatchTouchEvent', {'type': 'touchEnd', 'touchPoints': []})
        expect(page.locator('#forecast-label')).to_contain_text('Preview')
        assert state()['world'] == before
        expect(page.locator('#forecast-live')).to_be_in_viewport()
        assert page.evaluate('document.documentElement.scrollWidth <= innerWidth && document.documentElement.scrollHeight <= innerHeight')
        page.screenshot(path=str(artifacts / f'planning-{width}.png'))
        page.locator('#forecast-live').click()
        client.detach()
    page.set_viewport_size({'width': 1360, 'height': 1000})
    page.wait_for_timeout(250)
    print('PASS: future slider, timed arrival and firing across turns, reload feedback, maneuver replanning, immutable reservations, unreachable arrival, Live, and touch layouts')

"""Exercise moving cover and maneuver-driven shot replanning through real controls."""
from playwright.sync_api import expect


def plan_burn(page, fraction_x, fraction_y):
    points = page.locator('#arrival-handle').evaluate('''(h, [fx, fy]) => {
        const start = new DOMPoint(0,0).matrixTransform(h.getScreenCTM());
        const ring = document.querySelector('.reach-area');
        const end = new DOMPoint(+ring.getAttribute('cx') + +ring.getAttribute('r') * fx,
            +ring.getAttribute('cy') - +ring.getAttribute('r') * fy).matrixTransform(ring.getScreenCTM());
        return {start: {x:start.x,y:start.y}, end:{x:end.x,y:end.y}};
    }''', [fraction_x, fraction_y])
    page.mouse.move(**points['start'])
    page.mouse.down()
    page.mouse.move(**points['end'], steps=5)
    page.mouse.up()


def plan_slowdown(page):
    plan_burn(page, 0, -1.05)
    page.wait_for_function('readState().planned_burn.y < -49 && Math.abs(readState().planned_burn.x) < .001')


def coast(page):
    page.locator('#arrival-handle').focus()
    page.keyboard.press('Home')
    page.wait_for_function('Math.hypot(readState().planned_burn.x,readState().planned_burn.y) < .001')


def check_asteroids(page, artifacts):
    def state(): return page.evaluate('readState()')
    original = state()['world']
    assert len(original['asteroids']) == 2
    expect(page.locator('#asteroid-map circle')).to_have_count(2)
    expect(page.locator('#gun-feedback')).to_contain_text('Blocked by A1')
    expect(page.locator('#cover-contact text')).to_contain_text('Blocked')
    # Selection changes both the firing solution and the known asteroid position.
    page.locator('#forecast-time').focus()
    page.keyboard.press('PageUp')
    page.keyboard.press('PageUp')
    expect(page.locator('#forecast-time')).to_have_attribute('aria-valuenow', '240')
    expect(page.locator('#asteroid-forecast text')).to_have_text(['A1 · 240 s', 'A2 · 240 s'])
    page.wait_for_function('''async () => {
        const {mapProjection}=await import(browserModule('map-overlay'));
        const s=readState(), svg=document.querySelector('#mission-overlay');
        return s.world.asteroids.every(a => {
        const v=mapProjection(svg,s.map), [x,y]=v.project({x:a.position.x+a.velocity.x*240,y:a.position.y+a.velocity.y*240});
        const g=document.querySelector(`#asteroid-forecast g[data-asteroid-id="${a.id}"]`);
        return g.getAttribute('transform')===`translate(${x} ${y})`;
        });
    }''')
    page.locator('#queue-shot').click()
    order=state()['mission']['queued'][0]
    assert order['solution']['obstruction'] is not None
    plan_slowdown(page)
    updated=state()['mission']['queued'][0]
    assert updated['id']==order['id'] and updated['target']==order['target'] and updated['launch_tick']==order['launch_tick']
    assert updated['solution']['obstruction'] is None
    assert state()['world']==original
    for width,height in [(1360,1000),(390,844)]:
        page.set_viewport_size({'width':width,'height':height})
        for circle in page.locator('#asteroid-map circle').all(): expect(circle).to_be_visible()
        for circle in page.locator('#asteroid-forecast circle').all(): expect(circle).to_be_visible()
        # Enemy-side cover labels face inward and avoid the current motion labels.
        page.wait_for_function('''() => {
            const a=document.querySelector('#asteroid-map g[data-asteroid-id="2"] text').getBoundingClientRect();
            const map=document.querySelector('#mission-overlay').getBoundingClientRect();
            return a.left >= map.left && a.right <= map.right &&
                ['enemy-speed','enemy-motion'].every(id => {
                    const b=document.getElementById(id).getBoundingClientRect();
                    return a.right <= b.left || a.left >= b.right || a.bottom <= b.top || a.top >= b.bottom;
                });
        }''')
        page.screenshot(path=str(artifacts / f'asteroid-plan-{width}.png'))
    page.set_viewport_size({'width':1360,'height':1000})
    page.locator('#retry').click()
    page.locator('#retry-confirm').click()
    page.wait_for_function('readState().mission.queued.length === 0 && readState().planned_burn.y === 0')
    page.locator('#queue-shot').click()
    page.locator('#next-turn').click()
    page.wait_for_function('readState().time_seconds === 120')
    assert state()['world']['projectiles']==[]
    assert len(state()['world']['asteroid_impacts'])==1
    expect(page.locator('#shot-labels text')).to_have_count(0)
    expect(page.locator('#shot-paths path')).to_have_count(0)
    page.locator('#retry').click()
    page.locator('#retry-confirm').click()
    page.wait_for_function('readState().time_seconds === 0')
    assert state()['world']==original
    # A deliberately unsafe map-authored route must predict and produce own-ship loss.
    own=original['ships'][0]
    asteroid=original['asteroids'][0]
    intercept=1000.0
    factor=0.5*intercept*(intercept+state()['timing']['step_seconds'])
    acceleration={axis:(asteroid['position'][axis]+asteroid['velocity'][axis]*intercept
        -own['position'][axis]-own['velocity'][axis]*intercept)/factor for axis in ['x','y']}
    plan_burn(page, acceleration['x']/own['max_acceleration'], acceleration['y']/own['max_acceleration'])
    page.locator('#forecast-time').focus()
    for _ in range(9): page.keyboard.press('PageUp')
    expect(page.locator('#forecast-label')).to_contain_text('COLLISION')
    expect(page.locator('#cover-contact text')).to_contain_text('Collision')
    for _ in range(9):
        page.locator('#next-turn').click()
        page.wait_for_timeout(120)
        if state()['mission']['outcome'] != 'running': break
    assert state()['mission']['outcome']=='own_disabled'
    expect(page.locator('#result-title')).to_have_text('DEFEAT')
    expect(page.locator('#result-reason')).to_have_text('Your ship collided with an asteroid.')
    expect(page.locator('#result-score')).to_have_text('0')
    page.screenshot(path=str(artifacts / 'asteroid-collision.png'))
    page.locator('#result-retry').click()
    page.wait_for_function('readState().time_seconds === 0')
    assert state()['world']==original
    print('PASS: known asteroid motion, timeline ephemeris, physical cover, disappeared round trails, unchanged reservations recalculated after a map-authored maneuver, predicted/executed own-ship collision, and desktop/phone layouts')

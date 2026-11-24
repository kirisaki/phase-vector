"""Exercise help navigation without executing inserted Lua examples."""

from pathlib import Path
from playwright.sync_api import expect
from browser_controls import connect_engine


def check_help(page):
    dialog = page.locator('#help-dialog')
    body = page.locator('#help-body')
    opener = page.locator('#help-open')
    expected = page.evaluate("/^ja(?:-|$)/i.test(navigator.language) ? 'ja' : 'en'")
    expect(dialog).to_have_attribute('lang', expected)
    # Exercise both continuous and boundary-limited playback; help never resumes either.
    for control in ['#toggle', '#commit']:
        page.locator('[data-speed="1"]').click()
        page.locator(control).click()
        expect(page.locator('#mode')).to_have_text('EXECUTING')
        opener.click()
        expect(dialog).to_be_visible()
        expect(page.locator('#help-close')).to_be_focused()
        paused = page.evaluate('readState()')
        assert paused['paused'] and not paused['stop_at_boundary']
        page.wait_for_timeout(250)
        assert page.evaluate('readState().world.tick') == paused['world']['tick']
        page.keyboard.press('Escape')
        expect(dialog).to_be_hidden()
        expect(opener).to_be_focused()
        page.wait_for_timeout(250)
        assert page.evaluate('readState().world.tick') == paused['world']['tick']

    # Switching language preserves the section, expanded references and game state.
    opener.click()
    page.locator('[data-help-language="ja"]').click()
    page.locator('[data-help-topic="lua"]').click()
    page.locator('[data-help-group="settings"] summary').click()
    state = page.evaluate('readState()')
    page.locator('[data-help-language="en"]').click()
    expect(body).to_have_attribute('data-topic', 'lua')
    expect(page.locator('[data-help-group="settings"]')).to_have_attribute('open', '')
    assert page.evaluate('readState()') == state
    expect(page.locator('html')).to_have_attribute('lang', 'en')
    expect(page.locator('#commit')).to_have_text('Commit')
    expect(page.locator('#help-close')).to_be_in_viewport()
    page.keyboard.press('Tab')
    assert page.evaluate("document.querySelector('#help-dialog').contains(document.activeElement)")

    # Inserting a destructive command must only fill and focus the existing input.
    source = 'game.command("mission")'
    page.locator('[data-help-insert]').filter(has_text='Insert into REPL').last.click()
    expect(dialog).to_be_hidden()
    expect(page.locator('#lua-input')).to_have_value(source)
    expect(page.locator('#lua-input')).to_be_focused()
    expect(page.locator('#lua-highlight')).to_have_text(source + '\u200b')
    assert page.evaluate('readState().world') == state['world']
    assert page.evaluate('readState().mission') is None
    page.locator('#console-close').click()

    artifacts = Path(__file__).resolve().parents[1] / 'artifacts'
    artifacts.mkdir(exist_ok=True)
    for width, height in [(1360, 1000), (390, 844), (320, 568), (844, 390)]:
        page.set_viewport_size({'width': width, 'height': height})
        expect(opener).to_be_in_viewport()
        expect(page.locator('#console-toggle')).to_be_in_viewport()
        assert page.evaluate('document.body.scrollWidth <= innerWidth')
        opener.click()
        page.locator('[data-help-topic="quick"]').click()
        page.locator('[data-help-language="ja"]').click()
        scoring = page.locator('[data-help-group="scoring"]')
        if not scoring.evaluate('e => e.open'):
            scoring.locator('summary').click()
        expect(scoring).to_contain_text('敵の無力化は勝利点 2,000 点')
        expect(scoring).to_contain_text('敵の Δv 消費率 × 500 点')
        page.locator('[data-help-language="en"]').click()
        expect(scoring).to_have_attribute('open', '')
        expect(scoring).to_contain_text('Disabling the enemy earns 2,000 victory points')
        expect(scoring).to_contain_text('Every defeat scores zero')
        page.locator('[data-help-language="ja"]').click()
        scoring.locator('article').last.scroll_into_view_if_needed()
        assert body.evaluate('e => e.scrollWidth <= e.clientWidth')
        expect(page.locator('#help-close')).to_be_in_viewport()
        page.screenshot(path=str(artifacts / f'help-ja-{width}x{height}.png'))
        page.locator('[data-help-topic="lua"]').click()
        body.evaluate('e => { e.scrollTop = e.scrollHeight; }')
        assert body.evaluate('e => e.scrollTop > 0 && e.scrollWidth <= e.clientWidth')
        expect(page.locator('#help-close')).to_be_in_viewport()
        expect(page.locator('[data-help-language="en"]')).to_be_in_viewport()
        page.locator('#help-close').click()
        expect(dialog).to_be_hidden()

    page.set_viewport_size({'width': 1360, 'height': 1000})
    opener.click()
    page.mouse.click(3, 3)
    expect(dialog).to_be_hidden()
    expect(opener).to_be_focused()
    # Stored choice takes precedence over the browser's initial language.
    page.reload()
    expect(page.locator('body')).to_have_attribute('data-ready', 'true', timeout=90000)
    expect(dialog).to_have_attribute('lang', 'ja')
    expect(page.locator('html')).to_have_attribute('lang', 'en')
    connect_engine(page)
    print('PASS: help pauses Auto/Commit, keeps playback paused, traps/restores focus, preserves topic/details across languages, stores language, inserts Lua without execution, and fits desktop/mobile layouts')

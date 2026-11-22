"""Exercise diagnostic settings through the visible Lua console and shared engine."""

from playwright.sync_api import expect


def lua(page, source, expected=None, error=False):
    page.locator('#lua-input').fill(source)
    page.locator('#run').click()
    line = page.locator('#console-output p').last
    expect(line).to_have_class('error-line' if error else 'result')
    if expected is not None:
        expect(line).to_contain_text(expected)


def check_parameters(page):
    page.locator('#console-toggle').click()
    lua(page, 'game.parameters().reaction_seconds', '120')
    lua(page, 'game.configure("reaction_seconds", 60); return game.state().mission.run_kind', 'custom')
    lua(page, 'game.configure("reload_seconds", 60); return game.parameters().reload_seconds', '60')
    lua(page, 'game.configure("reload_seconds", 1.1)', 'multiple', error=True)
    lua(page, 'game.configure("unknown", 0)', 'Unknown parameter', error=True)
    lua(page, 'game.configure("reaction_seconds", 0/0)', 'finite', error=True)
    lua(page, 'game.fire(game.aim()); game.fire(game.aim()); game.command("play"); game.configure("reaction_seconds", 30)', 'Pause execution', error=True)
    lua(page, 'game.command("pause")')
    before = page.evaluate('readState()')
    lua(page, 'game.configure("reaction_seconds", 30); return game.state().mission.run_kind', 'debug')
    after = page.evaluate('readState()')
    assert before['world'] == after['world']
    assert before['command_log'] == after['command_log']
    assert before['mission']['queued'] == after['mission']['queued']
    lua(page, 'game.configure("reload_seconds", 30)', 'before the first command', error=True)
    lua(page, 'game.state().mission.parameter_edits[3].during_play', 'true')
    lua(page, 'game.command("retry"); return game.parameters().reaction_seconds', '60')
    lua(page, 'game.parameters().reload_seconds', '60')
    assert page.evaluate('readState().world.tick') == 0
    lua(page, 'game.command("mission"); return game.state().mission.run_kind', 'standard')
    page.locator('#console-toggle').click()
    print('PASS: visible Lua configuration, atomic errors, paused-only policy edits, retained orders/rounds, custom Retry baseline and standard reset')

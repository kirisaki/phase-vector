"""Exercise diagnostic settings through the visible Lua console and shared engine."""

from playwright.sync_api import expect
from pathlib import Path


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


def check_evaluation(page):
    artifacts = Path(__file__).resolve().parents[1] / 'artifacts'
    artifacts.mkdir(exist_ok=True)
    for width, height in [(1360, 1000), (390, 844), (844, 390)]:
        page.set_viewport_size({'width': width, 'height': height})
        page.locator('#console-toggle').click()
        lua(page, 'game.command("mission"); game.configure("deadline_seconds", 120); game.burn(10, 0); game.fire(game.aim()); game.fire(game.aim())')
        lua(page, 'game.state().mission.evaluation', 'nil')
        lua(page, 'game.command("step"); game.configure("reaction_seconds", 60)')
        page.locator('#console-toggle').click()
        expect(page.locator('#mission-status')).to_contain_text('Debug run')
        page.locator('#next-turn').click()
        expect(page.locator('#mission-result')).to_be_visible()
        expect(page.locator('#result-title')).to_have_text('VICTORY')
        expect(page.locator('#result-score')).to_have_text('1950')
        expect(page.locator('#score-victory')).to_have_text('1000')
        expect(page.locator('#score-shots')).to_have_text('460')
        expect(page.locator('#score-fuel')).to_have_text('490')
        expect(page.locator('#result-run-kind')).to_have_text('Debug run')
        expect(page.locator('#result-settings-note')).to_contain_text('diagnostic')
        expect(page.locator('#result-shots')).to_have_text('2')
        expect(page.locator('#result-dv')).to_have_text('1.2 km/s')
        expect(page.locator('#result-title')).to_be_in_viewport()
        expect(page.locator('#result-retry')).to_be_in_viewport()
        expect(page.locator('#result-view')).to_be_in_viewport()
        assert page.evaluate('document.documentElement.scrollWidth <= innerWidth')
        page.screenshot(path=str(artifacts / f'evaluation-{width}x{height}.png'))
        # The result remains inspectable after dismissal, and Lua uses the same score.
        page.locator('#result-view').click()
        expect(page.locator('#mission-metrics')).to_contain_text('1950 pts')
        page.locator('#console-toggle').click()
        lua(page, 'game.state().mission.evaluation.total', '1950')
        lua(page, 'game.configure("reaction_seconds", 90)', 'Mission is over', error=True)
        page.locator('#console-toggle').click()
        page.locator('#retry').click()
        expect(page.locator('#mission-result')).to_be_hidden()
        expect(page.locator('#mission-status')).to_contain_text('Custom settings')
        assert page.evaluate('readState().world.tick') == 0
        assert page.evaluate('readState().mission.evaluation') is None
        page.locator('#console-toggle').click()
        lua(page, 'game.parameters().reaction_seconds', '120')
        lua(page, 'game.parameters().deadline_seconds', '120')
        lua(page, 'game.command("mission")')
        page.locator('#console-toggle').click()
    page.set_viewport_size({'width': 1360, 'height': 1000})
    print('PASS: terminal score/breakdown, Lua agreement, debug/custom labels, retained inline score and Retry baseline on desktop/phone/landscape')

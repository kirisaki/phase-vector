"""Check lexical coloring and native editing behavior in the Lua console."""

from pathlib import Path
from playwright.sync_api import expect


def check_repl_highlight(page):
    fixtures = [
        ('local answer = 0x1.fp+2 + .5e-1 -- return 99\nreturn "雪 <tag>"',
         [('keyword', 'local'), ('number', '0x1.fp+2'), ('number', '.5e-1'),
          ('comment', '-- return 99'), ('keyword', 'return'), ('string', '"雪 <tag>"')]),
        ('--[==[comment\n]] still ]==]\nreturn [=[a ]==] b]=]',
         [('comment', '--[==[comment\n]] still ]==]'), ('keyword', 'return'), ('string', '[=[a ]==] b]=]')]),
        (r'''return 'it\'s -- text' .. "end"''',
         [('keyword', 'return'), ('string', r"'it\'s -- text'"), ('string', '"end"')]),
        ('--[=[unfinished\nreturn 1', [('comment', '--[=[unfinished\nreturn 1')]),
        ('local x = [==[unfinished', [('keyword', 'local'), ('string', '[==[unfinished')]),
        ('return "broken\nlocal n=2', [('keyword', 'return'), ('string', '"broken'), ('keyword', 'local'), ('number', '2')]),
        ('return "\\z \n hi"', [('keyword', 'return'), ('string', '"\\z \n hi"')]),
        ('return 1..2, foo123', [('keyword', 'return'), ('number', '1'), ('number', '2')]),
    ]
    for source, expected in fixtures:
        tokens = page.evaluate('''async source => {
            const {luaTokens} = await import(browserModule('lua-highlight'));
            return luaTokens(source);
        }''', source)
        assert ''.join(token['text'] for token in tokens) == source
        assert [(t['kind'], t['text']) for t in tokens if t['kind'] != 'plain'] == expected

    input_box = page.locator('#lua-input')
    mirror = page.locator('#lua-highlight')
    expect(mirror).to_have_attribute('aria-hidden', 'true')
    input_box.fill('return "<img src=x onerror=alert(1)>"')
    expect(mirror.locator('img')).to_have_count(0)
    expect(mirror.locator('.lua-string')).to_have_text('"<img src=x onerror=alert(1)>"')
    input_box.fill('')
    input_box.press_sequentially('return 42', delay=15)
    input_box.press('ControlOrMeta+z')
    assert input_box.input_value() != 'return 42'
    input_box.press('ControlOrMeta+Shift+z')
    expect(input_box).to_have_value('return 42')
    input_box.press('ControlOrMeta+a')
    assert input_box.evaluate('e => e.selectionEnd - e.selectionStart') == len('return 42')
    input_box.press('ArrowRight')
    input_box.press('ControlOrMeta+Enter')
    expect(page.locator('#console-output p').last).to_have_text('42')
    expect(page.locator('#console-output p').last).to_have_class('result')

    # Composition keeps native text/underlines and cannot submit via Ctrl+Enter.
    lines = page.locator('#console-output p').count()
    input_box.dispatch_event('compositionstart')
    assert input_box.evaluate('e => getComputedStyle(e).webkitTextFillColor') != 'rgba(0, 0, 0, 0)'
    input_box.press('ControlOrMeta+Enter')
    assert page.locator('#console-output p').count() == lines
    input_box.dispatch_event('compositionend')
    expect(input_box).to_have_value('return 42')

    # Programmatic example insertion must repaint without rewriting the input on each keystroke.
    page.locator('#console-panel summary').click()
    example = page.locator('[data-example]').filter(has_text='Store & print')
    example.click()
    assert mirror.text_content() == input_box.input_value() + '\u200b'
    page.locator('#console-panel summary').click()

    artifacts = Path(__file__).resolve().parents[1] / 'artifacts'
    source = '-- Lua console\nlocal message = "こんにちは"\nlocal n = 0x2a\n' + 'print(message, n)\n' * 10 + '-- ' + 'long_label_' * 25 + '\n'
    for width, height in [(1360, 1000), (390, 844), (844, 390)]:
        page.set_viewport_size({'width': width, 'height': height})
        input_box.fill(source)
        input_box.evaluate('e => { e.scrollTop = e.scrollHeight; }')
        page.wait_for_function('''() => {
            const input = document.querySelector('#lua-input'), mirror = document.querySelector('#lua-highlight');
            return input.scrollTop > 0 && Math.abs(input.scrollTop - mirror.scrollTop) < 1;
        }''')
        assert mirror.text_content() == input_box.input_value() + '\u200b'
        sizes = page.evaluate('''() => ['lua-input', 'lua-highlight'].map(id => {
            const e=document.getElementById(id), s=getComputedStyle(e);
            return [e.clientWidth, e.clientHeight, e.scrollHeight, s.font, s.padding, s.whiteSpace, s.overflowWrap];
        })''')
        assert sizes[0] == sizes[1], sizes
        expect(input_box).to_be_in_viewport()
        expect(page.locator('#run')).to_be_in_viewport()
        input_box.fill('-- Observations only\nlocal s = game.state()\nreturn "enemy", 42, true')
        artifacts.mkdir(exist_ok=True)
        page.screenshot(path=str(artifacts / f'lua-highlight-{width}x{height}.png'))

    page.emulate_media(forced_colors='active')
    expect(mirror).to_be_hidden()
    assert input_box.evaluate('e => getComputedStyle(e).webkitTextFillColor') != 'rgba(0, 0, 0, 0)'
    page.emulate_media(forced_colors='none')
    page.set_viewport_size({'width': 1360, 'height': 1000})
    input_box.fill('return 1 + 2')
    print('PASS: Lua tokens/long brackets/incomplete input, literal HTML, native undo/redo/selection, shortcut, composition guard, example insertion, scroll/wrap alignment, mobile layouts and forced colors')

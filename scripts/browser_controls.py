"""Shared browser input geometry; gameplay expectations stay in the smoke tests."""


def connect_engine(page):
    # Development serves TypeScript; the release artifact contains plain ES modules.
    page.evaluate(r"""async () => {
        const entry = [...document.querySelectorAll('script[type="module"][src]')]
            .find(script => /\/app\.(ts|js)$/.test(new URL(script.src).pathname)).src;
        const extension = new URL(entry).pathname.endsWith('.ts') ? '.ts' : '.js';
        window.browserModule = name => new URL(`./${name}${extension}`, location.href).href;
        const source = await (await fetch(entry)).text();
        const binding = source.match(/from ['"]([^'"]*pkg\/phase_vector\.js[^'"]*)['"]/)[1];
        window.wasm = await import(new URL(binding, entry).href);
        window.readState = window.readPhaseVectorState = () => JSON.parse(wasm.state_json());
    }""")


def read_ui_settings(page):
    return page.evaluate("async () => ({ ...await import(browserModule('ui-settings')) })")


def timeline_point(page, fraction, offset_pixels=0):
    slider = page.locator('#forecast-time')
    box = slider.bounding_box()
    thumb = slider.evaluate("e => parseFloat(getComputedStyle(e).getPropertyValue('--timeline-thumb-size'))")
    return {'x': box['x'] + thumb / 2 + (box['width'] - thumb) * fraction + offset_pixels,
            'y': box['y'] + box['height'] / 2}

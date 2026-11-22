"""Shared browser input geometry; gameplay expectations stay in the smoke tests."""


def read_ui_settings(page):
    return page.evaluate("async () => ({ ...await import(new URL('./ui-settings.js', location.href).href) })")


def timeline_point(page, fraction, offset_pixels=0):
    slider = page.locator('#forecast-time')
    box = slider.bounding_box()
    thumb = slider.evaluate("e => parseFloat(getComputedStyle(e).getPropertyValue('--timeline-thumb-size'))")
    return {'x': box['x'] + thumb / 2 + (box['width'] - thumb) * fraction + offset_pixels,
            'y': box['y'] + box['height'] / 2}

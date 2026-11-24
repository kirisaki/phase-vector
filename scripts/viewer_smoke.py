#!/usr/bin/env python3
"""Exercise the private viewer using test data and locally simulated Access."""
import subprocess
from pathlib import Path
from playwright.sync_api import sync_playwright, expect

ROOT = Path(__file__).resolve().parents[1]


def main():
    artifacts = ROOT / 'artifacts'
    artifacts.mkdir(exist_ok=True)
    server = subprocess.Popen(['node', 'backend/test/viewer-server.mjs'], cwd=ROOT,
                              stdout=subprocess.PIPE, text=True)
    try:
        ready = server.stdout.readline().strip()
        assert ready.startswith('READY '), ready
        url = ready.split(' ', 1)[1]
        with sync_playwright() as p:
            browser = p.chromium.launch(headless=True, args=['--no-sandbox'])
            for width, height in [(1360, 900), (390, 844), (320, 640)]:
                page = browser.new_page(viewport={'width': width, 'height': height},
                                        has_touch=width < 700)
                errors = []
                page.on('pageerror', lambda e: errors.append(str(e)))
                page.on('dialog', lambda d: (_ for _ in ()).throw(AssertionError(d.message)))
                page.goto(url)
                expect(page.locator('.report')).to_have_count(25)
                if width < 700:
                    expect(page.locator('article')).not_to_be_visible()
                page.locator('.report').first.click()
                expect(page.locator('article')).to_be_visible()
                expect(page.locator('.message')).to_contain_text('<script>alert("private")</script>')
                image = page.locator('article img')
                expect(image).to_be_visible()
                assert image.evaluate('e => e.complete && e.naturalWidth > 0')
                assert page.evaluate('document.documentElement.scrollWidth <= innerWidth')
                page.screenshot(path=str(artifacts / f'viewer-detail-{width}.png'), full_page=False)
                if width < 700:
                    expect(page.locator('.list')).not_to_be_visible()
                    page.get_by_role('link', name='← 一覧に戻る').click()
                    expect(page.locator('.list')).to_be_visible()
                page.get_by_label('カテゴリ', exact=True).select_option('balance')
                page.get_by_label('期間', exact=True).select_option('7')
                page.get_by_label('バージョン（完全一致）').fill('test-commit')
                page.get_by_role('button', name='絞り込む').click()
                expect(page.locator('.report')).to_have_count(13)
                assert page.evaluate('document.documentElement.scrollWidth <= innerWidth')
                page.screenshot(path=str(artifacts / f'viewer-list-{width}.png'), full_page=False)
                page.get_by_label('バージョン（完全一致）').fill('unknown')
                page.get_by_role('button', name='絞り込む').click()
                expect(page.get_by_text('該当する投稿はありません。')).to_be_visible()
                page.get_by_role('link', name='解除・最新へ').click()
                page.get_by_role('link', name='以前の 25 件 →').click()
                expect(page.locator('.report')).to_have_count(2)
                page.get_by_role('link', name='最新へ', exact=True).click()
                expect(page.locator('.report')).to_have_count(25)
                assert not errors, errors
                page.close()
            browser.close()
        print('Viewer browser checks passed: desktop, 390px and 320px mobile, filters, navigation, images, escaped text.')
    finally:
        server.terminate()
        server.wait(timeout=15)


if __name__ == '__main__':
    main()

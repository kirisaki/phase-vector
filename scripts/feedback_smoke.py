#!/usr/bin/env python3
"""Exercise feedback in Chromium against the local D1/R2 Worker; no Cloudflare account."""
import base64
import io
import json
import subprocess
from pathlib import Path
from urllib.request import urlopen
from playwright.sync_api import sync_playwright, expect
from PIL import Image, ImageChops, ImageStat
from browser_site import RELEASE_URL, route_release_site
from browser_controls import connect_engine

ROOT=Path(__file__).resolve().parents[1]
ARTIFACTS=ROOT/'artifacts'
TURNSTILE='''window.turnstile={render:(node,options)=>{window.feedbackChallenge=options;setTimeout(()=>options.callback('browser:'+crypto.randomUUID()),0);return 'test-widget'},remove:()=>{},reset:()=>{}};'''

def main():
    ARTIFACTS.mkdir(exist_ok=True)
    server=subprocess.Popen(['node','backend/test/browser-server.mjs'],cwd=ROOT,stdout=subprocess.PIPE,text=True)
    try:
        ready=server.stdout.readline().strip()
        assert ready.startswith('READY '),ready
        api=ready.split(' ',1)[1]
        def rows():return json.load(urlopen(api+'/__test/submissions'))
        with sync_playwright() as p:
            browser=p.chromium.launch(headless=True,args=['--no-sandbox','--use-angle=swiftshader','--enable-unsafe-swiftshader'])
            for width,height in [(1360,1000),(390,844)]:
                page=browser.new_page(viewport={'width':width,'height':height},has_touch=True)
                errors=[];page.on('pageerror',lambda e:errors.append(str(e)))
                route_release_site(page)
                page.route('**/runtime-config.json',lambda r:r.fulfill(json={'version':'feedback-browser-test','feedbackApiUrl':'https://feedback.test','turnstileSiteKey':'test-public-key'}))
                page.route('https://challenges.cloudflare.com/**',lambda r:r.fulfill(content_type='application/javascript',body=TURNSTILE))
                requests=[]
                def proxy(route):
                    requests.append(route.request.post_data_buffer)
                    response=route.fetch(url=api+route.request.url.split('feedback.test',1)[1])
                    route.fulfill(response=response)
                page.route('https://feedback.test/**',proxy)
                page.goto(RELEASE_URL)
                expect(page.locator('body')).to_have_attribute('data-ready','true',timeout=90000)
                connect_engine(page)
                page.wait_for_timeout(400)
                before=page.evaluate('readState()')
                reference=Image.open(io.BytesIO(page.screenshot())).convert('RGB')
                reference.save(ARTIFACTS/f'feedback-reference-{width}.png')
                page.locator('#feedback-open').click()
                expect(page.locator('#feedback-dialog')).to_be_visible(timeout=30000)
                expect(page.locator('#feedback-preview')).to_be_visible()
                expect(page.locator('#feedback-attach')).not_to_be_checked()
                expect(page.locator('#feedback-send')).to_be_disabled()
                assert requests==[]
                encoded=page.locator('#feedback-preview').evaluate('''async img=>{
                  const b=await(await fetch(img.src)).blob();return await new Promise(r=>{const f=new FileReader();f.onload=()=>r(f.result.split(',')[1]);f.readAsDataURL(b)})}''')
                png=base64.b64decode(encoded);capture=Image.open(io.BytesIO(png)).convert('RGB')
                capture.save(ARTIFACTS/f'feedback-capture-{width}.png')
                assert capture.size==(width,height)
                # Compare the full DOM + WebGL + SVG image with the actual browser frame.
                difference=ImageStat.Stat(ImageChops.difference(reference,capture)).mean
                assert max(difference)<18,difference
                box=page.locator('#tactical-map').bounding_box()
                # Exclude the canvas focus outline, which disappears when Feedback is clicked.
                crop=(int(box['x'])+4,int(box['y'])+4,int(box['x']+box['width'])-4,int(box['y']+box['height'])-4)
                map_pixels=capture.crop(crop)
                assert max(ImageStat.Stat(map_pixels).stddev)>10,'Missing tactical map content'
                reference_map=reference.crop(crop)
                for color in ['cyan','amber']:
                    def count(image):
                        return sum((g>110 and b>110 and r<160) if color=='cyan' else (r>160 and 70<g<220 and b<170) for r,g,b in image.getdata())
                    assert count(map_pixels)>=count(reference_map)*0.65,(width,crop,color,count(map_pixels),count(reference_map))
                assert page.evaluate('readState().world.tick')==before['world']['tick']
                page.locator('#feedback-fields summary').click()
                assert page.locator('#feedback-fields details').evaluate('e => e.scrollHeight <= e.clientHeight + 1'), 'Metadata must not be clipped'
                metadata=json.loads(page.locator('#feedback-metadata').inner_text())
                assert set(metadata['metadata'])=={'version','outcome','runKind','simulationSeconds','forecastSeconds','score','shotsFired','ownDeltaVSpent','viewport'}
                page.locator('#feedback-rating').select_option('4')
                page.locator('#feedback-category').select_option('balance')
                page.locator('#feedback-message').fill(f'Desktop/mobile feedback {width}')
                page.locator('#feedback-attach').check()
                page.locator('#feedback-consent').check()
                page.locator('#feedback-verify').click()
                expect(page.locator('#feedback-send')).to_be_enabled()
                # Simulate losing the response after the Worker already stored the report.
                urlopen(api+'/__test/lose-next').close()
                page.locator('#feedback-send').click()
                expect(page.locator('#feedback-status')).to_contain_text('Submission failed')
                expect(page.locator('#feedback-message')).to_have_value(f'Desktop/mobile feedback {width}')
                page.locator('#feedback-verify').click()
                expect(page.locator('#feedback-send')).to_be_enabled()
                page.locator('#feedback-send').click()
                expect(page.locator('#feedback-status')).to_contain_text('received privately')
                matching=[r for r in rows() if json.loads(r['payload_json']).get('message')==f'Desktop/mobile feedback {width}']
                assert len(matching)==1
                stored=urlopen(api+'/__test/image/'+matching[0]['id']).read();assert stored==png
                page.locator('#feedback-close').click()
                # Console contents must be absent from both image and metadata.
                page.locator('#console-toggle').click()
                page.locator('#lua-input').fill('PRIVATE_LUA_CANARY')
                page.locator('#console-panel').evaluate("e=>{const c=document.createElement('div');c.style.cssText='height:60px;background:rgb(255,0,255)';c.textContent='PRIVATE_LUA_CANARY';e.prepend(c)}")
                page.locator('#feedback-open').click()
                expect(page.locator('#feedback-dialog')).to_be_visible(timeout=30000)
                expect(page.locator('#feedback-preview')).to_be_visible()
                assert 'PRIVATE_LUA_CANARY' not in page.locator('#feedback-metadata').inner_text()
                redacted=page.locator('#feedback-preview').evaluate('''img=>{const c=document.createElement('canvas');c.width=img.naturalWidth;c.height=img.naturalHeight;const x=c.getContext('2d');x.drawImage(img,0,0);return [...x.getImageData(0,0,c.width,c.height).data].filter((v,i,a)=>i%4===0&&v>240&&a[i+1]<10&&a[i+2]>240).length}''')
                assert redacted==0
                page.locator('#feedback-remove-image').click()
                expect(page.locator('#feedback-preview')).to_be_hidden()
                page.locator('#feedback-rating').select_option('3');page.locator('#feedback-message').fill('Text-only report')
                page.locator('#feedback-consent').check();page.locator('#feedback-verify').click()
                expect(page.locator('#feedback-send')).to_be_enabled();page.locator('#feedback-send').click()
                expect(page.locator('#feedback-status')).to_contain_text('received privately')
                assert all(r['image_key'] is None for r in rows() if json.loads(r['payload_json']).get('message')=='Text-only report')
                page.screenshot(path=str(ARTIFACTS/f'feedback-dialog-{width}.png'))
                assert page.evaluate('document.documentElement.scrollWidth <= innerWidth')
                assert not errors,errors
                page.close()
            # A capture error must still allow an explicit text-only submission.
            page=browser.new_page(viewport={'width':390,'height':844})
            route_release_site(page)
            page.route('**/runtime-config.json',lambda r:r.fulfill(json={'version':'failure-test','feedbackApiUrl':'https://feedback.test','turnstileSiteKey':'test-public-key'}))
            page.route('**/feedback-capture.js',lambda r:r.fulfill(content_type='application/javascript',body='export async function captureApplication(){throw new Error("Test capture failure")}'))
            page.route('https://challenges.cloudflare.com/**',lambda r:r.fulfill(content_type='application/javascript',body=TURNSTILE))
            page.route('https://feedback.test/**',lambda r:r.fulfill(response=r.fetch(url=api+r.request.url.split('feedback.test',1)[1])))
            page.goto(RELEASE_URL)
            expect(page.locator('body')).to_have_attribute('data-ready','true',timeout=90000)
            page.locator('#feedback-open').click()
            expect(page.locator('#feedback-status')).to_contain_text('Screenshot unavailable')
            expect(page.locator('#feedback-attach')).to_be_disabled()
            page.locator('#feedback-rating').select_option('2');page.locator('#feedback-message').fill('Capture failed but text works')
            page.locator('#feedback-consent').check();page.locator('#feedback-verify').click()
            expect(page.locator('#feedback-send')).to_be_enabled()
            page.evaluate("window.feedbackChallenge['expired-callback']()")
            expect(page.locator('#feedback-send')).to_be_disabled()
            page.locator('#feedback-verify').click();expect(page.locator('#feedback-send')).to_be_enabled()
            page.locator('#feedback-send').click();expect(page.locator('#feedback-status')).to_contain_text('received privately')
            assert any(json.loads(r['payload_json']).get('message')=='Capture failed but text works' and r['image_key'] is None for r in rows())
            page.close()
            # An unconfigured deployment still runs and never contacts Turnstile/API.
            page=browser.new_page(viewport={'width':320,'height':568});route_release_site(page)
            page.route('**/runtime-config.json',lambda r:r.fulfill(json={'version':'offline-test','feedbackApiUrl':'','turnstileSiteKey':''}))
            page.goto(RELEASE_URL)
            expect(page.locator('body')).to_have_attribute('data-ready','true',timeout=90000)
            page.locator('#feedback-open').click();expect(page.locator('#feedback-status')).to_contain_text('not configured')
            expect(page.locator('#feedback-send')).to_be_disabled();expect(page.locator('#feedback-verify')).to_be_disabled()
            page.locator('#feedback-close').click();page.locator('#step').click()
            expect(page.locator('#time')).not_to_have_text('0.00')
            assert page.evaluate('document.documentElement.scrollWidth <= innerWidth')
            connect_engine(page)
            page.evaluate("() => { for(let i=0;i<10;i++) wasm.command('next_turn'); }")
            expect(page.locator('#mission-result')).to_be_visible()
            page.locator('#result-feedback').click()
            expect(page.locator('#feedback-dialog')).to_be_visible(timeout=30000)
            expect(page.locator('#feedback-preview')).to_be_visible()
            page.locator('#feedback-fields summary').click()
            result_metadata=json.loads(page.locator('#feedback-metadata').inner_text())['metadata']
            assert result_metadata['outcome']=='breakthrough' and result_metadata['score']==0
            page.locator('#feedback-close').click()
            expect(page.locator('#result-feedback')).to_be_focused()
            page.close()
            browser.close()
        print('PASS: full DOM/WebGL/SVG screenshots, private opt-in upload, metadata review/redaction, D1/R2 persistence, lost-response deduplication, text-only reports, desktop/mobile layouts')
    finally:
        server.terminate();server.wait(timeout=15)

if __name__=='__main__':main()

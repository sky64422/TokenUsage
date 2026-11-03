"""Check isolated Windows WebView geometry, theme transitions and real frame cadence."""
import argparse
import json
import pyautogui
from pathlib import Path
from playwright.sync_api import sync_playwright, expect
from notch_test_support import prepare_hover_fixture, reset_notch

ROOT = Path(__file__).resolve().parent.parent
SAMPLE_FRAMES = 45
POINTER_MOVE_SECONDS = .15

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--port', type=int, default=9223)
args = parser.parse_args()

with sync_playwright() as p:
    browser = p.chromium.connect_over_cdp(f'http://127.0.0.1:{args.port}')
    page = browser.contexts[0].pages[0]
    # This check must precede every mutation, including fixture setup.
    assert page.evaluate('async()=>window.__TAURI__.app.getIdentifier()') == 'com.tokenusage.notch-preview'
    errors = []
    page.on('pageerror', lambda error: errors.append(str(error)))
    prepare_hover_fixture(page)
    results = []
    original_pointer = pyautogui.position()
    try:
        for theme in ['dark', 'glass', 'liquid']:
            reset_notch(page)
            page.locator('.notch').dispatch_event('contextmenu')
            expect(page.locator('.detail-settings')).to_be_visible()
            page.locator('#tab-btn-appearance').dispatch_event('click')
            button = page.locator(f'[data-theme-option="{theme}"]')
            if button.get_attribute('aria-pressed') != 'true':
                button.click()
                expect(button).to_have_attribute('aria-pressed', 'true')
                expect(button).to_be_enabled()
            assert page.evaluate('async()=> (await window.__TAURI__.core.invoke("get_state")).settings.theme') == theme
            heights = []
            for tab in ['appearance', 'models', 'general']:
                page.locator('#tab-btn-' + tab).click()
                geometry = page.locator('.notch-detail').evaluate('''e=>{
                  const r=e.getBoundingClientRect(); return {height:r.height,left:r.left,top:r.top,
                    right:r.right,bottom:r.bottom,viewportWidth:innerWidth,viewportHeight:innerHeight,
                    scrollWidth:e.scrollWidth,clientWidth:e.clientWidth};
                }''')
                heights.append(geometry['height'])
                assert geometry['left'] >= 0 and geometry['top'] >= 0, geometry
                assert geometry['right'] <= geometry['viewportWidth'] and geometry['bottom'] <= geometry['viewportHeight'], geometry
                assert geometry['scrollWidth'] <= geometry['clientWidth'], geometry
            assert max(heights) - min(heights) <= 1, {'theme': theme, 'settings_heights': heights}
            page.locator('#btn-close-settings').dispatch_event('click')
            reset_notch(page)
            # A synthetic pointerenter leaves the OS cursor over the disappearing
            # settings panel; its later pointerleave legitimately closes hover.
            target = page.evaluate('''async()=>{
              const p=await window.__TAURI__.window.getCurrentWindow().innerPosition();
              const r=document.querySelector('.notch-cell:not([hidden])').getBoundingClientRect();
              return {x:p.x+(r.x+r.width/2)*devicePixelRatio,y:p.y+(r.y+r.height/2)*devicePixelRatio};
            }''')
            # Two instantaneous jumps can be coalesced while the native window is
            # settling. Exercise a real crossing before asserting hover stability.
            pyautogui.moveTo(target['x'] - 100, target['y'], duration=POINTER_MOVE_SECONDS)
            pyautogui.moveTo(target['x'], target['y'], duration=POINTER_MOVE_SECONDS)
            expect(page.locator('.notch-detail')).to_be_visible()
            sample = page.evaluate('''async count=>{
              const rail=document.querySelector('.notch');
              const frames=[]; const rect=()=>{const r=rail.getBoundingClientRect();return [screenX,screenY,innerWidth,innerHeight,r.x,r.y,r.width,r.height]};
              for(let i=0;i<count;i++) await new Promise(resolve=>requestAnimationFrame(t=>{
                const d=document.querySelector('.notch-detail');
                frames.push({t,rect:rect(),visible:!d.hidden,transform:getComputedStyle(d).transform});resolve();
              }));
              return frames;
            }''', SAMPLE_FRAMES)
            expect(page.locator('.notch-detail')).to_be_visible()
            assert all(frame['visible'] for frame in sample), theme
            baseline = sample[0]['rect']
            assert all(all(abs(a-b) <= .1 for a,b in zip(frame['rect'], baseline)) for frame in sample), theme
            deltas = [sample[i]['t'] - sample[i-1]['t'] for i in range(1, len(sample))]
            results.append({'theme': theme, 'settings_heights_dip': heights, 'frame_count': len(sample),
                            'frame_interval_ms': {'mean': sum(deltas)/len(deltas), 'max': max(deltas)},
                            'observed_transforms': len({frame['transform'] for frame in sample}),
                            'window_and_rail_stable': True})
        # Exercise actual UI persistence in both directions; debug autostart can touch OS state.
        reset_notch(page)
        page.locator('.notch').dispatch_event('contextmenu')
        page.locator('#tab-btn-appearance').dispatch_event('click')
        for toggle, field in [('always-show','always_show_notch'), ('hover-detail','hover_detail'), ('show-animation','show_orbit')]:
            control = page.locator('#' + toggle)
            initial = control.is_checked()
            for value in [not initial, initial]:
                page.locator(f'label[for="{toggle}"]').click()
                expect(control).to_be_enabled()
                assert control.is_checked() == value
                assert page.evaluate('async field => (await window.__TAURI__.core.invoke("get_state")).settings[field]', field) == value
                if value != initial:
                    page.reload()
                    page.wait_for_selector('.notch-cell')
                    page.locator('.notch').dispatch_event('contextmenu')
                    expect(page.locator('.detail-settings')).to_be_visible()
                    page.locator('#tab-btn-appearance').dispatch_event('click')
                    assert control.is_checked() == value, {'toggle': toggle, 'after_reload': value}
        assert not errors, errors
        output = ROOT / 'tmp' / 'native-quality-results.json'
        output.write_text(json.dumps({'themes': results, 'page_errors': errors,
                                     'limitations': 'WebView requestAnimationFrame intervals, not presented FPS; no CPU or long-duration leak measurement.'}, indent=2), encoding='utf-8')
        print(json.dumps({'themes': results, 'report': str(output)}, ensure_ascii=False))
    finally:
        pyautogui.moveTo(original_pointer.x, original_pointer.y)
        page.evaluate('()=>clearInterval(window.hoverFixtureTimer)')

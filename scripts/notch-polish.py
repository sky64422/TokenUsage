"""Native auto-hide/activity smoke. Isolated preview only; synthetic activity."""
from notch_test_support import prepare_preview, reset_notch
import ctypes
import json
from ctypes import wintypes
from pathlib import Path

import pyautogui
from PIL import ImageGrab
from playwright.sync_api import sync_playwright, expect

u = ctypes.windll.user32
u.WindowFromPoint.argtypes = [wintypes.POINT]
u.WindowFromPoint.restype = wintypes.HWND
u.GetAncestor.argtypes = [wintypes.HWND, wintypes.UINT]
u.GetAncestor.restype = wintypes.HWND

with sync_playwright() as p:
    browser = p.chromium.connect_over_cdp('http://127.0.0.1:9223')
    page = browser.contexts[0].pages[0]
    assert page.evaluate('async()=>window.__TAURI__.app.getIdentifier()') == 'com.tokenusage.notch-preview'
    prepare_preview(page, always_show=False)
    page.set_default_timeout(8000)
    errors = []
    page.on('pageerror', lambda e: errors.append(str(e)))

    def invoke(command, args=None):
        return page.evaluate('async([c,a])=>window.__TAURI__.core.invoke(c,a)', [command, args or {}])

    def folded(value):
        page.wait_for_function('(v)=>document.querySelector(".notch").classList.contains("is-folded")===v', arg=value)
        page.wait_for_function('()=>document.querySelector(".notch-surface").getAnimations().length===0')

    def away():
        pyautogui.moveTo(1000, 500, duration=.15)

    def hover_rest(layout):
        n = layout['notch']; edge = layout['edge']
        x, y = n['x'] + n['width']/2, n['y'] + n['height']/2
        if edge == 'right': x = n['x'] + n['width'] - 3
        if edge == 'left': x = n['x'] + 3
        if edge == 'top': y = n['y'] + 3
        if edge == 'bottom': y = n['y'] + n['height'] - 3
        pyautogui.moveTo(x, y, duration=.15)
        folded(False)

    def shot(layout, label):
        n = layout['notch']
        box = tuple(map(int, (n['x']-20, n['y']-20, n['x']+n['width']+20, n['y']+n['height']+20)))
        ImageGrab.grab(bbox=box, all_screens=True).save(f'tmp/polish-{label}.png')

    assert all(s['state'] == 'unknown' for s in invoke('get_provider_activity'))
    for edge in ['right', 'left', 'top', 'bottom']:
        reset_notch(page)
        page.evaluate('()=>document.activeElement?.blur()')
        invoke('set_notch_focus', {'focused': False})
        away()
        layout = invoke('set_notch_placement', {'placement': {'edge': edge, 'offset': .35, 'monitor_hint': None}})
        page.wait_for_function('(e)=>document.querySelector(".notch").dataset.edge===e', arg=edge)
        original = page.evaluate('()=>[outerWidth,outerHeight,screenX,screenY]')
        for cycle in range(3):
            away(); folded(True)
            assert page.locator('.notch-cells').evaluate('(e)=>e.inert')
            if cycle == 0: shot(layout, f'{edge}-folded')
            hover_rest(layout)
            assert page.evaluate('()=>[outerWidth,outerHeight,screenX,screenY]') == original
        shot(layout, f'{edge}-open')
        print('context', edge, flush=True)
        pyautogui.click(button='right')
        expect(page.locator('.detail-settings')).to_be_visible()
        away()
        assert invoke('get_notch_reveal') is True
        expect(page.locator('.notch')).not_to_have_class('notch is-folded')
        reset_notch(page)
        folded(True)

    page.keyboard.press('Shift+F10')
    expect(page.locator('.detail-settings')).to_be_hidden()
    hover_rest(layout)
    page.evaluate('async()=>window.__TAURI__.event.emit("provider-activity",[{provider_id:"codex",state:"running",observed_at:null},{provider_id:"claude",state:"recent",observed_at:null}])')
    assert page.locator('[data-id=codex] .activity-orbit').is_visible()
    expect(page.locator('[data-id=codex]')).to_have_attribute('data-activity', 'running')
    assert 'drop-shadow' in page.locator('[data-id=codex] .ring-wrap img').evaluate('(e)=>getComputedStyle(e).filter')
    expect(page.locator('[data-id=claude]')).to_have_attribute('data-activity', 'recent')
    page.emulate_media(reduced_motion='reduce')
    assert page.locator('[data-id=codex] .activity-orbit').evaluate('el => getComputedStyle(el).animationName') == 'none'
    page.emulate_media(reduced_motion='no-preference')
    page.screenshot(path='tmp/polish-activity.png', omit_background=True)
    page.evaluate('async()=>window.__TAURI__.event.emit("provider-activity",[{provider_id:"codex",state:"idle",observed_at:null}])')
    expect(page.locator('[data-id=codex]')).to_have_attribute('data-activity', 'idle')
    assert not errors, errors
    print(json.dumps({'four_edges': True, 'hover_cycles': 12, 'stable_native_geometry': True, 'settings_hold': True, 'keyboard_shortcuts_inactive': True, 'activity_and_reduced_motion': True, 'page_errors': errors}))

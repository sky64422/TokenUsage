"""Native edge-transition regression; requires isolated WebView2 preview (windows-dev.md)."""
import json
from pathlib import Path
import pyautogui
from PIL import ImageGrab
from playwright.sync_api import sync_playwright, expect

with sync_playwright() as p:
    browser=p.chromium.connect_over_cdp('http://127.0.0.1:9223')
    page=browser.contexts[0].pages[0]
    assert page.evaluate('async()=>window.__TAURI__.app.getIdentifier()')=='com.tokenusage.notch-preview'
    def invoke(c,a={}):return page.evaluate('async([c,a])=>window.__TAURI__.core.invoke(c,a)',[c,a])
    page.evaluate('async()=>{window.dragEvents=[]; document.addEventListener("gotpointercapture",e=>window.lastCapture={element:e.target,id:e.pointerId}); await window.__TAURI__.event.listen("notch-drag", e=>window.dragEvents.push(e.payload));}')
    def reset():
        page.locator('.detail-close').evaluate('(e)=>e.click()')
        page.evaluate('()=>document.activeElement?.blur()')
        pyautogui.moveTo(1000,600,duration=.1)
        return invoke('set_notch_placement',{'placement':{'edge':'right','offset':.5,'monitor_hint':None}})
    def start(l):
        n=l['notch'];scale=l['scale']
        x=n['x']+36*scale;y=n['y']+(l['metrics']['inset']+l['metrics']['cell']/2)*scale
        pyautogui.moveTo(x,y,duration=.2)
        page.evaluate('()=>window.dragEvents=[]')
        pyautogui.mouseDown()
        # Round-trip ensures begin was delivered before the deliberate gesture.
        page.wait_for_function('()=>document.querySelector(".notch-cell:hover")!==null')
    def edge(e):page.wait_for_function('(e)=>document.querySelector(".notch").dataset.edge===e',arg=e)
    def capture(name):
        for cell in page.locator('.notch-cell:visible').all():
            r=cell.bounding_box();rail=page.locator('.notch').bounding_box()
            assert r['y']>=rail['y'] and r['y']+r['height']<=rail['y']+rail['height']+.5
            assert r['x']>=rail['x'] and r['x']+r['width']<=rail['x']+rail['width']+.5
        page.screenshot(path=f'tmp/drag-{name}.png',omit_background=True)
    def finished():page.wait_for_function('()=>window.dragEvents.some(e=>e.finished)')
    l=reset()
    before=invoke('get_state')['settings']['notch']
    page.locator('.notch').focus()
    for key in ['ArrowUp','ArrowDown','ArrowLeft','ArrowRight','Shift+ArrowUp','Shift+ArrowDown','Shift+ArrowLeft','Shift+ArrowRight']:page.keyboard.press(key)
    assert invoke('get_state')['settings']['notch']==before
    start(l)
    try:
        pyautogui.moveTo(2440,8,duration=.6);edge('top');capture('top')
        assert invoke('get_state')['settings']['notch']['edge']=='right'
        pyautogui.moveTo(2558,2,duration=.2);edge('top')
        pyautogui.moveTo(8,300,duration=.8);edge('left');capture('left')
        pyautogui.moveTo(1000,1438,duration=.8);edge('bottom');capture('bottom')
        position=page.evaluate('async()=>window.__TAURI__.window.getCurrentWindow().outerPosition()')
        size=page.evaluate('async()=>window.__TAURI__.window.getCurrentWindow().outerSize()')
        assert position['y']+size['height']==1440
        ImageGrab.grab(bbox=(int(position['x'])-12,int(position['y'])-16,int(position['x']+size['width'])+12,1440),all_screens=True).save('tmp/drag-bottom-desktop.png')
        pyautogui.moveTo(2558,600,duration=.8);edge('right');capture('right')
        pyautogui.moveTo(2400,8,duration=.6);edge('top')
        # Release away from the relocated notch, through the desktop interior.
        pyautogui.moveTo(1200,700,duration=.4)
    finally:pyautogui.mouseUp()
    finished();assert invoke('get_state')['settings']['notch']['edge']=='top'
    expect(page.locator('[data-id=claude]')).to_have_attribute('aria-pressed','false')
    # Escape cancellation after transition, with mouse still down.
    l=reset();before=invoke('get_state')['settings']['notch'];start(l)
    try:
        pyautogui.moveTo(2400,8,duration=.5);edge('top')
        page.keyboard.press('ArrowRight')
        assert invoke('get_state')['settings']['notch']==before, 'keyboard persisted during drag'
        pyautogui.keyDown('esc')
        try:
            finished()
            page.wait_for_function('()=>!window.lastCapture.element.hasPointerCapture(window.lastCapture.id)')
        finally:pyautogui.keyUp('esc')
    finally:pyautogui.mouseUp()
    assert invoke('get_state')['settings']['notch']==before;edge('right')
    # A stale completion must not affect a later session.
    old_id=page.evaluate('()=>window.dragEvents.at(-1).id')
    l=reset();start(l)
    try:
        pyautogui.moveTo(2400,8,duration=.5);edge('top')
        invoke('finish_notch_drag',{'id':old_id,'cancel':True});edge('top')
    finally:pyautogui.mouseUp()
    finished();assert invoke('get_state')['settings']['notch']['edge']=='top'
    # Move into the real secondary display (negative coordinates).
    l=reset();start(l)
    try:
        pyautogui.moveTo(-40,600,duration=1.)
        page.wait_for_function('async()=> (await window.__TAURI__.window.getCurrentWindow().outerPosition()).x < 0')
        capture('secondary')
    finally:pyautogui.mouseUp()
    finished();assert invoke('get_state')['settings']['notch']['monitor_hint']!=l['monitor']
    # Crossing a shared seam while following the top edge must switch displays.
    l=reset();start(l)
    try:
        pyautogui.moveTo(2400,8,duration=.5);edge('top')
        pyautogui.moveTo(-100,8,duration=1.)
        page.wait_for_function('async()=> (await window.__TAURI__.window.getCurrentWindow().outerPosition()).x < 0')
        edge('top')
    finally:pyautogui.mouseUp()
    finished()
    # A real drag that cannot be persisted restores the starting placement and capture.
    l=reset();before=invoke('get_state')['settings']['notch']
    data=Path(page.evaluate('async()=>window.__TAURI__.path.appDataDir()'))
    state=data/'token-usage-state.json';backup=data/'token-usage-state.edge-backup.json'
    assert not backup.exists()
    state.rename(backup);state.mkdir()
    try:
        start(l)
        try:pyautogui.moveTo(2400,8,duration=.5);edge('top')
        finally:pyautogui.mouseUp()
        finished()
        assert invoke('get_state')['settings']['notch']==before
        expect(page.locator('.notch-error')).to_be_visible()
        page.wait_for_function('()=>!window.lastCapture.element.hasPointerCapture(window.lastCapture.id)')
    finally:state.rmdir();backup.rename(state)
    reset()
    print(json.dumps({'cancel_releases_capture':True,'no_keyboard_save_mid_drag':True,'outer_edge_monitor_crossing':True,'failed_drag_save_rollback':True,'right_top_left_right':True,'corner_retention':True,'bottom_overlays_taskbar':True,'release_outside':True,'escape_rollback':True,'stale_end_ignored':True,'negative_monitor_drag':True}))

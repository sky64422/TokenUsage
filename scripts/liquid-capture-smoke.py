"""Native screenshot regression. Requires the isolated preview on CDP 9223."""
import ctypes
import json
from ctypes import wintypes
from pathlib import Path
from PIL import ImageGrab
from playwright.sync_api import sync_playwright

u = ctypes.windll.user32
u.GetWindowDisplayAffinity.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.DWORD)]
u.GetWindowRect.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.RECT)]
Path('tmp').mkdir(exist_ok=True)
with sync_playwright() as p:
    browser = p.chromium.connect_over_cdp('http://127.0.0.1:9223')
    page = browser.contexts[0].pages[0]
    assert page.evaluate('async()=>await window.__TAURI__.app.getIdentifier()') == 'com.tokenusage.notch-preview'
    cell = page.locator('.notch-cell[data-id=codex]')
    if cell.get_attribute('aria-pressed') != 'true':
        cell.dispatch_event('click')
    assert page.locator('.notch-shell').get_attribute('data-theme') in ['glass', 'liquid']
    page.evaluate('async()=>{for(let i=0;i<60;i++)await new Promise(requestAnimationFrame)}')
    assert page.locator('.notch-shell').get_attribute('data-liquid-status') == 'ready', 'The backdrop must be running, not silently unavailable'
    bounds = page.evaluate('()=>[screenX,screenY,screenX+innerWidth,screenY+innerHeight]')
    windows = []
    @ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
    def visit(hwnd, _):
        rect = wintypes.RECT()
        u.GetWindowRect(hwnd, ctypes.byref(rect))
        if u.IsWindowVisible(hwnd) and [rect.left, rect.top, rect.right, rect.bottom] == bounds:
            value = wintypes.DWORD()
            assert u.GetWindowDisplayAffinity(hwnd, ctypes.byref(value))
            windows.append({'hwnd': int(hwnd), 'affinity': value.value})
        return True
    u.EnumWindows(visit, 0)
    assert len(windows) == 1, windows
    assert windows[0]['affinity'] == 0, 'The live material must remain in OS screenshots: ' + str(windows)
    # Use Windows screen capture, not CDP's renderer-only screenshot (which also
    # succeeds for excluded HWNDs). Save just the widget for visual inspection.
    rect = page.locator('.notch-detail').bounding_box()
    ratio = page.evaluate('devicePixelRatio')
    crop = tuple(round(v) for v in [bounds[0] + rect['x'] * ratio,
        bounds[1] + rect['y'] * ratio, bounds[0] + (rect['x'] + rect['width']) * ratio,
        bounds[1] + (rect['y'] + rect['height']) * ratio])
    image = ImageGrab.grab(bbox=crop, all_screens=True)
    image.save('tmp/liquid-capture-screen.png')
    # Inclusion must also be visible in actual pixels, not just the affinity flag.
    label = page.locator('.provider-entry:visible .provider-name').first.bounding_box()
    label_crop = tuple(round(v) for v in [bounds[0] + label['x'] * ratio,
        bounds[1] + label['y'] * ratio, bounds[0] + (label['x'] + label['width']) * ratio,
        bounds[1] + (label['y'] + label['height']) * ratio])
    text = ImageGrab.grab(bbox=label_crop, all_screens=True)
    assert sum(min(pixel[:3]) > 140 for pixel in text.getdata()) > 10, 'Foreground text missing from OS capture'
    print(json.dumps({'capture_included': True, 'window': windows[0], 'screenshot': 'tmp/liquid-capture-screen.png'}))

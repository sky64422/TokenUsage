"""Fixed-window pixel changes: idle wake, moving bar, and real blur support.

The paced native fixture is intentionally wall-clock driven; renderer unit tests
use fake clocks. Run only against the isolated preview on CDP 9223.
"""
import argparse
import ctypes
import json
import subprocess
import time
import tkinter as tk
from pathlib import Path
from playwright.sync_api import sync_playwright

ROOT = Path(__file__).resolve().parent.parent
PULSE_MS = 150
IDLE_MS = 1400
PULSES = 6
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--label', default='after')
parser.add_argument('--require-responsive', action='store_true')
args = parser.parse_args()
u = ctypes.windll.user32
u.SetProcessDPIAware()
u.FindWindowW.argtypes = [ctypes.c_wchar_p, ctypes.c_wchar_p]
u.FindWindowW.restype = ctypes.c_void_p
u.GetAncestor.argtypes = [ctypes.c_void_p, ctypes.c_uint]
u.GetAncestor.restype = ctypes.c_void_p
u.GetWindowThreadProcessId.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_ulong)]
u.SetWindowPos.argtypes = [ctypes.c_void_p, ctypes.c_void_p, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_uint]

with sync_playwright() as p:
    browser = p.chromium.connect_over_cdp('http://localhost:9223')
    page = browser.contexts[0].pages[0]
    assert page.evaluate('async()=>window.__TAURI__.app.getIdentifier()') == 'com.tokenusage.notch-preview'
    def invoke(command, args=None):
        return page.evaluate('async([c,a])=>window.__TAURI__.core.invoke(c,a)', [command, args or {}])
    invoke('set_theme', {'theme': 'liquid'})
    invoke('set_always_show_notch', {'enabled': True})
    invoke('set_hover_detail', {'enabled': False})
    for provider in ['claude', 'codex', 'grok', 'agy']:
        invoke('set_provider_enabled', {'provider': provider, 'enabled': True})
    page.reload()
    page.wait_for_function('document.querySelector(".notch-shell")?.dataset.liquidStatus==="ready"')
    processes = json.loads(subprocess.check_output(['powershell', '-NoProfile', '-Command',
        'Get-Process token-usage | Select-Object Id,Path | ConvertTo-Json -Compress'], creationflags=0x08000000))
    if isinstance(processes, dict): processes = [processes]
    expected = (ROOT/'src-tauri/target/debug/token-usage.exe').resolve()
    pid = next(row['Id'] for row in processes if Path(row['Path']).resolve() == expected)
    handles = []
    callback_type = ctypes.WINFUNCTYPE(ctypes.c_bool, ctypes.c_void_p, ctypes.c_void_p)
    def find_window(handle, _):
        owner = ctypes.c_ulong()
        u.GetWindowThreadProcessId(handle, ctypes.byref(owner))
        name = ctypes.create_unicode_buffer(256)
        u.GetClassNameW(ctypes.c_void_p(handle), name, 256)
        if owner.value == pid and name.value == 'Tauri Window': handles.append(handle)
        return True
    u.EnumWindows(callback_type(find_window), None)
    assert len(handles) == 1, handles
    hwnd = handles[0]
    pos = page.evaluate('''async()=>{
      const p=await window.__TAURI__.window.getCurrentWindow().innerPosition();
      const r=document.querySelector('.notch').getBoundingClientRect(),s=devicePixelRatio;
      return {x:Math.round(p.x+r.x*s),y:Math.round(p.y+r.y*s),w:Math.round(r.width*s),h:Math.round(r.height*s)};
    }''')
    page.evaluate('''()=>{
      window.motionSamples=[];
      const put=CanvasRenderingContext2D.prototype.putImageData;
      CanvasRenderingContext2D.prototype.putImageData=function(image,...args){
        let red=0;
        for(let i=0;i<image.data.length;i+=4) if(image.data[i]>180&&image.data[i+1]<100) red++;
        motionSamples.push({at:Date.now(),red,width:image.width,height:image.height});
        return put.call(this,image,...args);
      };
    }''')
    margin = 100
    background = tk.Tk()
    background.overrideredirect(True)
    background.attributes('-topmost', True)
    w, h = pos['w']+2*margin, pos['h']+2*margin
    background.geometry(f'{w}x{h}+0+0')
    canvas = tk.Canvas(background, width=w, height=h, background='#195cda', highlightthickness=0)
    canvas.pack()
    background.update()
    bhwnd = u.GetAncestor(background.winfo_id(), 2)
    assert u.SetWindowPos(hwnd, ctypes.c_void_p(-1), 0, 0, 0, 0, 0x13)
    assert u.SetWindowPos(bhwnd, hwnd, pos['x']-margin, pos['y']-margin, w, h, 0x10)
    background.update()
    errors = []
    page.on('pageerror', lambda e: errors.append(str(e)))
    results = []
    try:
        for index in range(PULSES):
            page.wait_for_timeout(IDLE_MS + index*37)
            page.evaluate('window.motionSamples=[]')
            start = page.evaluate('Date.now()')
            canvas.configure(background='#dc3020')
            background.update()
            page.wait_for_timeout(PULSE_MS)
            canvas.configure(background='#195cda')
            background.update()
            page.wait_for_timeout(PULSE_MS)
            samples = page.evaluate('motionSamples')
            hits = [s for s in samples if s['red'] > s['width']*s['height']*.8]
            results.append({'pulse': index, 'hit': bool(hits),
                'arrival_ms': hits[0]['at']-start if hits else None})
        page.wait_for_timeout(IDLE_MS)
        page.evaluate('window.motionSamples=[]')
        bar = canvas.create_rectangle(margin+pos['w']//2-5, 0, margin+pos['w']//2+5, 30,
            fill='#dc3020', outline='')
        start = time.perf_counter()
        steps = 120
        for index in range(steps):
            y = margin-30 + (index % 60)/59*(pos['h']+30)
            canvas.coords(bar, margin+pos['w']//2-5, y, margin+pos['w']//2+5, y+30)
            background.update()
            page.wait_for_timeout(16)
        elapsed = time.perf_counter()-start
        moving = page.evaluate('motionSamples')
        # A bar just above the surface is outside its visible bounds, inside blur support.
        canvas.coords(bar, margin+pos['w']//2-5, margin-12, margin+pos['w']//2+5, margin-2)
        background.update()
        page.wait_for_timeout(IDLE_MS)
        support = page.evaluate('''()=>{
          const c=document.querySelector('.notch .liquid-layer canvas'),ctx=c.getContext('2d');
          const d=ctx.getImageData(0,0,c.width,c.height).data;let red=0;
          for(let i=0;i<d.length;i+=4) if(d[i]>180&&d[i+1]<100)red++;
          return {width:c.width,height:c.height,red,filter:c.style.filter};
        }''')
        page.locator('.notch').screenshot(path=str(ROOT/f'tmp/liquid-motion-{args.label}.png'))
        # The first detail must have pixels even after the source has settled.
        page.locator('.notch-cell').first.dispatch_event('click')
        page.wait_for_function("()=>{const d=document.querySelector('.notch-detail');return !d.hidden&&!d.querySelector('.liquid-layer').hidden}", timeout=800)
        result = {'first_detail_ready_after_idle': True, 'pulses': results, 'moving_paints': len(moving), 'moving_seconds': elapsed,
            'moving_hz': len(moving)/elapsed, 'blur_support': support, 'errors': errors}
        (ROOT/f'tmp/liquid-motion-{args.label}.json').write_text(json.dumps(result, indent=2))
        print(json.dumps(result), flush=True)
        if args.require_responsive:
            assert all(s['hit'] and s['arrival_ms'] < PULSE_MS for s in results), results
            assert result['moving_hz'] >= 40, result
            assert support['red'] > 0, support
        assert not errors, errors
    finally:
        background.destroy()
        page.reload()

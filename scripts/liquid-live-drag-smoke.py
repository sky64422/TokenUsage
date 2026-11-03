"""Actual pointer drag over striped desktop pixels; isolated WGC preview only."""
import ctypes
import json
import math
import os
from pathlib import Path
import subprocess
import time
import tkinter as tk
import pyautogui
from playwright.sync_api import sync_playwright
from notch_test_support import prepare_hover_fixture, reset_notch

ROOT = Path(__file__).resolve().parent.parent
STRIPE_HEIGHT = 80
SWEEPS = 6
SWEEP_SECONDS = .8
MIN_DRAG_PAINT_HZ = 45
u = ctypes.windll.user32
u.SetProcessDPIAware()
u.FindWindowW.argtypes = [ctypes.c_wchar_p, ctypes.c_wchar_p]
u.FindWindowW.restype = ctypes.c_void_p
u.GetAncestor.argtypes = [ctypes.c_void_p, ctypes.c_uint]
u.GetAncestor.restype = ctypes.c_void_p
u.GetWindowThreadProcessId.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_ulong)]
u.GetWindowDisplayAffinity.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_uint)]
u.SetWindowPos.argtypes = [ctypes.c_void_p, ctypes.c_void_p, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_uint]
log = Path(os.environ['LOCALAPPDATA']) / 'TokenUsage/logs/capture-debug.log'

with sync_playwright() as p:
    browser = p.chromium.connect_over_cdp('http://127.0.0.1:9223')
    page = browser.contexts[0].pages[0]
    prepare_hover_fixture(page)
    def invoke(command, args=None):
        return page.evaluate('async([c,a])=>window.__TAURI__.core.invoke(c,a)', [command, args or {}])
    original = invoke('get_state')['settings']['notch']
    invoke('set_theme', {'theme':'liquid'})
    invoke('set_hover_detail', {'enabled':False})
    reset_notch(page)
    monitor = next(m for m in invoke('get_notch_monitors') if m['bounds']['x']==0 and m['bounds']['y']==0)
    layout = invoke('set_notch_placement', {'placement':{'edge':'right','offset':.5,'monitor_hint':monitor['name']}})
    page.wait_for_function('document.querySelector(".notch").dataset.edge==="right"')
    page.wait_for_function('document.querySelector(".notch-shell").dataset.liquidStatus==="ready"')
    hwnd = u.FindWindowW('Tauri Window', 'Token Usage')
    pid = ctypes.c_ulong()
    u.GetWindowThreadProcessId(hwnd, ctypes.byref(pid))
    executable = subprocess.check_output(['powershell','-NoProfile','-Command',
        f'(Get-Process -Id {pid.value}).Path'], creationflags=0x08000000).decode().strip()
    assert Path(executable).resolve() == (ROOT/'src-tauri/target/debug/token-usage.exe').resolve()
    width, height = pyautogui.size()
    backdrop = tk.Tk()
    backdrop.overrideredirect(True)
    backdrop.attributes('-topmost', True)
    backdrop.geometry(f'160x{height}+{width-160}+0')
    canvas = tk.Canvas(backdrop, width=160, height=height, highlightthickness=0, borderwidth=0)
    canvas.pack()
    for y in range(0,height,STRIPE_HEIGHT):
        canvas.create_rectangle(0,y,160,y+STRIPE_HEIGHT,fill='#dc3020' if y//STRIPE_HEIGHT%2 else '#195cda',outline='')
    backdrop.update()
    assert u.SetWindowPos(hwnd, ctypes.c_void_p(-1),0,0,0,0,0x13)
    assert u.SetWindowPos(u.GetAncestor(backdrop.winfo_id(),2),hwnd,0,0,0,0,0x13)
    errors=[]
    page.on('pageerror',lambda error:errors.append(str(error)))
    page.evaluate('''()=>{
      window.dragPaints=[];
      const put=CanvasRenderingContext2D.prototype.putImageData;
      CanvasRenderingContext2D.prototype.putImageData=function(image,...args){
        window.dragPaints.push({at:performance.now(),rgb:[...image.data.slice(0,3)],dragging:document.querySelector('.notch').classList.contains('is-dragging')});
        return put.call(this,image,...args);
      };
    }''')
    page.wait_for_timeout(1200)
    initial_filter = page.locator('.notch .liquid-layer canvas').evaluate('e=>e.style.filter')
    position=page.evaluate('''async()=>{const p=await window.__TAURI__.window.getCurrentWindow().innerPosition();const r=document.querySelector('.notch-cell').getBoundingClientRect();return {x:p.x+(r.x+r.width/2)*devicePixelRatio,y:p.y+(r.y+r.height/2)*devicePixelRatio}}''')
    x=position['x']
    start_y=position['y']
    pyautogui.PAUSE=0
    # PyAutoGUI defaults to 50ms steps even for duration-based moves; that would
    # measure a 20Hz synthetic mouse instead of the renderer's smoothness.
    pyautogui.MINIMUM_SLEEP=.008
    try:
        pyautogui.moveTo(x,start_y,duration=.2)
        page.wait_for_function('document.querySelector(".notch-cell:hover")!==null')
        pyautogui.mouseDown()
        pyautogui.moveTo(x,start_y+20,duration=.15)
        page.wait_for_function('document.querySelector(".notch").classList.contains("is-dragging")')
        page.wait_for_timeout(150)
        page.evaluate('window.dragPaints=[]')
        log_before = log.read_text(encoding='utf-8').count('monitor session started')
        start=time.perf_counter()
        for i in range(SWEEPS):
            pyautogui.moveTo(x,start_y+(160 if i%2 else -160),duration=SWEEP_SECONDS)
        elapsed=time.perf_counter()-start
        paints=page.evaluate('dragPaints.filter(s=>s.dragging)')
        intervals=sorted(b['at']-a['at'] for a,b in zip(paints,paints[1:]))
        assert len(paints)>=elapsed*MIN_DRAG_PAINT_HZ, (len(paints),elapsed)
        assert len(paints)<=elapsed*63, (len(paints),elapsed)
        assert len({tuple(s['rgb']) for s in paints})>=2, 'Background froze during motion'
        assert page.locator('.notch .liquid-layer canvas').evaluate('e=>e.style.filter')==initial_filter
        starts=log.read_text(encoding='utf-8').count('monitor session started')-log_before
        assert starts==0, starts
        # While still dragging, stop at a position and require its actual stripe
        # color, not merely repeated paints of old-position pixels.
        pyautogui.moveTo(x,start_y+100)
        position=page.evaluate('''async()=>{const p=await window.__TAURI__.window.getCurrentWindow().innerPosition();const r=document.querySelector('.notch').getBoundingClientRect();return Math.round(p.y+r.y*devicePixelRatio)}''')
        expected=[220,48,32] if position//STRIPE_HEIGHT%2 else [25,92,218]
        page.wait_for_function('(rgb)=>dragPaints.at(-1)?.rgb.every((v,i)=>v===rgb[i])',arg=expected,timeout=800)
        affinity=ctypes.c_uint()
        assert u.GetWindowDisplayAffinity(hwnd,ctypes.byref(affinity)) and affinity.value==0
        page.locator('.notch').screenshot(path=str(ROOT/'tmp/liquid-live-drag.png'))
        page.evaluate('window.dragPaints=[]')
        pyautogui.mouseUp()
        page.wait_for_function('!document.querySelector(".notch").classList.contains("is-dragging")')
        page.wait_for_function('dragPaints.some(s=>!s.dragging)',timeout=1000)
        assert not errors,errors
        result={'elapsed_s':elapsed,'moving_paints':len(paints),'paint_intervals_ms':{'p50':intervals[len(intervals)//2],'p95':intervals[math.ceil(len(intervals)*.95)-1],'max':max(intervals)},'distinct_colors':len({tuple(s['rgb']) for s in paints}),
            'session_restarts_during_motion':starts,'filter_preserved':True,'final_stripe_rgb':expected,
            'affinity':affinity.value,'release_repaint':True,'errors':errors}
        (ROOT/'tmp/liquid-live-drag-result.json').write_text(json.dumps(result,indent=2))
        print(json.dumps(result))
    finally:
        pyautogui.mouseUp()
        backdrop.destroy()
        reset_notch(page)
        invoke('set_notch_placement',{'placement':original})

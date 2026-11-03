"""Exercise failed WGC source geometry and queued resets without changing OS policy."""
import ctypes
from ctypes import wintypes
import json
import subprocess
import tkinter as tk
from pathlib import Path
from playwright.sync_api import sync_playwright

ROOT = Path(__file__).resolve().parent.parent
u = ctypes.windll.user32
u.SetProcessDPIAware()
u.FindWindowW.argtypes = [ctypes.c_wchar_p, ctypes.c_wchar_p]
u.FindWindowW.restype = ctypes.c_void_p
u.GetWindowRect.argtypes = [ctypes.c_void_p, ctypes.POINTER(wintypes.RECT)]
u.GetWindowThreadProcessId.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_ulong)]
u.GetWindowDisplayAffinity.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_uint)]
u.SetWindowPos.argtypes = [ctypes.c_void_p, ctypes.c_void_p, ctypes.c_int, ctypes.c_int,
                         ctypes.c_int, ctypes.c_int, ctypes.c_uint]
NO_SIZE_ZORDER_ACTIVATION = 0x0015

with sync_playwright() as p:
    browser = p.chromium.connect_over_cdp('http://127.0.0.1:9223')
    page = browser.contexts[0].pages[0]
    assert page.evaluate('async()=>window.__TAURI__.app.getIdentifier()') == 'com.tokenusage.notch-preview'
    page.evaluate('async()=>{await window.__TAURI__.core.invoke("set_theme",{theme:"liquid"});await window.__TAURI__.core.invoke("set_always_show_notch",{enabled:true})}')
    page.reload()
    page.wait_for_function('document.querySelector(".notch-shell")?.dataset.liquidStatus==="ready"')
    hwnd = u.FindWindowW('Tauri Window', 'Token Usage')
    assert hwnd
    process_id = ctypes.c_ulong()
    u.GetWindowThreadProcessId(hwnd, ctypes.byref(process_id))
    executable = subprocess.check_output(['powershell','-NoProfile','-Command',
        f'(Get-Process -Id {process_id.value}).Path'], creationflags=0x08000000).decode().strip()
    assert Path(executable).resolve() == (ROOT/'src-tauri/target/debug/token-usage.exe').resolve(), executable
    original = wintypes.RECT()
    assert u.GetWindowRect(hwnd, ctypes.byref(original))
    # Beyond the current virtual desktop; restore even if IPC/renderer assertions fail.
    virtual_right = u.GetSystemMetrics(76) + u.GetSystemMetrics(78)
    offscreen_x = virtual_right + (original.right - original.left) + 100
    try:
        assert u.SetWindowPos(hwnd, None, offscreen_x, original.top, 0, 0, NO_SIZE_ZORDER_ACTIVATION)
        failure = page.evaluate('''async()=>{
          try { await window.__TAURI__.core.invoke('capture_liquid_backdrop',
            {x:0,y:0,width:8,height:8,surface:0,generation:900001});return null; }
          catch(error) { return String(error); }
        }''')
        assert failure and 'WGC (no GDI fallback)' in failure and 'no monitor' in failure, failure
        affinity = ctypes.c_uint()
        assert u.GetWindowDisplayAffinity(hwnd, ctypes.byref(affinity))
        assert affinity.value == 0, affinity.value
    finally:
        assert u.SetWindowPos(hwnd, None, original.left, original.top, 0, 0, NO_SIZE_ZORDER_ACTIVATION)
        page.reload()
    page.wait_for_function('document.querySelector(".notch-shell")?.dataset.liquidStatus==="ready"')
    reset_bytes = page.evaluate('''async()=>{
      await Promise.all(Array.from({length:20},()=>window.__TAURI__.core.invoke('destroy_liquid_backdrop')));
      const deadline=performance.now()+3000;
      while(performance.now()<deadline) {
        const bytes=(await window.__TAURI__.core.invoke('capture_liquid_backdrop',
          {x:0,y:0,width:8,height:8,surface:0,generation:900002})).byteLength;
        if(bytes) return bytes;
        await new Promise(requestAnimationFrame);
      }
      throw new Error('No fresh frame after reset storm');
    }''')
    assert reset_bytes == 12 + 8 * 8 * 4, reset_bytes
    # Alternate differently sized surfaces and generations. A quiet desktop must
    # still supply a first image when detail opens or its canvas is recreated.
    backdrop = tk.Tk()
    backdrop.overrideredirect(True)
    backdrop.attributes('-topmost', True)
    backdrop.geometry(f'64x32{original.left:+d}{original.top:+d}')
    canvas = tk.Canvas(backdrop, width=64, height=32, highlightthickness=0, borderwidth=0)
    canvas.pack()
    canvas.create_rectangle(0, 0, 32, 32, fill='#dc3020', outline='')
    canvas.create_rectangle(32, 0, 64, 32, fill='#195cda', outline='')
    backdrop.update()
    u.GetAncestor.argtypes = [ctypes.c_void_p, ctypes.c_uint]
    u.GetAncestor.restype = ctypes.c_void_p
    assert u.SetWindowPos(hwnd, ctypes.c_void_p(-1), 0, 0, 0, 0, 0x13)
    assert u.SetWindowPos(u.GetAncestor(backdrop.winfo_id(), 2), hwnd, 0, 0, 0, 0, 0x13)
    try:
        alternating = page.evaluate('''async()=>{
      let frames=0;
      for(let i=0;i<40;i++) {
        const surface=i%2,width=surface?16:8,height=surface?12:8,padding=(i%3)*2;
        const outputWidth=width+2*padding,outputHeight=height+2*padding;
        const expected=surface?[25,92,218]:[220,48,32];
        const deadline=performance.now()+3000;
        let bytes,correct=false;
        do {
          bytes=new Uint8Array(await window.__TAURI__.core.invoke('capture_liquid_backdrop',
            {x:surface?40:8,y:8,width,height,padding,surface,generation:910000+i}));
          correct=bytes.length===12+outputWidth*outputHeight*4 &&
            bytes.slice(12).every((value,index)=>value===(index%4===3?255:expected[index%4]));
          if(correct) break;
          await new Promise(requestAnimationFrame);
        } while(performance.now()<deadline);
        if(!correct) throw new Error(`Wrong static ROI pixels ${i}: ${[...bytes.slice(12,16)]}`);
        const header=new DataView(bytes.buffer);
        if(bytes.length!==12+outputWidth*outputHeight*4 || String.fromCharCode(...bytes.slice(0,4))!=='LQ01'
           || header.getUint32(4,true)!==outputWidth || header.getUint32(8,true)!==outputHeight)
          throw new Error(`Invalid alternating surface frame ${i}: ${bytes.length}`);
        frames++;
      }
      // Raw diagnostic callers do not opt into empty cache-hit responses.
      for(let i=0;i<2;i++) {
        const raw=new Uint8Array(await window.__TAURI__.core.invoke('capture_liquid_backdrop',
          {x:8,y:8,width:8,height:8}));
        if(raw.length!==54+8*8*4 || raw[0]!==66 || raw[1]!==77)
          throw new Error(`Raw snapshot cache regression: ${raw.length}`);
      }
      return frames;
    }''')
    finally:
        backdrop.destroy()
    assert alternating == 40, alternating
    page.reload()
    page.wait_for_function('document.querySelector(".notch-shell")?.dataset.liquidStatus==="ready"')
    result = {'invalid_monitor_error': failure, 'failure_affinity': affinity.value,
              'restored_ready': True, 'concurrent_resets': 20, 'fresh_frame_bytes': reset_bytes,
              'alternating_surface_generations': alternating, 'padding_variants': [0,2,4], 'raw_snapshots': 2}
    (ROOT/'tmp/liquid-wgc-capability').mkdir(parents=True, exist_ok=True)
    (ROOT/'tmp/liquid-wgc-capability/recovery-result.json').write_text(json.dumps(result,indent=2))
    print(json.dumps(result))

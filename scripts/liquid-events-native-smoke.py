"""Real window events, GDI pixels and exclusion release in the isolated preview."""
import ctypes
import argparse
import json
import math
import subprocess
import tkinter as tk
from pathlib import Path
from playwright.sync_api import sync_playwright

ROOT = Path(__file__).resolve().parent.parent
OBSERVER_FRAMES = 200
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--screenshots', action='store_true', help='Observe layered OS screenshots during changing pixels')
parser.add_argument('--latency-samples',type=int,default=1)
parser.add_argument('--screen-samples', type=int, default=20)
parser.add_argument('--require-visible', action='store_true', help='Fail on any active screenshot omission or nonzero affinity')
parser.add_argument('--observer', action='store_true', help='Run the compiled independent WGC observer alongside the screenshot sequence')
parser.add_argument('--observer-control', action='store_true', help='Hide notch rendering to require the observer to detect missing foreground')
args = parser.parse_args()
user32 = ctypes.windll.user32
user32.SetProcessDPIAware()
with sync_playwright() as p:
    browser = p.chromium.connect_over_cdp('http://127.0.0.1:9223')
    page = browser.contexts[0].pages[0]
    assert page.evaluate('async()=>window.__TAURI__.app.getIdentifier()') == 'com.tokenusage.notch-preview'
    page.evaluate('''async()=>{
      await window.__TAURI__.core.invoke('set_theme',{theme:'liquid'});
      await window.__TAURI__.core.invoke('set_always_show_notch',{enabled:true});
    }''')
    page.reload()
    page.wait_for_function('document.querySelector(".notch-shell")?.dataset.liquidStatus==="ready"')
    errors = []
    page.on('pageerror', lambda e: errors.append(str(e)))
    position = page.evaluate('''async()=>{
      const pos=await window.__TAURI__.window.getCurrentWindow().innerPosition();
      const r=document.querySelector('.notch').getBoundingClientRect(),s=devicePixelRatio;
      return {x:Math.round(pos.x+r.x*s),y:Math.round(pos.y+r.y*s),w:Math.round(r.width*s),h:Math.round(r.height*s)};
    }''')
    page.evaluate('''async()=>{
      window.captureSamples=[];window.windowEvents=[];
      const original=CanvasRenderingContext2D.prototype.putImageData;
      CanvasRenderingContext2D.prototype.putImageData=function(image,...args){
        window.captureSamples.push({at:Date.now(),bytes:image.data.length,rgb:[...image.data.slice(0,3)]});
        return original.call(this,image,...args);
      };
      window.stopEvents=await window.__TAURI__.event.listen('liquid-backdrop-changed',()=>window.windowEvents.push(Date.now()));
    }''')
    user32.FindWindowW.argtypes = [ctypes.c_wchar_p, ctypes.c_wchar_p]
    user32.FindWindowW.restype = ctypes.c_void_p
    user32.GetWindowDisplayAffinity.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_uint)]
    title = page.evaluate('async()=>window.__TAURI__.window.getCurrentWindow().title()')
    hwnd = user32.FindWindowW('Tauri Window', title)
    assert hwnd
    user32.GetWindowThreadProcessId.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_ulong)]
    process_id = ctypes.c_ulong()
    user32.GetWindowThreadProcessId(hwnd, ctypes.byref(process_id))
    executable = subprocess.check_output(['powershell','-NoProfile','-Command',
        f'(Get-Process -Id {process_id.value}).Path'], creationflags=0x08000000).decode().strip()
    assert Path(executable).resolve() == (ROOT/'src-tauri/target/debug/token-usage.exe').resolve(), executable
    backdrop = tk.Tk()
    backdrop.overrideredirect(True)
    backdrop.attributes('-topmost', True)
    backdrop.configure(background='#195cda')
    x, y = position['x'] - 40, position['y'] - 40
    backdrop.geometry(f"{position['w']+80}x{position['h']+80}{x:+d}{y:+d}")
    observer = None
    try:
        backdrop.update()
        backdrop.lift()
        backdrop.update()
        user32.SetWindowPos.argtypes = [ctypes.c_void_p, ctypes.c_void_p, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_uint]
        user32.SetWindowPos(hwnd, ctypes.c_void_p(-1), 0, 0, 0, 0, 0x0013)
        user32.GetAncestor.argtypes = [ctypes.c_void_p, ctypes.c_uint]
        user32.GetAncestor.restype = ctypes.c_void_p
        background_hwnd = user32.GetAncestor(backdrop.winfo_id(), 2)
        user32.SetWindowPos(background_hwnd, hwnd, 0, 0, 0, 0, 0x0013)
        page.wait_for_function('window.windowEvents.length>0 && window.captureSamples.some(s=>s.bytes>0)')
        # Let the static background settle, then send one native window move.
        page.wait_for_timeout(1600)
        start = page.evaluate('Date.now()')
        page.evaluate('window.captureSamples=[];window.windowEvents=[]')
        backdrop.geometry(f"{position['w']+80}x{position['h']+80}{x-4:+d}{y:+d}")
        backdrop.configure(background='#dc3020')
        backdrop.update()
        page.wait_for_function('window.windowEvents.length>0 && window.captureSamples.some(s=>s.bytes>0)', timeout=800)
        latency = page.evaluate('(start)=>captureSamples.find(s=>s.bytes>0).at-start', start)
        assert latency < 800, latency
        latencies=[latency]
        for index in range(args.latency_samples-1):
            red=index%2==1
            expected=220 if red else 25
            start=page.evaluate('Date.now()')
            backdrop.configure(background='#dc3020' if red else '#195cda')
            backdrop.geometry(f"{position['w']+80}x{position['h']+80}{x-(index%2)*4:+d}{y:+d}")
            backdrop.update()
            page.wait_for_function('([start,red])=>captureSamples.some(s=>s.at>=start&&s.rgb[0]===red)',arg=[start,expected],timeout=800)
            latencies.append(page.evaluate('([start,red])=>captureSamples.find(s=>s.at>=start&&s.rgb[0]===red).at-start',[start,expected]))
        page.evaluate('window.captureSamples=[]')
        affinities = []
        screenshot_variation = []
        if args.observer_control:
            assert args.observer and not args.require_visible
            page.evaluate('document.querySelector(".notch").style.visibility="hidden"')
        if args.observer:
            observer = subprocess.Popen([str(ROOT/'tmp/liquid-wgc-capability/probe.exe'), '--observe',
                str(position['x']),str(position['y']),str(position['w']),str(position['h']), str(OBSERVER_FRAMES)],
                stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, creationflags=0x08000000)
        for i in range(args.screen_samples):
            backdrop.configure(background='#195cda' if i % 2 else '#dc3020')
            backdrop.update()
            page.wait_for_timeout(65)
            affinity = ctypes.c_uint()
            assert user32.GetWindowDisplayAffinity(hwnd, ctypes.byref(affinity))
            affinities.append(affinity.value)
            if args.screenshots:
                from PIL import ImageGrab
                ox,oy=user32.GetSystemMetrics(76),user32.GetSystemMetrics(77)
                image=ImageGrab.grab(all_screens=True,include_layered_windows=True).crop((position['x']-ox,position['y']-oy,position['x']+position['w']-ox,position['y']+position['h']-oy))
                # The backdrop is one solid color; visible notch chrome is non-uniform.
                colors=image.getcolors(maxcolors=image.width*image.height)
                visible=len(colors)>16
                if visible not in screenshot_variation:
                    image.save(str(ROOT / ('tmp/liquid-sharing-visible.png' if visible else 'tmp/liquid-sharing-absent.png')))
                screenshot_variation.append(visible)
        observed = None
        if observer:
            stdout, stderr = observer.communicate(timeout=48)
            observed = json.loads(stdout)
            assert observed['observer_frames'] == OBSERVER_FRAMES, (stdout, stderr)
            if args.observer_control:
                assert observer.returncode == 1 and observed['missing'] == OBSERVER_FRAMES, (stdout, stderr)
            else:
                assert observer.returncode == 0 and observed['missing'] == 0, (stdout, stderr)
        if args.observer_control:
            page.evaluate('document.querySelector(".notch").style.removeProperty("visibility")')
        sample = page.evaluate('({changed_paints:captureSamples.length})')
        sample['normal_affinity_samples'] = affinities.count(0)
        sample['affinity_samples'] = len(affinities)
        assert sample['changed_paints'] >= 10 and sample['normal_affinity_samples'] >= 5, sample
        page.wait_for_function('document.querySelector(".notch-shell").dataset.liquidStatus==="ready"')
        user32.SetWindowPos(hwnd, ctypes.c_void_p(-1), 0, 0, 0, 0, 0x0013)
        page.wait_for_timeout(600)
        page.evaluate('async()=>window.__TAURI__.core.invoke("release_liquid_backdrop")')
        page.wait_for_timeout(100)
        page.locator('.notch').screenshot(path=str(ROOT / 'tmp/liquid-events-webview.png'))
        affinity = ctypes.c_uint()
        assert user32.GetWindowDisplayAffinity(hwnd, ctypes.byref(affinity))
        assert affinity.value == 0
        from PIL import ImageGrab
        # Transparent HWNDs are layered; Pillow's default screenshot omits them.
        origin_x, origin_y = user32.GetSystemMetrics(76), user32.GetSystemMetrics(77)
        ImageGrab.grab(all_screens=True, include_layered_windows=True).crop((x-origin_x, y-origin_y, position['x']+position['w']-origin_x, y+position['h']+80-origin_y)).save(str(ROOT / 'tmp/liquid-events-native.png'))
        assert not errors, errors
        result = {'event_to_changed_frame_ms': latency, 'continuous_background': sample, 'errors': errors}
        result['latency_ms']={'samples':latencies,'p50':sorted(latencies)[math.ceil(len(latencies)*.5)-1],'p95':sorted(latencies)[math.ceil(len(latencies)*.95)-1]}
        if args.screenshots:
            result['active_screenshot_notch_visible']=screenshot_variation
        if observed:
            result['independent_observer'] = observed
        (ROOT / 'tmp/liquid-events-native-results.json').write_text(json.dumps(result, indent=2))
        print(json.dumps(result))
        if args.require_visible:
            assert args.screenshots and screenshot_variation and all(screenshot_variation), result
            assert all(value == 0 for value in affinities), result
    finally:
        if observer and observer.poll() is None:
            observer.terminate()
            observer.wait(timeout=5)
        page.evaluate('document.querySelector(".notch").style.removeProperty("visibility")')
        backdrop.destroy()
        page.evaluate('window.stopEvents?.()')

"""ABBA frontend comparison on one native backend; no installed-profile mutations.

Baseline transformed modules are generated from the requested git ref under tmp/.
Real elapsed windows are measurements, not timing-based functional assertions.
"""
import ctypes
import argparse
import json
import os
import subprocess
import time
import tkinter as tk
from pathlib import Path
from playwright.sync_api import sync_playwright

ROOT = Path(__file__).resolve().parent.parent
SAMPLE_SECONDS = 8
CREATE_NO_WINDOW = 0x08000000
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--gpu', action='store_true', help='Additional coarse GPU-engine sampling (A/B only)')
parser.add_argument('--baseline-ref', default='HEAD')
parser.add_argument('--backend-label', help='Measure current renderer only; caller must launch the named backend')
parser.add_argument('--seconds', type=int, default=SAMPLE_SECONDS)
parser.add_argument('--raw-gpu', action='store_true', help='Difference raw per-process GPU engine counters over a five-second interval')
parser.add_argument('--preview-owner-pid', type=int, help='Require the measured HWND to descend from the launcher process')
args = parser.parse_args()
baseline_commit = subprocess.check_output(['git','rev-parse',args.baseline_ref],cwd=ROOT).decode().strip()
baseline_folder = ROOT/'tmp/liquid-baseline'
baseline_folder.mkdir(parents=True,exist_ok=True)
for name in ([] if args.backend_label else ['liquid-loop.ts','liquid-glass.ts']):
    source=subprocess.check_output(['git','show',f'{baseline_commit}:src/ui/{name}'],cwd=ROOT).decode()
    if name=='liquid-glass.ts':
        source=source.replace('"./','"../../src/ui/').replace('"../../src/ui/liquid-loop.ts"','"./liquid-loop.ts"')
        source=source.replace('    relayout() {','    refresh() {},\n    relayout() {')
    (baseline_folder/name).write_text(source,encoding='utf-8')

def powershell(code):
    return json.loads(subprocess.check_output(['powershell', '-NoProfile', '-Command', code], creationflags=CREATE_NO_WINDOW))

u = ctypes.windll.user32
u.SetProcessDPIAware()
u.FindWindowW.argtypes = [ctypes.c_wchar_p, ctypes.c_wchar_p]
u.FindWindowW.restype = ctypes.c_void_p
u.GetWindowThreadProcessId.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_ulong)]
u.SetWindowPos.argtypes = [ctypes.c_void_p, ctypes.c_void_p, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_uint]
u.GetAncestor.argtypes = [ctypes.c_void_p, ctypes.c_uint]
u.GetAncestor.restype = ctypes.c_void_p

with sync_playwright() as p:
    browser = p.chromium.connect_over_cdp('http://localhost:9223')
    page = browser.contexts[0].pages[0]
    assert page.evaluate('async()=>window.__TAURI__.app.getIdentifier()') == 'com.tokenusage.notch-preview'
    page.evaluate('''async()=>{
      await window.__TAURI__.core.invoke('set_theme',{theme:'liquid'});
      await window.__TAURI__.core.invoke('set_always_show_notch',{enabled:true});
      await window.__TAURI__.core.invoke('set_show_animation',{enabled:false});
    }''')
    hwnd = u.FindWindowW('Tauri Window', 'Token Usage')
    assert hwnd
    pid = ctypes.c_ulong()
    u.GetWindowThreadProcessId(hwnd, ctypes.byref(pid))
    executable = powershell(f'(Get-Process -Id {pid.value}).Path | ConvertTo-Json -Compress')
    assert Path(executable).resolve() == (ROOT/'src-tauri/target/debug/token-usage.exe').resolve(), executable
    rows = powershell('Get-CimInstance Win32_Process | Select-Object ProcessId,ParentProcessId | ConvertTo-Json -Compress')
    if args.preview_owner_pid:
        parents = {r['ProcessId']: r['ParentProcessId'] for r in rows}
        ancestor, visited = pid.value, set()
        while ancestor != args.preview_owner_pid and ancestor in parents and ancestor not in visited:
            visited.add(ancestor)
            ancestor = parents[ancestor]
        assert ancestor == args.preview_owner_pid, (pid.value, args.preview_owner_pid, visited)
    ids = {pid.value}
    while True:
        found = {r['ProcessId'] for r in rows if r['ParentProcessId'] in ids}
        if found <= ids: break
        ids |= found
    id_list = ','.join(map(str, sorted(ids)))

    def resources():
        return powershell(f'''$r=Get-Process -Id {id_list} -ErrorAction Stop;
          $dwmReadable=@(Get-Process dwm|Where-Object {{$null -ne $_.CPU}});
          [pscustomobject]@{{time=[DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds();
          cpu=($r|Measure-Object CPU -Sum).Sum;private=($r|Measure-Object PrivateMemorySize64 -Sum).Sum;
          dwm_cpu=$(if($dwmReadable.Count){{($dwmReadable|Measure-Object CPU -Sum).Sum}}else{{$null}});
          handles=($r|Measure-Object HandleCount -Sum).Sum;count=$r.Count}}|ConvertTo-Json -Compress''')

    results = []
    backdrop = tk.Tk()
    backdrop.overrideredirect(True)
    backdrop.attributes('-topmost', True)
    try:
        for variant in (['current'] if args.backend_label else ['baseline', 'current'] if args.gpu else ['baseline', 'current', 'current', 'baseline']):
            page.unroute('**/src/ui/liquid-glass.ts')
            if variant == 'baseline':
                def baseline(route):
                    route.fulfill(response=route.fetch(url='http://localhost:1524/tmp/liquid-baseline/liquid-glass.ts'))
                page.route('**/src/ui/liquid-glass.ts', baseline)
            page.reload()
            page.wait_for_function('document.querySelector(".notch-shell")?.dataset.liquidStatus==="ready"')
            pos = page.evaluate('''async()=>{
              const p=await window.__TAURI__.window.getCurrentWindow().innerPosition();
              const r=document.querySelector('.notch').getBoundingClientRect(),s=devicePixelRatio;
              return {x:Math.round(p.x+r.x*s),y:Math.round(p.y+r.y*s),w:Math.round(r.width*s),h:Math.round(r.height*s)};
            }''')
            x,y=pos['x']-40,pos['y']-40
            backdrop.geometry(f"{pos['w']+80}x{pos['h']+80}{x:+d}{y:+d}")
            backdrop.configure(background='#195cda')
            backdrop.update()
            u.SetWindowPos(hwnd,ctypes.c_void_p(-1),0,0,0,0,0x13)
            u.SetWindowPos(u.GetAncestor(backdrop.winfo_id(),2),hwnd,0,0,0,0,0x13)
            page.evaluate('''()=>{
              window.benchPaints=0;window.benchRaf=0;
              const put=CanvasRenderingContext2D.prototype.putImageData;
              CanvasRenderingContext2D.prototype.putImageData=function(...args){window.benchPaints++;return put.apply(this,args)};
              const raf=window.requestAnimationFrame;
              window.requestAnimationFrame=cb=>raf(t=>{window.benchRaf++;cb(t)});
            }''')
            for workload in ['static','moving','pixel_changes'] + (['unrelated_pixels'] if args.raw_gpu else []):
                if workload == 'unrelated_pixels':
                    # The default primary right-edge fixture does not overlap this patch.
                    assert pos['x'] > 350 or pos['y'] > 350, pos
                    backdrop.geometry('180x180+100+100')
                    backdrop.update()
                page.wait_for_timeout(1600)
                before=resources()
                gpu_process = None
                if args.raw_gpu:
                    gpu_process = subprocess.Popen(['powershell','-NoProfile','-ExecutionPolicy','Bypass','-File',
                        str(ROOT/'scripts/liquid-gpu-sample.ps1'),'-ProcessIds',id_list],
                        stdout=subprocess.PIPE, creationflags=CREATE_NO_WINDOW)
                elif args.gpu:
                    pattern = '^pid_(' + '|'.join(map(str, sorted(ids))) + ')_'
                    gpu_process = subprocess.Popen(['powershell','-NoProfile','-Command',
                        f"$s=@(); 1..5 | ForEach-Object {{ $r=Get-CimInstance Win32_PerfFormattedData_GPUPerformanceCounters_GPUEngine | Where-Object Name -match '{pattern}'; $s+= [double](($r|Measure-Object UtilizationPercentage -Sum).Sum); Start-Sleep -Milliseconds 1000 }}; ConvertTo-Json -InputObject $s -Compress"],
                        stdout=subprocess.PIPE, creationflags=CREATE_NO_WINDOW)
                page.evaluate('window.benchPaints=0;window.benchRaf=0')
                start=time.perf_counter();step=0
                while time.perf_counter()-start<args.seconds:
                    if workload!='static':
                        backdrop.configure(background='#195cda' if step%2 else '#dc3020')
                        if workload=='moving':
                            backdrop.geometry(f"{pos['w']+80}x{pos['h']+80}{x-(step%2)*4:+d}{y:+d}")
                        backdrop.update()
                    page.wait_for_timeout(65)
                    step+=1
                counts=page.evaluate('({paints:benchPaints,raf:benchRaf})')
                after=resources()
                elapsed=(after['time']-before['time'])/1000
                result={'variant':variant,'workload':workload,'elapsed_s':elapsed,
                    'backend_label':args.backend_label,
                    'dwm_cpu_one_core_percent':None if before['dwm_cpu'] is None or after['dwm_cpu'] is None else (after['dwm_cpu']-before['dwm_cpu'])/elapsed*100,
                    'cpu_one_core_percent':(after['cpu']-before['cpu'])/elapsed*100,
                    'cpu_machine_percent':(after['cpu']-before['cpu'])/elapsed*100/os.cpu_count(),
                    'private_mib':after['private']/1024**2,'handles':after['handles'],
                    'process_count':after['count'],**counts}
                if gpu_process:
                    output,_=gpu_process.communicate(timeout=15)
                    assert gpu_process.returncode == 0
                    result['gpu_raw_counters' if args.raw_gpu else 'gpu_engine_percent_sum_samples']=json.loads(output)
                results.append(result);print(json.dumps(result),flush=True)
                output_name=f'liquid-backend-{args.backend_label}.json' if args.backend_label else 'liquid-native-gpu.json' if args.gpu else 'liquid-native-benchmark.json'
                (ROOT/'tmp'/output_name).write_text(json.dumps({'samples':results,'pids':sorted(ids),'logical_cpus':os.cpu_count(),'baseline_commit':baseline_commit,'scope':'Current renderer; backend selected by caller environment.' if args.backend_label else 'Frontend A/B on shared current native backend; native hook and log changes are not isolated.'},indent=2))
    finally:
        backdrop.destroy()
        page.unroute('**/src/ui/liquid-glass.ts')
        page.reload()

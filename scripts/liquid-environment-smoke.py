"""Native monitor/edge/reload checks and a bounded resource observation window."""
import argparse
import ctypes
import json
import subprocess
import time
from pathlib import Path
from playwright.sync_api import sync_playwright, expect
from notch_test_support import reset_notch

parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--seconds',type=int,default=180)
args=parser.parse_args()
ROOT=Path(__file__).resolve().parent.parent

def resources():
    code="""$root=Get-Process token-usage | Where-Object Path -eq 'C:\\dev\\TokenUsage\\src-tauri\\target\\debug\\token-usage.exe';
    $ids=@($root.Id);$rows=Get-CimInstance Win32_Process;
    do {$old=$ids.Count;$ids=@($ids)+@($rows|Where-Object {$_.ParentProcessId -in $ids}|Select-Object -ExpandProperty ProcessId);$ids=@($ids|Select-Object -Unique)} while($ids.Count -ne $old);
    Get-Process -Id $ids -ErrorAction Stop | Select-Object Id,CPU,PrivateMemorySize64,HandleCount | ConvertTo-Json -Compress"""
    return json.loads(subprocess.check_output(['powershell','-NoProfile','-Command',code],creationflags=0x08000000))

with sync_playwright() as p:
    browser=p.chromium.connect_over_cdp('http://localhost:9223')
    page=browser.contexts[0].pages[0]
    assert page.evaluate('async()=>window.__TAURI__.app.getIdentifier()')=='com.tokenusage.notch-preview'
    def invoke(command,args={}):
        return page.evaluate('async([c,a])=>window.__TAURI__.core.invoke(c,a)',[command,args])
    original=invoke('get_state')['settings']['notch']
    monitors=invoke('get_notch_monitors')
    errors=[]
    page.on('pageerror',lambda error:errors.append(str(error)))
    checks=[]
    try:
        invoke('set_theme',{'theme':'liquid'})
        invoke('set_always_show_notch',{'enabled':True})
        page.reload()
        page.wait_for_function('document.querySelector(".notch-shell")?.dataset.liquidStatus==="ready"')
        for monitor in monitors:
            for edge in ['right','left','top','bottom']:
                reset_notch(page)
                layout=invoke('set_notch_placement',{'placement':{'edge':edge,'offset':.5,'monitor_hint':monitor['name']}})
                page.wait_for_function('(edge)=>document.querySelector(".notch").dataset.edge===edge',arg=layout['edge'])
                page.wait_for_function('document.querySelector(".notch-shell").dataset.liquidStatus==="ready"')
                page.locator('.notch').dispatch_event('contextmenu')
                expect(page.locator('.detail-settings')).to_be_visible()
                heights=[]
                positions=[]
                for tab in ['appearance','models','general']:
                    page.locator('#tab-btn-'+tab).click()
                    geometry=page.locator('.notch-detail').evaluate('''e=>{
                      const r=e.getBoundingClientRect();return {height:r.height,left:r.left,top:r.top,right:r.right,bottom:r.bottom,w:innerWidth,h:innerHeight};
                    }''')
                    assert geometry['left']>=0 and geometry['top']>=0,geometry
                    assert geometry['right']<=geometry['w']+1 and geometry['bottom']<=geometry['h']+1,geometry
                    heights.append(geometry['height'])
                    positions.append(page.evaluate('async()=>window.__TAURI__.window.getCurrentWindow().innerPosition()'))
                assert max(heights)-min(heights)<=1,heights
                assert positions[0]==positions[1]==positions[2],positions
                checks.append({'monitor':monitor['name'],'scale':monitor['scale'],'requested_edge':edge,'actual_edge':layout['edge'],'settings_heights':heights})
        reset_notch(page)
        invoke('set_notch_placement',{'placement':original})
        for _ in range(10):
            page.reload()
            page.wait_for_function('document.querySelector(".notch-shell")?.dataset.liquidStatus==="ready"')
        before=resources()
        start=time.perf_counter()
        samples=[]
        while time.perf_counter()-start<args.seconds:
            sample=page.evaluate('''()=>({status:document.querySelector('.notch-shell').dataset.liquidStatus,
              canvases:document.querySelectorAll('canvas').length,nodes:document.getElementsByTagName('*').length,
              heap:performance.memory?.usedJSHeapSize})''')
            assert sample['status']=='ready',sample
            samples.append(sample)
            page.wait_for_timeout(5000)
        after=resources()
        assert not errors,errors
        result={'monitors':monitors,'checks':checks,'reloads':10,'observation_seconds':time.perf_counter()-start,
            'samples':samples,'processes_before':before,'processes_after':after,'errors':errors,
            'limits':'No OS sleep, monitor unplug, DPI setting mutation, or third-party screen-sharing session.'}
        (ROOT/'tmp/liquid-environment-results.json').write_text(json.dumps(result,indent=2))
        print(json.dumps({'checks':len(checks),'reloads':10,'seconds':result['observation_seconds'],'errors':errors}),flush=True)
    finally:
        reset_notch(page)
        invoke('set_notch_placement',{'placement':original})

"""Deterministic real-renderer cadence checks; native pixels are synthetic."""
import argparse
import json
from pathlib import Path
from playwright.sync_api import sync_playwright

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--port', type=int, default=1524)
args = parser.parse_args()
with sync_playwright() as p:
    browser = p.chromium.launch(channel='msedge', headless=True)
    page = browser.new_page(viewport={'width': 700, 'height': 600})
    errors = []
    page.on('pageerror', lambda error: errors.append(str(error)))
    page.route('**/src/main.ts', lambda route: route.fulfill(content_type='text/javascript', body=''))
    page.clock.install()
    page.goto(f'http://localhost:{args.port}')
    # Keep wall-clock time between Python/CDP calls out of cadence assertions.
    page.clock.pause_at(page.evaluate('Date.now()') + 1000)
    page.evaluate('''async()=>{
      document.body.innerHTML='<div class="notch-shell" data-theme="liquid"><nav class="notch" data-edge="right" style="position:absolute;left:320px;top:20px;width:64px;height:376px"><div class="notch-finish"></div></nav><section class="notch-detail" hidden></section></div>';
      window.reads=[];window.frames=0;window.changing=false;window.dirty=true;window.releases=0;
      const raf=window.requestAnimationFrame.bind(window);
      window.requestAnimationFrame=cb=>raf(t=>{window.frames++;cb(t)});
      window.__TAURI_INTERNALS__={invoke:async(command,args)=>{
        if(command==='release_liquid_backdrop'){window.releases++;return;}
        if(command!=='capture_liquid_backdrop')return;
        window.reads.push(performance.now());
        if(!window.changing&&!window.dirty)return new ArrayBuffer(0);
        window.dirty=false;
        const buffer=new ArrayBuffer(12+args.width*args.height*4),view=new DataView(buffer);
        new Uint8Array(buffer).set([76,81,48,49]);
        view.setUint32(4,args.width,true);view.setUint32(8,args.height,true);
        new Uint8Array(buffer,12).fill(window.reads.length%2?220:80);
        return buffer;
      }};
      const {mountLiquidGlass}=await import('/src/ui/liquid-glass.ts');
      window.renderer=mountLiquidGlass(document.querySelector('.notch-shell'),document.querySelector('.notch'),document.querySelector('.notch-detail'));
      window.renderer.setEnabled(true);
    }''')
    page.clock.run_for(1600)
    page.evaluate('window.reads=[];window.frames=0')
    page.clock.run_for(3000)
    idle = page.evaluate('({reads:reads.length,frames})')
    assert 2 <= idle['reads'] <= 3 and idle['frames'] <= 4, idle
    page.evaluate('window.reads=[];window.changing=true;window.renderer.refresh()')
    page.clock.run_for(1000)
    active = page.evaluate('reads.length')
    assert 25 <= active <= 32, active
    assert page.evaluate('releases') >= active
    # Hints arriving between normal capture deadlines must not double the cadence.
    page.evaluate('window.reads=[]')
    for _ in range(100):
        page.evaluate('window.renderer.refresh()')
        page.clock.run_for(10)
    hinted = page.evaluate('reads.length')
    assert hinted <= 33, hinted
    page.evaluate('window.changing=false')
    page.clock.run_for(1600)
    page.evaluate('window.reads=[];window.frames=0')
    page.clock.run_for(3000)
    settled = page.evaluate('({reads:reads.length,frames})')
    assert settled['reads'] <= 3 and settled['frames'] <= 4, settled
    # One hint just after a heartbeat must survive the cooldown without more events.
    page.evaluate('window.renderer.refresh()')
    page.clock.run_for(50)
    page.evaluate('window.reads=[];window.renderer.refresh()')
    page.clock.run_for(100)
    assert page.evaluate('reads.length') == 1
    before_drag_filter = page.locator('.notch .liquid-layer canvas').evaluate('e=>e.style.filter')
    page.evaluate('window.changing=true;document.querySelector(".notch").classList.add("is-dragging")')
    page.clock.run_for(100)
    page.evaluate('window.reads=[]')
    # Native position events can outnumber display frames; don't restart work or
    # regenerate canvas maps on each event.
    for _ in range(150):
        page.evaluate('window.renderer.relayout()')
        page.clock.run_for(10)
    dragging = page.evaluate('reads.length')
    assert 80 <= dragging <= 94, dragging
    assert page.locator('.notch .liquid-layer canvas').evaluate('e=>e.style.filter') == before_drag_filter
    page.evaluate('window.reads=[]')
    page.evaluate('document.querySelector(".notch").classList.remove("is-dragging")')
    page.clock.run_for(50)
    assert page.evaluate('reads.length') >= 1
    page.emulate_media(reduced_motion='reduce')
    page.evaluate('window.reads=[];window.changing=true')
    for _ in range(30):
        page.evaluate('window.renderer.refresh()')
        page.clock.run_for(33)
    reduced = page.evaluate('reads.length')
    assert reduced <= 5, reduced
    page.evaluate('window.renderer.destroy();window.reads=[]')
    page.clock.run_for(3000)
    assert page.evaluate('reads.length') == 0
    assert not errors, errors
    result = {'idle_3s': idle, 'changing_1s': active, 'dragging_1_5s': dragging, 'settled_3s': settled, 'reduced_1s': reduced, 'errors': errors}
    out = Path(__file__).resolve().parent.parent / 'tmp/liquid-scheduling-results.json'
    out.parent.mkdir(exist_ok=True)
    out.write_text(json.dumps(result, indent=2))
    print(json.dumps(result))
    browser.close()

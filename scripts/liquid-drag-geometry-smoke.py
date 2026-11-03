"""Verify live Liquid pixels keep the same scale through drag preparation/motion."""
import json
from pathlib import Path
from playwright.sync_api import sync_playwright

ROOT = Path(__file__).resolve().parent.parent
with sync_playwright() as p:
    browser = p.chromium.launch(channel='msedge', headless=True)
    page = browser.new_page(viewport={'width':800,'height':600})
    page.clock.install()
    page.clock.pause_at(page.evaluate('Date.now()')+1000)
    page.set_content('''<div class="notch-shell" data-theme="liquid" data-liquid-status="ready">
      <nav class="notch" data-edge="right" style="position:absolute;left:200px;top:100px;width:64px;height:300px">
        <div class="notch-surface"><div class="liquid-layer"><canvas width="64" height="300"></canvas></div></div>
      </nav></div>''')
    page.add_style_tag(path=str(ROOT/'src/styles/notch.css'))
    page.evaluate('''()=>{
      const ctx=document.querySelector('canvas').getContext('2d');
      for(let y=0;y<300;y++) {ctx.fillStyle=y%20<10?'#195cda':'#dc3020';ctx.fillRect(0,y,64,1);}
    }''')
    def geometry():
        return page.locator('.notch-surface').evaluate('''e=>{
          const r=e.getBoundingClientRect(),c=getComputedStyle(e);
          return {x:r.x,y:r.y,width:r.width,height:r.height,transform:c.transform};
        }''')
    resting = geometry()
    samples=[]
    for state in ['is-drag-armed','is-drag-armed is-dragging','is-dragging']:
        page.locator('.notch').evaluate('(e,state)=>e.className="notch "+state', state)
        for _ in range(24):
            page.clock.run_for(16)
            sample=geometry()
            assert all(abs(sample[k]-resting[k])<.01 for k in ['x','y','width','height']), (state,sample,resting)
            samples.append(sample)
    page.locator('.notch').evaluate('e=>e.className="notch is-snapped"')
    assert page.locator('.notch-surface').evaluate('e=>getComputedStyle(e).animationName')=='liquid-snap-x'
    # Grabbing again before the docking animation ends must not keep stretching
    # the live background under the new pointer gesture.
    page.locator('.notch').evaluate('e=>e.classList.add("is-drag-armed")')
    page.clock.run_for(16)
    assert page.locator('.notch-surface').evaluate('e=>getComputedStyle(e).animationName')=='none'
    assert all(abs(geometry()[k]-resting[k])<.01 for k in ['x','y','width','height'])
    result={'drag_geometry_samples':len(samples),'background_scale_preserved':True,'release_animation_preserved':True}
    (ROOT/'tmp/liquid-drag-geometry-result.json').write_text(json.dumps(result,indent=2))
    print(json.dumps(result))
    browser.close()

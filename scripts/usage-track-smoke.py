"""Pixel regression: activity sheen must not brighten or blur the empty quota track."""
import math
import os
from pathlib import Path
from PIL import Image, ImageChops
from playwright.sync_api import sync_playwright

base = os.environ.get("TOKENUSAGE_TEST_URL", "http://localhost:1420")
Path("tmp").mkdir(exist_ok=True)
with sync_playwright() as p:
    browser = p.chromium.launch(channel="msedge", headless=True)
    page = browser.new_page(viewport={"width": 340, "height": 200})
    page.route("**/usage-track-smoke", lambda r: r.fulfill(content_type="text/html", body='''
        <html><body style="background:#303030"><section class="notch-detail" style="left:30px;top:30px;width:260px;height:120px">
        <div class="detail-body"><div id="rows"></div></div></section></body></html>'''))
    page.goto(base + "/usage-track-smoke")
    page.evaluate("""async () => {
        await import('/src/styles/fonts.css'); await import('/src/styles/tokens.css');
        await import('/src/styles/app.css'); await import('/src/styles/notch.css');
        const {mountProviders}=await import('/src/ui/providers.ts');
        window.ui=mountProviders(document.querySelector('#rows'));
        window.publish=pct=>window.ui.setSnapshots([{provider_id:'codex',display_name:'Codex',
          status:'ok',source:'vendor',message:null,primary_used_percent:pct,as_of:'2026-10-18T00:00:00Z',
          windows:[{kind:'rolling_5h',label:'5h',used:pct,used_percent:pct,limit:100,unit:'percent',resets_at:null}]}]);
        window.publish(0);await document.fonts.ready;
    }""")
    page.add_style_tag(content="* { transition: none !important; }")
    page.screenshot(path="tmp/track-empty.png", animations="disabled")
    page.evaluate("window.publish(30)")
    box = page.locator(".track").bounding_box()
    fill = page.locator(".track-fill").bounding_box()
    page.screenshot(path="tmp/track-idle.png", animations="disabled")
    page.evaluate("window.ui.setActivities([{provider_id:'codex',state:'running'}])")
    page.screenshot(path="tmp/track-running.png", animations="disabled")
    crop = (math.ceil(fill["x"]+fill["width"])+2, math.floor(box["y"])-9,
            math.floor(box["x"]+box["width"])-3, math.ceil(box["y"]+box["height"])+9)
    empty, idle, running = [Image.open(f"tmp/track-{state}.png").convert("RGB").crop(crop)
                            for state in ["empty", "idle", "running"]]
    assert ImageChops.difference(idle, running).getbbox() is None, "Running activity adds a halo to the unfilled track"
    assert ImageChops.difference(empty, idle).getbbox() is None, "Fill glow bleeds beyond its actual width"
    browser.close()
    print("PASS: empty track pixels stay unchanged across zero/partial/running usage")

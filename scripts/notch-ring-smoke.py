"""Single-ring renderer regression; synthetic quota only, no native settings writes."""
import os
from pathlib import Path
from playwright.sync_api import sync_playwright

base = os.environ.get("TOKENUSAGE_TEST_URL", "http://localhost:1420")
Path("tmp").mkdir(exist_ok=True)
with sync_playwright() as p:
    browser = p.chromium.launch(channel="msedge", headless=True)
    page = browser.new_page(viewport={"width": 340, "height": 440}, reduced_motion="reduce")
    page.route("**/notch-ring-smoke", lambda r: r.fulfill(content_type="text/html", body='''
        <html><body style="background:#303030"><nav id="rail" class="notch" data-edge="right"
        style="left:220px;top:20px;width:64px;height:376px"></nav></body></html>'''))
    page.goto(base + "/notch-ring-smoke")
    page.evaluate("""async () => {
        await import('/src/styles/fonts.css');await import('/src/styles/tokens.css');
        await import('/src/styles/app.css');await import('/src/styles/notch.css');
        const {mountNotch}=await import('/src/ui/notch.ts');
        window.rail=mountNotch(document.querySelector('#rail'),{hover:()=>{},pin:()=>{}});
        window.publish=value=>window.rail.update([{provider_id:'codex',display_name:'Codex',
          primary_used_percent:value,primary_resets_at:null,status:'ok',source:'vendor',message:null,
          windows:[{kind:'rolling_5h',label:'5h',used:value??0,used_percent:value,limit:100,unit:'percent',resets_at:null}]}],['codex']);
        window.publish(50);window.rail.activity([{provider_id:'codex',state:'running'}]);
        await document.fonts.ready;
    }""")
    assert page.locator('[data-id=codex] .ring-fill').count() == 1
    assert page.locator('.ring-segments, mask[id^=ring-segments]').count() == 0
    assert page.locator('[data-id=codex] .activity-orbit').is_visible()
    for value in [None, 0, 25, 50, 100, 120]:
        page.evaluate('value=>window.publish(value)', value)
        fill = page.locator('[data-id=codex] .ring-fill')
        assert fill.get_attribute('visibility') == ('visible' if value is not None and value > 0 else 'hidden')
        expected = min(value or 0, 100)
        assert fill.evaluate('el=>el.style.strokeDasharray').startswith(str(expected))
    page.screenshot(path='tmp/single-notch-ring.png')
    browser.close()
    print('PASS: one continuous ring, no sub-ring, unknown/zero/partial/full/overage rendering')

"""Regression for quota detail sizing; Vite and Python Playwright required."""
from playwright.sync_api import sync_playwright

with sync_playwright() as p:
    browser = p.chromium.launch(channel="msedge", headless=True)
    page = browser.new_page(viewport={"width": 320, "height": 300}, reduced_motion="reduce")
    page.route("**/detail-layout", lambda r: r.fulfill(content_type="text/html", body='''
        <section class="notch-detail" style="--detail-radius:16px;top:16px;left:16px">
        <div class="detail-body"><div class="detail-quota"><div id="rows"></div>
        <p class="detail-state">Usage unavailable</p></div></div></section>'''))
    page.goto("http://localhost:1420/detail-layout")
    metrics = page.evaluate("""async () => {
        await import('/src/styles/fonts.css'); await import('/src/styles/tokens.css');
        await import('/src/styles/app.css'); await import('/src/styles/notch.css');
        const {mountProviders} = await import('/src/ui/providers.ts');
        const ui = mountProviders(document.querySelector('#rows'));
        const snap = {provider_id:'grok',display_name:'Grok',status:'degraded',source:'vendor',
            message:'Usage unavailable',windows:[{kind:'weekly',label:'Week',used:0,
            limit:100,unit:'percent',used_percent:null,resets_at:'2026-10-08T12:36:54Z'}],
            primary_used_percent:null};
        ui.setSnapshots([snap]);
        await document.fonts.ready;
        const body=document.querySelector('.detail-body'), detail=document.querySelector('.notch-detail');
        detail.style.height=Math.ceil(body.getBoundingClientRect().height+36)+'px';
        return {height:detail.clientHeight,scroll:detail.scrollHeight,width:detail.clientWidth,
            scrollWidth:detail.scrollWidth};
    }""")
    page.screenshot(path="tmp/grok-detail-fit.png")
    assert metrics["scroll"] <= metrics["height"], metrics
    assert metrics["scrollWidth"] <= metrics["width"], metrics
    browser.close()
    print("PASS: single-row Grok detail fits without vertical/horizontal scrolling")

"""Optional browser regression against Vite on port 1420; synthetic quota only."""
from pathlib import Path
from playwright.sync_api import sync_playwright

with sync_playwright() as p:
    browser = p.chromium.launch(channel="msedge", headless=True)
    page = browser.new_page(viewport={"width": 420, "height": 200}, device_scale_factor=2)
    errors = []
    page.on("pageerror", lambda error: errors.append(str(error)))
    page.route("**/grok-check", lambda route: route.fulfill(
        content_type="text/html",
        body='<html><body style="background:#181818;padding:24px"><div id="rows"></div></body></html>',
    ))
    page.goto("http://localhost:1420/grok-check")
    results = page.evaluate("""async () => {
        await import('/src/styles/fonts.css');
        await import('/src/styles/tokens.css');
        await import('/src/styles/app.css');
        const {mountProviders} = await import('/src/ui/providers.ts');
        const {headline} = await import('/src/ui/notch-state.ts');
        const ui = mountProviders(document.querySelector('#rows'));
        window.quotaUi = ui;
        const snap = {provider_id:'grok', display_name:'Grok', status:'degraded',
            source:'vendor', as_of:new Date().toISOString(), message:'Usage unavailable',
            primary_used_percent:null, primary_resets_at:'2026-10-08T12:36:54Z',
            windows:[{kind:'weekly',label:'Week',used:0,limit:100,unit:'percent',
                used_percent:null,resets_at:'2026-10-08T12:36:54Z'}]};
        const results = [];
        for (const value of [null, 0, 37, null]) {
            snap.status = value === null ? 'degraded' : 'ok';
            snap.primary_used_percent = value;
            snap.windows[0].used_percent = value;
            snap.windows[0].used = value ?? 0;
            ui.setSnapshots([snap]);
            results.push({value, headline:headline(snap),
                pct:document.querySelector('.provider-pct').textContent,
                period:document.querySelector('.window-label').textContent,
                reset:document.querySelector('.window-reset').textContent});
        }
        await document.fonts.ready;
        return results;
    }""")
    for result in results:
        value = result["value"]
        assert result["pct"] == (chr(0x2014) if value is None else f"{value}%"), result
        assert result["headline"]["text"] == result["pct"], result
        assert result["period"] == "Week" and "10/8" in result["reset"], result
        if value is None:
            assert result["headline"]["label"] == "No data", result
    assert not errors, errors
    Path("tmp").mkdir(exist_ok=True)
    page.screenshot(path="tmp/grok-missing-usage.png")
    page.evaluate("""() => {
        document.querySelector('#rows').style.width='246px';
        window.quotaUi.setSnapshots([{provider_id:'agy',display_name:'Antigravity',
            status:'unavailable',source:'unavailable',as_of:new Date().toISOString(),
            windows:[],message:'Reading Antigravity CLI quota',
            primary_used_percent:null,primary_resets_at:null}]);
    }""")
    assert page.locator('.usage-msg').inner_text() == 'Loading' + chr(0x2026)
    assert page.locator('.usage-msg').get_attribute('title') == 'Reading Antigravity CLI quota'
    assert page.locator('.usage-msg').evaluate('(e)=>e.scrollWidth<=e.clientWidth')
    page.screenshot(path="tmp/provider-status-summary.png")
    browser.close()
    print("PASS: unknown -> zero -> 37% -> unknown; headline/detail/reset preserved")

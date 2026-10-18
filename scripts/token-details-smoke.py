"""Codex token detail regression; requires Vite, Edge and Playwright. No live reads."""
import json
import os
from pathlib import Path
from playwright.sync_api import sync_playwright, expect

base = os.environ.get("TOKENUSAGE_TEST_URL", "http://localhost:1420")
Path("tmp").mkdir(exist_ok=True)
with sync_playwright() as p:
    browser = p.chromium.launch(channel="msedge", headless=True)
    page = browser.new_page(viewport={"width": 380, "height": 720}, reduced_motion="reduce")
    errors = []
    page.on("pageerror", lambda e: errors.append(str(e)))
    page.route("**/token-details-smoke", lambda r: r.fulfill(content_type="text/html", body='''
        <html><body style="background:#353535"><div class="notch-shell" data-theme="dark">
        <section class="notch-detail" style="--detail-radius:16px;left:36px;top:30px;width:260px;height:560px">
        <div class="detail-body"><div class="detail-quota"><div id="rows"></div></div></div>
        </section></div></body></html>'''))
    page.goto(base + "/token-details-smoke")
    page.evaluate("""async () => {
        await import('/src/styles/fonts.css'); await import('/src/styles/tokens.css');
        await import('/src/styles/app.css'); await import('/src/styles/notch.css');
        const {mountProviders} = await import('/src/ui/providers.ts');
        window.ui = mountProviders(document.querySelector('#rows'), () => window.resizeCalls = (window.resizeCalls || 0)+1);
        const counts = {input:1000,cache_read:750,cache_write:null,output:200,reasoning:50};
        const period = {models:[{model:'gpt-example',counts}],totals:counts};
        window.snap = {provider_id:'codex',display_name:'Codex',status:'ok',source:'vendor',message:null,
            as_of:'2026-10-18T00:00:00Z',primary_used_percent:40,primary_resets_at:null,
            windows:[{kind:'rolling_5h',label:'5h',used:40,limit:100,unit:'percent',used_percent:40,resets_at:null}],
            token_usage:{source:'local_records',as_of:'2026-10-18T00:00:00Z',status:'ready',today:period,week:period,month:period}};
        window.ui.setSnapshots([window.snap]); await document.fonts.ready;
    }""")
    assert page.locator(".provider-name img").count() == 1
    assert "invert(1)" in page.locator(".provider-name img").evaluate("el => getComputedStyle(el).filter")
    assert page.locator('[data-period]').count() == 3
    expect(page.locator('[data-period=week]')).to_have_text('이번 주')
    page.locator("summary").click()
    expect(page.locator("details")).to_have_attribute("open", "")
    expect(page.locator(".token-totals")).to_contain_text("1,200")
    expect(page.locator(".token-totals")).to_contain_text("75%")
    expect(page.locator(".token-metrics")).to_contain_text("CacheWrite—")
    month = page.locator('[data-period="month"]')
    month.click()
    assert page.locator('.token-periods').evaluate("el => getComputedStyle(el,'::before').transform") != 'none'
    month.press('ArrowLeft')
    expect(page.locator('[data-period=week]')).to_have_attribute('aria-selected', 'true')
    page.locator('[data-period=week]').press('End')
    expect(month).to_have_attribute('aria-selected', 'true')
    month.focus()
    page.evaluate("""() => {
        window.snap.token_usage.as_of='2026-10-18T00:00:05Z';
        window.ui.setSnapshots([structuredClone(window.snap)]);
    }""")
    expect(month).to_be_focused()
    expect(month).to_have_attribute("aria-selected", "true")
    page.evaluate("""() => {
        const other={...window.snap,provider_id:'agy',display_name:'Antigravity',token_usage:null,
          windows:[{...window.snap.windows[0],group:'Gemini'},{...window.snap.windows[0],group:'Claude'}]};
        window.ui.setSnapshots([other]);
    }""")
    assert page.locator(".quota-service-name:visible img").count() == 1
    assert page.locator(".provider-entry:visible .provider-name img").count() == 0
    page.evaluate("window.ui.setSnapshots([window.snap])")
    expect(month).to_have_attribute("aria-selected", "true")
    expect(page.locator("details:visible")).to_have_attribute("open", "")

    page.evaluate("""() => {
        const a={input:1000,cache_read:750,cache_write:0,output:200,reasoning:50};
        const b={input:100,cache_read:0,cache_write:0,output:20,reasoning:0};
        window.snap.token_usage.month={models:[{model:'gpt-6.1-sol',counts:a},{model:'gpt-6-astra',counts:b}],
          totals:{...a,input:1100,output:220}};
        window.ui.setSnapshots([structuredClone(window.snap)]);
    }""")
    expect(page.locator('.token-model-legend')).to_contain_text('6.1 Sol')
    expect(page.locator('.token-model-legend')).to_contain_text('6 Astra')
    assert page.locator('.token-model-bar > span').count() == 2
    legend = page.evaluate("""() => {
        const parts=[...document.querySelectorAll('.token-model-bar > span')];
        const dots=[...document.querySelectorAll('.token-model-dot')];
        const names=[...document.querySelectorAll('.token-legend-item')];
        return {colors:parts.map(el=>getComputedStyle(el).backgroundColor),dots:dots.map(el=>getComputedStyle(el).backgroundColor),
          ys:names.map(el=>el.getBoundingClientRect().y),barBottom:document.querySelector('.token-model-bar').getBoundingClientRect().bottom,
          legendY:document.querySelector('.token-model-legend').getBoundingClientRect().y};
    }""")
    assert legend['colors'] == legend['dots'], legend
    assert len(set(legend['ys'])) == 1 and legend['legendY'] > legend['barBottom'], legend
    page.locator('.token-model-legend').focus()
    page.evaluate("""() => {
        window.snap.token_usage.month.models[0].counts.input++;
        window.snap.token_usage.month.totals.input++;
        window.ui.setSnapshots([structuredClone(window.snap)]);
    }""")
    expect(page.locator('.token-model-legend')).to_be_focused()
    page.screenshot(path='tmp/token-model-shades.png')

    for state, label in [("loading", "집계 중"), ("partial", "일부 기록"), ("unavailable", "읽기 실패")]:
        page.evaluate("state => {window.snap.token_usage.status=state;window.ui.setSnapshots([structuredClone(window.snap)]);}", state)
        expect(page.locator(".token-status:visible")).to_have_text(label)
    page.evaluate("""() => {
        window.snap.token_usage.status='ready';
        const counts={input:0,cache_read:0,cache_write:0,output:0,reasoning:null};
        window.snap.token_usage.month={models:[{model:null,counts}],totals:counts};
        window.ui.setSnapshots([structuredClone(window.snap)]);
    }""")
    expect(page.locator(".token-model-heading")).to_contain_text("모델 미제공")
    expect(page.locator(".token-totals")).to_contain_text("0")
    expect(page.locator(".token-totals")).to_contain_text("—")
    page.evaluate("""() => {
        const counts={input:1000000,cache_read:800000,cache_write:500,output:100000,reasoning:100};
        window.snap.token_usage.status='partial';
        window.snap.token_usage.month={models:Array.from({length:7},(_,i)=>({model:i===0?'<img src=x onerror=alert(1)>':'gpt-very-long-model-name-'+i,counts})),
          totals:Object.fromEntries(Object.entries(counts).map(([key,value])=>[key,value*7]))};
        window.ui.setSnapshots([structuredClone(window.snap)]);
    }""")
    assert page.locator(".token-model img").count() == 0
    for theme in ["dark", "glass", "liquid"]:
        for scale in [1, 0.92]:
            page.evaluate("""({theme,scale}) => {
                const shell=document.querySelector('.notch-shell');shell.dataset.theme=theme;
                shell.style.setProperty('--notch-scale',scale);
                document.querySelector('.notch-detail').style.width=(260*scale)+'px';
            }""", {"theme": theme, "scale": scale})
            metrics = page.evaluate("""() => {
                const detail=document.querySelector('.notch-detail');
                return {width:detail.clientWidth,scroll:detail.scrollWidth,height:detail.clientHeight,content:detail.scrollHeight};
            }""")
            assert metrics["scroll"] <= metrics["width"], (theme, scale, metrics)
            page.screenshot(path=f"tmp/token-details-{theme}-{scale}.png")
    page.evaluate("window.ui.setSnapshots([{...window.snap,windows:[],token_usage:null,status:'auth_required'}])")
    assert page.locator(".provider-name:visible img").count() == 1
    assert page.locator(".token-details-root:visible").count() == 0
    for provider in ["claude", "grok"]:
        page.evaluate("id => window.ui.setSnapshots([{...window.snap,provider_id:id,display_name:id,token_usage:null}])", provider)
        assert page.locator(".provider-name:visible img").count() == 1
    assert page.evaluate("window.resizeCalls") > 0
    assert not errors, errors
    browser.close()
    print("PASS: Codex tokens, refresh/focus/state, provider icons, missing/zero/errors, escaping, themes and widths")

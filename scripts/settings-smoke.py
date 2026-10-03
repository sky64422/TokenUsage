"""Settings browser regression. Run with Vite on port 1420; no real OS writes."""
from pathlib import Path
from playwright.sync_api import sync_playwright, expect

with sync_playwright() as p:
    browser = p.chromium.launch(channel="msedge", headless=True)
    page = browser.new_page(viewport={"width": 340, "height": 420}, reduced_motion="reduce")
    page.route("**/settings-smoke", lambda r: r.fulfill(content_type="text/html", body='''
        <html><body style="background:#303030"><section class="notch-detail is-settings"
        style="--detail-radius:16px;left:24px;top:24px;width:260px;height:356px">
        <div class="detail-body"><div class="detail-settings"><div id="root" class="settings-root">
        </div></div></div></section></body></html>'''))
    page.goto("http://localhost:1420/settings-smoke")
    page.evaluate("""async () => {
        window.events = {}; window.callbacks = {}; window.callbackId = 0;
        window.updateResult = false;
        window.__TAURI_INTERNALS__ = {
            transformCallback: fn => {const id=++window.callbackId; window.callbacks[id]=fn;return id;},
            invoke: async (cmd,args) => {
                if(cmd==='plugin:event|listen') {window.events[args.event]=window.callbacks[args.handler];return 1;}
                if(cmd==='check_for_updates') {
                    if(window.updateError) throw window.updateError;
                    if(window.deferUpdate) return new Promise(resolve=>window.updateResolve=resolve);
                    return window.updateResult;
                }
            }
        };
        await import('/src/styles/fonts.css'); await import('/src/styles/tokens.css');
        await import('/src/styles/app.css'); await import('/src/styles/notch.css');
        const {mountSettingsPanel} = await import('/src/ui/settings-panel.ts');
        window.saves=0;
        const save=()=>{window.saves++;return new Promise((resolve,reject)=>{window.saveResolve=resolve;window.saveReject=reject;});};
        const st={opacity:0.675,autostart:false,hover_detail:false,always_show_notch:false,
            show_orbit:true,show_icon_glow:true,
            claude:{enabled:true},codex:{enabled:true},grok:{enabled:true},agy:{enabled:true}};
        const noop=()=>{};
        mountSettingsPanel(document.querySelector('#root'),st,{
            onAutostart:save,onHoverDetail:save,onAlwaysShowNotch:save,onShowOrbit:save,
            onShowIconGlow:save,onOpacityChange:noop,
            onProviderEnabled:save,onDiagnostics:noop,onQuit:noop},'0.3.3').show();
        await document.fonts.ready;
    }""")

    # A rejected save must roll back rather than leave an unsaved checked switch.
    page.locator('label[for="always-show"]').click()
    expect(page.locator('#always-show')).to_be_disabled()
    page.evaluate('window.saveReject(new Error("simulated write failure"))')
    expect(page.locator('#always-show')).not_to_be_checked()
    expect(page.locator('#always-show')).to_be_enabled()
    expect(page.locator('.settings-save-status').first).to_contain_text("이전 설정으로 복원됨")
    expect(page.locator('label[for="always-show"]')).to_have_attribute('data-save-state','restored')
    page.screenshot(path='tmp/settings-rollback.png',animations='disabled')
    page.locator('label[for="always-show"]').click()
    page.evaluate('window.saveResolve()')
    expect(page.locator('#always-show')).to_be_checked()
    expect(page.locator('#always-show')).to_be_enabled()

    # Header stays still when the inner panel scrolls to the final toggle.
    header_y = page.locator('.settings-header').bounding_box()['y']
    page.locator('label[for="show-icon-glow"]').evaluate('(e)=>e.scrollIntoView({block:"nearest"})')
    assert page.locator('.settings-header').bounding_box()['y'] == header_y
    assert page.locator('.settings-scroll').evaluate('(e)=>e.scrollTop') > 0
    page.locator('#tab-btn-appearance').focus()
    page.keyboard.press('ArrowRight')
    expect(page.locator('#tab-panel-appearance')).to_be_visible()
    expect(page.locator('#opacity-val')).to_have_text('50%')
    expect(page.locator('#show-period')).to_have_count(0)

    page.locator('#tab-btn-models').click()
    expect(page.locator('#tab-btn-models')).to_contain_text('서비스')
    expect(page.locator('[data-provider="grok"]')).to_contain_text('표시')
    page.locator('[data-provider="grok"]').click()
    expect(page.locator('[data-provider="codex"]')).to_be_disabled()
    page.evaluate('window.saveReject(new Error("simulated write failure"))')
    expect(page.locator('[data-provider="grok"]')).to_have_attribute('aria-pressed','true')
    for provider in ['claude','codex','grok']:
        page.locator(f'[data-provider="{provider}"]').click()
        page.evaluate('window.saveResolve()')
        expect(page.locator(f'[data-provider="{provider}"]')).to_be_enabled()
    expect(page.locator('[data-provider="agy"]')).to_be_disabled()
    expect(page.locator('.provider-selection-hint')).to_contain_text('최소 1개')

    page.locator('#tab-btn-general').click()
    expect(page.locator('#btn-check-update')).to_have_text('업데이트 확인')
    page.evaluate('window.deferUpdate=true')
    page.locator('#btn-check-update').click()
    expect(page.locator('#btn-check-update')).to_have_attribute('data-feedback','loading')
    expect(page.locator('#btn-check-update')).to_have_attribute('aria-busy','true')
    expect(page.locator('#btn-check-update')).to_be_disabled()
    assert page.locator('.update-dots i').first.evaluate('(e)=>getComputedStyle(e).animationName') == 'none'
    page.emulate_media(reduced_motion='no-preference')
    assert page.locator('.update-dots i').first.evaluate('(e)=>getComputedStyle(e).animationName') == 'update-dot-pulse'
    page.evaluate('document.getAnimations().filter(a=>a.effect.getTiming().iterations!==Infinity).forEach(a=>a.finish())')
    page.screenshot(path='tmp/settings-update-loading.png')
    page.evaluate('window.updateResolve(false)')
    expect(page.locator('#btn-check-update')).to_have_attribute('data-feedback','done')
    expect(page.locator('#btn-check-update')).to_be_enabled()
    page.screenshot(path='tmp/settings-update-done.png',animations='disabled')
    page.emulate_media(reduced_motion='reduce')
    page.evaluate('window.events["update-available"]({payload:{version:"0.3.4"}})')
    expect(page.locator('#btn-check-update')).to_be_disabled()
    page.evaluate('window.events["update-ready"]({payload:{version:"0.3.4"}})')
    expect(page.locator('#btn-check-update')).to_have_text('재시작하여 적용')
    expect(page.locator('#btn-check-update')).to_be_enabled()
    expect(page.locator('#btn-check-update')).to_have_attribute('data-feedback','done')
    page.evaluate('window.updateError="A long simulated update failure: "+"details ".repeat(20)')
    page.locator('#btn-check-update').click()
    expect(page.locator('#update-status')).to_contain_text('업데이트 실패')
    expect(page.locator('#btn-check-update')).to_have_attribute('data-feedback','error')
    expect(page.locator('#update-status')).to_have_attribute('title', 'A long simulated update failure: '+'details '*20)
    assert page.locator('#update-status').evaluate('(e)=>e.scrollWidth<=e.clientWidth')
    expect(page.locator('#btn-check-update')).to_have_text('재시작하여 적용')

    Path('tmp').mkdir(exist_ok=True)
    for tab in ['appearance','models','general']:
        page.locator('#tab-btn-'+tab).click()
        page.mouse.move(330,410)
        page.screenshot(path=f'tmp/settings-improved-{tab}.png',animations='disabled')
        assert page.locator('.notch-detail').evaluate('(e)=>e.scrollWidth<=e.clientWidth')
    browser.close()
    print('PASS: settings scroll, mouse tabs, save rollback/retry, provider lock, update states')

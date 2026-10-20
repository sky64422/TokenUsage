"""Generate high-resolution representative screenshots for README.md with latest UI/UX."""
from pathlib import Path
from playwright.sync_api import sync_playwright

OUT_DIR = Path("docs/assets/screenshots")
OUT_DIR.mkdir(parents=True, exist_ok=True)

with sync_playwright() as p:
    browser = p.chromium.launch(channel="msedge", headless=True)

    # ----------------------------------------------------
    # Page 1: Notch + Quota Details (Codex with models, AGY) + Token Details + Activity
    # ----------------------------------------------------
    page = browser.new_page(viewport={"width": 460, "height": 680}, device_scale_factor=2)
    page.route("**/screenshot-render", lambda r: r.fulfill(
        content_type="text/html",
        body="""<!DOCTYPE html>
        <html>
        <head>
            <meta charset="utf-8">
            <style>
                body {
                    margin: 0;
                    padding: 0;
                    background: #121214;
                    font-family: Pretendard, -apple-system, BlinkMacSystemFont, system-ui, Roboto, sans-serif;
                    overflow: hidden;
                    width: 100vw;
                    height: 100vh;
                }
                .desktop-bg {
                    position: absolute;
                    inset: 0;
                    background: radial-gradient(circle at 100% 50%, #222226 0%, #0d0d0f 100%);
                }
                .container {
                    position: absolute;
                    inset: 0;
                }
                .notch-shell {
                    position: absolute;
                    right: 0;
                    top: 0;
                    bottom: 0;
                    width: 100%;
                    height: 100%;
                }
                .notch-detail {
                    --detail-radius: 16px;
                }
            </style>
        </head>
        <body>
            <div class="desktop-bg"></div>
            <div class="container">
                <div class="notch-shell" data-theme="dark">
                    <nav class="notch" aria-label="AI usage"></nav>
                    <section class="notch-detail" aria-label="Usage detail" hidden>
                        <div class="detail-body">
                            <div class="detail-quota">
                                <div class="quota-root"></div>
                                <p class="detail-state"></p>
                            </div>
                            <div class="detail-settings" hidden><div class="settings-root"></div></div>
                        </div>
                    </section>
                </div>
            </div>
        </body>
        </html>"""
    ))

    page.goto("http://localhost:1420/screenshot-render")
    page.evaluate("""async () => {
        window.events = {}; window.callbacks = {}; window.callbackId = 0;
        window.__TAURI_INTERNALS__ = {
            transformCallback: fn => {const id=++window.callbackId; window.callbacks[id]=fn;return id;},
            invoke: async (cmd,args) => {
                if(cmd==='plugin:event|listen') {window.events[args.event]=window.callbacks[args.handler];return 1;}
                return null;
            }
        };

        await import('/src/styles/fonts.css');
        await import('/src/styles/tokens.css');
        await import('/src/styles/app.css');
        await import('/src/styles/notch.css');

        const { mountNotch } = await import('/src/ui/notch.ts');
        const { mountProviders } = await import('/src/ui/providers.ts');

        const rail = document.querySelector('.notch');
        const quotaRoot = document.querySelector('.quota-root');

        const notch = mountNotch(rail, {
            hover: () => {},
            pin: () => {}
        });

        const providers = mountProviders(quotaRoot);

        window.railController = notch;
        window.providersController = providers;

        const layout = {
            edge: 'right',
            scale: 1,
            anchor_x: 'right',
            anchor_y: 'center',
            window: { x: 0, y: 0, width: 460, height: 680 },
            notch: { x: 396, y: 70, width: 64, height: 376 },
            detail: { x: 120, y: 70, width: 260, height: 380 },
            metrics: {
                depth: 64,
                cell: 72,
                inset: 44,
                rest_depth: 10,
                rest_length: 80,
                shoulder: 32,
                inner_radius: 32
            }
        };

        notch.layout(layout);
        notch.reveal(true);

        const row = (kind, used, resets, label, group) => ({
            kind, used, limit: 100, unit: 'percent',
            resets_at: resets, used_percent: used, label, group
        });

        const codexCountsA = { input: 128400, cache_read: 98200, cache_write: 1200, output: 18400, reasoning: 6500 };
        const codexCountsB = { input: 32100, cache_read: 21000, cache_write: 400, output: 4800, reasoning: 1200 };
        const codexTotals = { input: 160500, cache_read: 119200, cache_write: 1600, output: 23200, reasoning: 7700 };
        const codexPeriod = {
            models: [
                { model: 'gpt-5.3-codex', counts: codexCountsA },
                { model: 'gpt-5-mini', counts: codexCountsB }
            ],
            totals: codexTotals
        };

        const snaps = [
            {
                provider_id: 'claude',
                display_name: 'Claude',
                windows: [
                    row('rolling_5h', 73, '2026-10-20T14:30:00Z', '5h'),
                    row('weekly', 12, '2026-10-25T12:00:00Z', 'Week')
                ],
                status: 'ok',
                source: 'vendor',
                as_of: '2026-10-20T11:00:00Z',
                message: null,
                primary_resets_at: '2026-10-20T14:30:00Z',
                primary_used_percent: 73
            },
            {
                provider_id: 'codex',
                display_name: 'Codex',
                windows: [
                    row('rolling_5h', 38, '2026-10-20T15:00:00Z', '5h'),
                    row('weekly', 54, '2026-10-26T00:00:00Z', 'Week')
                ],
                status: 'ok',
                source: 'vendor',
                as_of: '2026-10-20T11:00:00Z',
                message: null,
                primary_resets_at: '2026-10-20T15:00:00Z',
                primary_used_percent: 54,
                token_usage: {
                    source: 'local_records',
                    as_of: '2026-10-20T11:00:00Z',
                    status: 'ready',
                    today: codexPeriod,
                    month: codexPeriod,
                    windows: [
                        {
                            kind: 'rolling_5h',
                            resets_at: '2026-10-20T15:00:00Z',
                            label: '5h',
                            usage: codexPeriod
                        }
                    ]
                }
            },
            {
                provider_id: 'grok',
                display_name: 'Grok',
                windows: [
                    row('weekly', 45, '2026-10-23T00:00:00Z', 'Week')
                ],
                status: 'ok',
                source: 'vendor',
                as_of: '2026-10-20T11:00:00Z',
                message: null,
                primary_resets_at: '2026-10-23T00:00:00Z',
                primary_used_percent: 45
            },
            {
                provider_id: 'agy',
                display_name: 'Antigravity',
                windows: [
                    row('rolling_5h', 32, '2026-10-20T16:00:00Z', '5h', 'Gemini'),
                    row('weekly', 18, '2026-10-27T00:00:00Z', 'Week', 'Gemini'),
                    row('rolling_5h', 42, '2026-10-20T16:00:00Z', '5h', 'Claude'),
                    row('weekly', 25, '2026-10-27T00:00:00Z', 'Week', 'Claude')
                ],
                status: 'ok',
                source: 'vendor',
                as_of: '2026-10-20T11:00:00Z',
                message: null,
                primary_resets_at: '2026-10-20T16:00:00Z',
                primary_used_percent: 42
            }
        ];

        window.snaps = snaps;
        notch.update(snaps, ['claude', 'codex', 'grok', 'agy']);

        await document.fonts.ready;
    }""")

    # 1. Notch Overview only (Right edge silhouette)
    page.evaluate("""() => {
        document.querySelector('.notch-detail').hidden = true;
    }""")
    page.wait_for_timeout(300)
    page.screenshot(
        path=str(OUT_DIR / "notch-overview.png"),
        clip={"x": 360, "y": 50, "width": 100, "height": 420}
    )
    print("Generated notch-overview.png")

    # 2. Quota Detail - Codex (with Model Shading and closed token details toggle)
    page.evaluate("""() => {
        const detail = document.querySelector('.notch-detail');
        detail.hidden = false;
        detail.style.left = '120px';
        detail.style.top = '100px';
        detail.style.width = '260px';
        detail.style.height = 'auto';
        window.railController.select('codex', true);
        const snap = window.snaps.find(s => s.provider_id === 'codex');
        window.providersController.setSnapshots([snap]);
        const detailsEl = detail.querySelector('details.token-details');
        if (detailsEl) detailsEl.open = false;
    }""")
    page.wait_for_timeout(300)
    page.screenshot(
        path=str(OUT_DIR / "quota-detail-codex.png"),
        clip={"x": 100, "y": 60, "width": 360, "height": 380}
    )
    print("Generated quota-detail-codex.png")

    # 2b. Quota Detail - Liquid Theme (Main Hero Screenshot)
    page.evaluate("""() => {
        const shell = document.querySelector('.notch-shell');
        shell.dataset.theme = 'liquid';
        document.body.style.background = 'radial-gradient(circle at 85% 45%, #193555 0%, #0c1826 50%, #060b12 100%)';
    }""")
    page.wait_for_timeout(300)
    page.screenshot(
        path=str(OUT_DIR / "quota-detail-liquid.png"),
        clip={"x": 100, "y": 60, "width": 360, "height": 380}
    )
    print("Generated quota-detail-liquid.png")
    page.evaluate("""() => {
        const shell = document.querySelector('.notch-shell');
        shell.dataset.theme = 'dark';
        document.body.style.background = '#121214';
    }""")

    # 3. Quota Detail - Antigravity (Grouped)
    page.evaluate("""() => {
        const detail = document.querySelector('.notch-detail');
        detail.hidden = false;
        detail.style.left = '120px';
        detail.style.top = '70px';
        detail.style.width = '260px';
        detail.style.height = 'auto';
        window.railController.select('agy', true);
        const snap = window.snaps.find(s => s.provider_id === 'agy');
        window.providersController.setSnapshots([snap]);
    }""")
    page.wait_for_timeout(300)
    page.screenshot(
        path=str(OUT_DIR / "quota-detail-agy.png"),
        clip={"x": 100, "y": 40, "width": 360, "height": 420}
    )
    print("Generated quota-detail-agy.png")

    # 4. Token Details Expanded (Codex token drilldown with Today / Month tabs and model grid)
    page.evaluate("""() => {
        const detail = document.querySelector('.notch-detail');
        detail.hidden = false;
        detail.style.left = '120px';
        detail.style.top = '30px';
        detail.style.width = '260px';
        detail.style.height = 'auto';
        window.railController.select('codex', true);
        const snap = window.snaps.find(s => s.provider_id === 'codex');
        window.providersController.setSnapshots([snap]);
        const detailsEl = detail.querySelector('details.token-details');
        if (detailsEl) detailsEl.open = true;
    }""")
    page.wait_for_timeout(300)
    page.screenshot(
        path=str(OUT_DIR / "token-details-expanded.png"),
        clip={"x": 100, "y": 20, "width": 360, "height": 580}
    )
    print("Generated token-details-expanded.png")

    # 5. Live Activity (Orbit & Glow)
    page.evaluate("""() => {
        document.querySelector('.notch-detail').hidden = true;
        window.railController.select(null, false);
        window.railController.activity([
            { provider_id: 'codex', state: 'running', observed_at: new Date().toISOString() },
            { provider_id: 'claude', state: 'recent', observed_at: new Date().toISOString() },
            { provider_id: 'grok', state: 'idle', observed_at: new Date().toISOString() },
            { provider_id: 'agy', state: 'idle', observed_at: new Date().toISOString() }
        ]);
    }""")
    page.wait_for_timeout(300)
    page.screenshot(
        path=str(OUT_DIR / "live-activity.png"),
        clip={"x": 360, "y": 120, "width": 100, "height": 180}
    )
    print("Generated live-activity.png")

    page.close()

    # ----------------------------------------------------
    # Page 2: Settings Tabs (Appearance, Services, General) with Theme Picker & v0.5.1
    # ----------------------------------------------------
    page_settings = browser.new_page(viewport={"width": 340, "height": 520}, device_scale_factor=2)
    page_settings.route("**/settings-render", lambda r: r.fulfill(
        content_type="text/html",
        body="""<!DOCTYPE html>
        <html>
        <head>
            <meta charset="utf-8">
            <style>
                body {
                    margin: 0;
                    padding: 24px;
                    background: #121214;
                    font-family: Pretendard, -apple-system, BlinkMacSystemFont, system-ui, Roboto, sans-serif;
                }
                .notch-detail {
                    position: relative;
                    width: 260px;
                    height: 440px;
                    --detail-radius: 16px;
                }
            </style>
        </head>
        <body>
            <section class="notch-detail is-settings">
                <div class="detail-body">
                    <div class="detail-settings">
                        <div id="settings-root" class="settings-root"></div>
                    </div>
                </div>
            </section>
        </body>
        </html>"""
    ))

    page_settings.goto("http://localhost:1420/settings-render")
    page_settings.evaluate("""async () => {
        window.events = {}; window.callbacks = {}; window.callbackId = 0;
        window.__TAURI_INTERNALS__ = {
            transformCallback: fn => {const id=++window.callbackId; window.callbacks[id]=fn;return id;},
            invoke: async (cmd,args) => {
                if(cmd==='plugin:event|listen') {window.events[args.event]=window.callbacks[args.handler];return 1;}
                if(cmd==='check_for_updates') return false;
                return null;
            }
        };

        await import('/src/styles/fonts.css');
        await import('/src/styles/tokens.css');
        await import('/src/styles/app.css');
        await import('/src/styles/notch.css');

        const { mountSettingsPanel } = await import('/src/ui/settings-panel.ts');

        const root = document.querySelector('#settings-root');
        const st = {
            theme: 'dark',
            opacity: 0.65,
            autostart: true,
            hover_detail: false,
            always_show_notch: true,
            show_orbit: true,
            show_icon_glow: true,
            claude: { enabled: true },
            codex: { enabled: true },
            grok: { enabled: true },
            agy: { enabled: true }
        };

        const panel = mountSettingsPanel(root, st, {
            onTheme: () => Promise.resolve(),
            onAutostart: () => Promise.resolve(),
            onHoverDetail: () => Promise.resolve(),
            onAlwaysShowNotch: () => Promise.resolve(),
            onShowAnimation: () => Promise.resolve(),
            onOpacityChange: () => {},
            onProviderEnabled: () => Promise.resolve(),
            onDiagnostics: () => Promise.resolve(),
            onQuit: () => {}
        }, '0.5.1');

        panel.show();
        window.settingsPanel = panel;

        await document.fonts.ready;
    }""")

    # 6. Settings: Appearance Tab
    page_settings.locator('#tab-btn-appearance').click()
    page_settings.wait_for_timeout(300)
    page_settings.locator('.notch-detail').screenshot(
        path=str(OUT_DIR / "settings-tab-appearance.png")
    )
    print("Generated settings-tab-appearance.png")

    # 7. Settings: Services Tab
    page_settings.locator('#tab-btn-models').click()
    page_settings.wait_for_timeout(300)
    page_settings.locator('.notch-detail').screenshot(
        path=str(OUT_DIR / "settings-tab-services.png")
    )
    print("Generated settings-tab-services.png")

    # 8. Settings: General Tab
    page_settings.locator('#tab-btn-general').click()
    page_settings.wait_for_timeout(300)
    page_settings.locator('.notch-detail').screenshot(
        path=str(OUT_DIR / "settings-tab-general.png")
    )
    print("Generated settings-tab-general.png")

    page_settings.close()

    # ----------------------------------------------------
    # Page 3: Themes Comparison (Dark, Glass, Liquid)
    # ----------------------------------------------------
    page_themes = browser.new_page(viewport={"width": 840, "height": 320}, device_scale_factor=2)
    page_themes.route("**/themes-render", lambda r: r.fulfill(
        content_type="text/html",
        body="""<!DOCTYPE html>
        <html>
        <head>
            <meta charset="utf-8">
            <style>
                body {
                    margin: 0; padding: 20px;
                    background: radial-gradient(circle at 50% 50%, #1e2630 0%, #0d1117 100%);
                    font-family: Pretendard, sans-serif;
                    display: flex; gap: 20px; justify-content: center; align-items: center;
                }
                .theme-col { display: flex; flex-direction: column; align-items: center; gap: 8px; }
                .theme-col span { color: #8892b0; font-size: 13px; font-weight: 500; }
                .notch-shell { position: relative; width: 250px; height: 220px; }
                .notch-detail { position: relative !important; width: 250px !important; height: auto !important; left: 0 !important; top: 0 !important; }
            </style>
        </head>
        <body>
            <div class="theme-col"><span>다크 (Dark)</span><div class="notch-shell" data-theme="dark"><section class="notch-detail"><div class="detail-body"><div class="detail-quota"><div class="quota-root" id="q-dark"></div></div></div></section></div></div>
            <div class="theme-col"><span>글라스 (Glass)</span><div class="notch-shell" data-theme="glass"><section class="notch-detail"><div class="detail-body"><div class="detail-quota"><div class="quota-root" id="q-glass"></div></div></div></section></div></div>
            <div class="theme-col"><span>리퀴드 (Liquid)</span><div class="notch-shell" data-theme="liquid"><section class="notch-detail"><div class="detail-body"><div class="detail-quota"><div class="quota-root" id="q-liquid"></div></div></div></section></div></div>
        </body>
        </html>"""
    ))
    page_themes.goto("http://localhost:1420/themes-render")
    page_themes.evaluate("""async () => {
        await import('/src/styles/fonts.css');
        await import('/src/styles/tokens.css');
        await import('/src/styles/app.css');
        await import('/src/styles/notch.css');
        const { mountProviders } = await import('/src/ui/providers.ts');

        const row = (kind, used, resets, label) => ({
            kind, used, limit: 100, unit: 'percent',
            resets_at: resets, used_percent: used, label
        });
        const snap = {
            provider_id: 'codex', display_name: 'Codex',
            windows: [row('rolling_5h', 38, '2026-10-20T15:00:00Z', '5h'), row('weekly', 54, '2026-10-26T00:00:00Z', 'Week')],
            status: 'ok', source: 'vendor', as_of: '2026-10-20T11:00:00Z', message: null,
            primary_resets_at: '2026-10-20T15:00:00Z', primary_used_percent: 54
        };
        for (const id of ['dark', 'glass', 'liquid']) {
            const p = mountProviders(document.querySelector('#q-' + id));
            p.setSnapshots([snap]);
        }
        await document.fonts.ready;
    }""")
    page_themes.wait_for_timeout(300)
    page_themes.screenshot(path=str(OUT_DIR / "themes-comparison.png"))
    print("Generated themes-comparison.png")
    page_themes.close()

    browser.close()
    print("All screenshots generated successfully!")

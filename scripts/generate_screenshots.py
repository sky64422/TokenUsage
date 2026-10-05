"""Generate high-resolution representative screenshots for README.md."""
from pathlib import Path
from playwright.sync_api import sync_playwright

OUT_DIR = Path("docs/assets/screenshots")
OUT_DIR.mkdir(parents=True, exist_ok=True)

with sync_playwright() as p:
    browser = p.chromium.launch(channel="msedge", headless=True)

    # ----------------------------------------------------
    # Page 1: Notch + Quota Details (Codex, AGY) + Activity
    # ----------------------------------------------------
    page = browser.new_page(viewport={"width": 460, "height": 480}, device_scale_factor=2)
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
                <div class="notch-shell">
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
        const detail = document.querySelector('.notch-detail');
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
            window: { x: 0, y: 0, width: 460, height: 480 },
            notch: { x: 396, y: 52, width: 64, height: 376 },
            detail: { x: 120, y: 120, width: 260, height: 240 },
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

        const snaps = [
            {
                provider_id: 'claude',
                display_name: 'Claude',
                windows: [
                    row('rolling_5h', 73, '2026-10-05T14:30:00Z', '5h'),
                    row('weekly', 7, '2026-10-10T12:00:00Z', 'Week')
                ],
                status: 'ok',
                source: 'vendor',
                as_of: '2026-10-05T11:00:00Z',
                message: null,
                primary_resets_at: '2026-10-05T14:30:00Z',
                primary_used_percent: 73
            },
            {
                provider_id: 'codex',
                display_name: 'Codex',
                windows: [
                    row('rolling_5h', 21, '2026-10-05T15:00:00Z', '5h'),
                    row('weekly', 48, '2026-10-11T00:00:00Z', 'Week')
                ],
                status: 'ok',
                source: 'vendor',
                as_of: '2026-10-05T11:00:00Z',
                message: null,
                primary_resets_at: '2026-10-05T15:00:00Z',
                primary_used_percent: 48
            },
            {
                provider_id: 'grok',
                display_name: 'Grok',
                windows: [
                    row('weekly', 52, '2026-10-08T00:00:00Z', 'Week')
                ],
                status: 'ok',
                source: 'vendor',
                as_of: '2026-10-05T11:00:00Z',
                message: null,
                primary_resets_at: '2026-10-08T00:00:00Z',
                primary_used_percent: 52
            },
            {
                provider_id: 'agy',
                display_name: 'Antigravity',
                windows: [
                    row('rolling_5h', 38, '2026-10-05T16:00:00Z', '5h', 'Gemini'),
                    row('weekly', 15, '2026-10-12T00:00:00Z', 'Week', 'Gemini'),
                    row('rolling_5h', 45, '2026-10-05T16:00:00Z', '5h', 'Claude'),
                    row('weekly', 22, '2026-10-12T00:00:00Z', 'Week', 'Claude')
                ],
                status: 'ok',
                source: 'vendor',
                as_of: '2026-10-05T11:00:00Z',
                message: null,
                primary_resets_at: '2026-10-05T16:00:00Z',
                primary_used_percent: 45
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
        clip={"x": 360, "y": 30, "width": 100, "height": 420}
    )
    print("Generated notch-overview.png")

    # 2. Quota Detail - Codex
    page.evaluate("""() => {
        const detail = document.querySelector('.notch-detail');
        detail.hidden = false;
        detail.style.left = '120px';
        detail.style.top = '120px';
        detail.style.width = '260px';
        detail.style.height = '146px';
        window.railController.select('codex', true);
        const snap = window.snaps.find(s => s.provider_id === 'codex');
        window.providersController.setSnapshots([snap]);
    }""")
    page.wait_for_timeout(300)
    page.screenshot(
        path=str(OUT_DIR / "quota-detail-codex.png"),
        clip={"x": 100, "y": 40, "width": 360, "height": 400}
    )
    print("Generated quota-detail-codex.png")

    # 3. Quota Detail - Antigravity (Grouped)
    page.evaluate("""() => {
        const detail = document.querySelector('.notch-detail');
        detail.hidden = false;
        detail.style.left = '120px';
        detail.style.top = '70px';
        detail.style.width = '260px';
        detail.style.height = '236px';
        window.railController.select('agy', true);
        const snap = window.snaps.find(s => s.provider_id === 'agy');
        window.providersController.setSnapshots([snap]);
    }""")
    page.wait_for_timeout(300)
    page.screenshot(
        path=str(OUT_DIR / "quota-detail-agy.png"),
        clip={"x": 100, "y": 40, "width": 360, "height": 400}
    )
    print("Generated quota-detail-agy.png")

    # 4. Live Activity (Orbit & Glow)
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
        clip={"x": 360, "y": 100, "width": 100, "height": 180}
    )
    print("Generated live-activity.png")

    page.close()

    # ----------------------------------------------------
    # Page 2: Settings Tabs (Appearance, Services, General)
    # ----------------------------------------------------
    page_settings = browser.new_page(viewport={"width": 320, "height": 480}, device_scale_factor=2)
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
                    height: 400px;
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

        const { mountSettingsPanel, SETTINGS_PANEL_HEIGHT } = await import('/src/ui/settings-panel.ts');

        const root = document.querySelector('#settings-root');
        const st = {
            opacity: 0.50,
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
            onAutostart: () => Promise.resolve(),
            onHoverDetail: () => Promise.resolve(),
            onAlwaysShowNotch: () => Promise.resolve(),
            onShowAnimation: () => Promise.resolve(),
            onOpacityChange: () => {},
            onProviderEnabled: () => Promise.resolve(),
            onDiagnostics: () => Promise.resolve(),
            onQuit: () => {}
        }, '0.3.6');

        panel.show();
        window.settingsPanel = panel;

        await document.fonts.ready;
    }""")

    # 5. Settings: Appearance Tab
    page_settings.locator('#tab-btn-appearance').click()
    page_settings.wait_for_timeout(300)
    page_settings.locator('.notch-detail').screenshot(
        path=str(OUT_DIR / "settings-tab-appearance.png")
    )
    print("Generated settings-tab-appearance.png")

    # 6. Settings: Services Tab
    page_settings.locator('#tab-btn-models').click()
    page_settings.wait_for_timeout(300)
    page_settings.locator('.notch-detail').screenshot(
        path=str(OUT_DIR / "settings-tab-services.png")
    )
    print("Generated settings-tab-services.png")

    # 7. Settings: General Tab
    page_settings.locator('#tab-btn-general').click()
    page_settings.wait_for_timeout(300)
    page_settings.locator('.notch-detail').screenshot(
        path=str(OUT_DIR / "settings-tab-general.png")
    )
    print("Generated settings-tab-general.png")

    browser.close()
    print("All screenshots generated successfully!")

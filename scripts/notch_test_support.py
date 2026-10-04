"""Shared setup for isolated native smoke scripts; never changes production settings."""


def prepare_preview(page, *, always_show=True):
    assert page.evaluate('async()=>window.__TAURI__.app.getIdentifier()') == 'com.tokenusage.notch-preview'
    page.evaluate('''async alwaysShow => {
        const invoke = window.__TAURI__.core.invoke;
        await invoke('set_hover_detail', {enabled: true});
        await invoke('set_always_show_notch', {enabled: alwaysShow});
        await invoke('set_show_orbit', {enabled: true});
        await invoke('set_show_icon_glow', {enabled: true});
        for (const provider of ['claude', 'codex', 'grok', 'agy']) {
            await invoke('set_provider_enabled', {provider, enabled: true});
        }
    }''', always_show)
    page.reload()
    page.wait_for_selector('.notch-cell')
    page.wait_for_function('()=>document.querySelector(".notch").dataset.edge !== undefined')
    page.set_default_timeout(8000)


def reset_notch(page):
    # Escape closes settings first, then pinned detail. The old close button was removed.
    page.keyboard.press('Escape')
    page.keyboard.press('Escape')
    page.evaluate('()=>document.activeElement?.blur()')


def prepare_hover_fixture(page):
    prepare_preview(page)
    page.evaluate('''async () => {
        for (const provider of ['grok', 'agy']) {
            await window.__TAURI__.core.invoke('set_provider_enabled', {provider, enabled: false});
        }
    }''')
    page.reload()
    page.wait_for_selector('.notch-cell')
    page.wait_for_function('()=>document.querySelector(".notch").dataset.edge !== undefined')
    reset_notch(page)
    page.evaluate('''async () => {
        const row = (kind, used) => ({kind, used, limit: 100, unit: 'percent',
            resets_at: '2099-10-04T00:00:00Z', used_percent: used, label: kind === 'weekly' ? 'Week' : '5h'});
        const snapshots = ['claude', 'codex'].map((id, i) => ({
            provider_id: id, display_name: i ? 'Codex' : 'Claude',
            windows: i ? [row('rolling_5h', 23), row('weekly', 61)] : [row('rolling_5h', 37)],
            status: 'ok', source: 'vendor', as_of: '2026-10-04T00:00:00Z', message: null,
            primary_resets_at: '2099-10-04T00:00:00Z', primary_used_percent: i ? 61 : 37
        }));
        const publish = () => window.__TAURI__.event.emit('snapshots-updated', snapshots);
        await publish();
        // Keep synthetic quota visible across the app's independent five-second refresh.
        window.hoverFixtureTimer = setInterval(publish, 200);
    }''')

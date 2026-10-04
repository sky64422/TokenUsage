"""Frame-level native hover regression; run against a debug WebView2 CDP port.

Requires two enabled providers with different detail heights and hover enabled.
Does not change persisted preferences. Keep the pointer away during the check.
"""
import argparse
import json
from pathlib import Path
from playwright.sync_api import sync_playwright

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--port", type=int, default=9250)
parser.add_argument("--fixture", action="store_true", help="Use synthetic unequal-height quotas in the isolated preview only")
args = parser.parse_args()

with sync_playwright() as p:
    browser = p.chromium.connect_over_cdp(f"http://127.0.0.1:{args.port}")
    page = browser.contexts[0].pages[0]
    if args.fixture:
        from notch_test_support import prepare_hover_fixture
        import pyautogui
        pyautogui.moveTo(1000, 500)
        prepare_hover_fixture(page)
    assert page.evaluate("()=>Boolean(window.__TAURI__)")
    assert page.locator('.notch-cell[aria-pressed="true"]').count() == 0, "Unpin detail first"
    assert not page.locator('.detail-settings').is_visible(), "Close settings first"
    result = page.evaluate("""async () => {
        const root = document.querySelector('.notch');
        const cells = [...root.querySelectorAll('.notch-cell:not([hidden])')];
        if (cells.length < 2) throw new Error('Enable at least two providers');
        const frames = [], heights = new Set();
        const sample = () => {
            const r = root.getBoundingClientRect();
            frames.push([screenX, screenY, innerWidth, innerHeight, r.x, r.y, r.width, r.height]);
        };
        const frame = () => new Promise(resolve => requestAnimationFrame(() => { sample(); resolve(); }));
        // Settle initial layout before recording the switching sequence.
        cells[0].dispatchEvent(new PointerEvent('pointerenter'));
        for (let i = 0; i < 20; i++) await frame();
        frames.length = 0;
        for (let cycle = 0; cycle < 10; cycle++) {
            for (const cell of cells) {
                sample();
                cell.dispatchEvent(new PointerEvent('pointerenter'));
                for (let i = 0; i < 12; i++) await frame();
                if (cell.dataset.detailOpen !== 'true') throw new Error('Hover did not select ' + cell.dataset.id);
                heights.add(document.querySelector('.notch-detail').getBoundingClientRect().height);
            }
        }
        return { frames, heights: [...heights], providers: cells.map(c => c.dataset.id) };
    }""")
    if args.fixture:
        page.evaluate('()=>clearInterval(window.hoverFixtureTimer)')
    Path('tmp').mkdir(exist_ok=True)
    Path('tmp/notch-hover-stability.json').write_text(json.dumps(result), encoding='utf-8')
    assert len(result['heights']) > 1, "Need unequal card heights to reproduce the regression"
    baseline = result['frames'][0]
    changed = [f for f in result['frames'] if any(abs(a-b) > 0.01 for a, b in zip(f, baseline))]
    assert not changed, f"Native window/notch moved: baseline={baseline}, frames={changed[:5]}"
    page.screenshot(path='tmp/notch-hover-stability.png', omit_background=True)
    print(f"PASS: {len(result['frames'])} frames, providers={result['providers']}, heights={result['heights']}, stable={baseline}")

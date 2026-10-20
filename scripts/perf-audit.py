"""Comprehensive Performance, Frame Cadence, Resource, and Stability Audit for TokenUsage.

Tests:
1. Resource: JS Heap memory, DOM node count, Idle IPC zero-overhead, Bundle footprint
2. Frames: Animation frame times, FPS, Jank / Long Tasks (>33.3ms), Liquid canvas paint time
3. Stability: 50x stress toggle, 100x sparkline hover sweep, theme switches, error tracking
"""

from __future__ import annotations

import argparse
import gzip
import json
import os
import subprocess
import sys
import time
from pathlib import Path
from typing import Any, Dict, List

ROOT = Path(__file__).resolve().parent.parent
DIST_DIR = ROOT / "dist"
REPORT_PATH = ROOT / "perf-results.json"


def check_port(port: int) -> bool:
    import socket
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        s.settimeout(0.5)
        return s.connect_ex(("127.0.0.1", port)) == 0


def ensure_dev_server(port: int = 1420) -> subprocess.Popen | None:
    if check_port(port):
        print(f"[Vite] Reusing existing server on port {port}")
        return None

    print(f"[Vite] Starting dev server on port {port}...")
    proc = subprocess.Popen(
        ["npx", "vite", "--port", str(port), "--strictPort"],
        cwd=str(ROOT),
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        shell=True,
    )
    for _ in range(30):
        if check_port(port):
            print(f"[Vite] Dev server ready on port {port}")
            return proc
        time.sleep(0.3)
    raise RuntimeError(f"Failed to start Vite dev server on port {port}")


def audit_bundle_footprint() -> Dict[str, Any]:
    print("[Audit] 1. Analyzing Production Bundle Footprint...")
    if not DIST_DIR.exists():
        print("[Audit] dist/ not found, running npm run build...")
        subprocess.run(["npm", "run", "build"], cwd=str(ROOT), check=True, shell=True)

    assets: List[Dict[str, Any]] = []
    total_raw = 0
    total_gzip = 0

    for file_path in DIST_DIR.rglob("*"):
        if file_path.is_file():
            raw_size = file_path.stat().st_size
            with open(file_path, "rb") as f:
                gz_size = len(gzip.compress(f.read()))
            total_raw += raw_size
            total_gzip += gz_size
            assets.append({
                "name": file_path.relative_to(DIST_DIR).as_posix(),
                "raw_bytes": raw_size,
                "gzip_bytes": gz_size,
                "raw_kb": round(raw_size / 1024, 2),
                "gzip_kb": round(gz_size / 1024, 2),
            })

    # Sort largest first
    assets.sort(key=lambda x: x["raw_bytes"], reverse=True)

    return {
        "total_raw_kb": round(total_raw / 1024, 2),
        "total_gzip_kb": round(total_gzip / 1024, 2),
        "asset_count": len(assets),
        "top_assets": assets[:8],
    }


def run_browser_perf_audit(port: int = 1420) -> Dict[str, Any]:
    from playwright.sync_api import sync_playwright

    print("[Audit] 2. Running Browser Frame, Resource, and Stability Tests...")
    results: Dict[str, Any] = {}
    errors: List[str] = []

    with sync_playwright() as p:
        browser = p.chromium.launch(channel="msedge", headless=True)
        context = browser.new_context(
            viewport={"width": 420, "height": 720},
            device_scale_factor=2,
        )
        page = context.new_page()

        page.on("pageerror", lambda e: errors.append(f"PageError: {e}"))
        page.on("console", lambda msg: errors.append(f"ConsoleError: {msg.text}") if msg.type == "error" else None)

        # Route a comprehensive test fixture
        page.route("**/perf-audit-harness", lambda r: r.fulfill(
            content_type="text/html",
            body="""<!DOCTYPE html>
            <html>
            <head><meta charset="utf-8"></head>
            <body style="background:#111;margin:0;padding:20px;overflow:hidden">
              <div class="notch-shell" data-theme="liquid" style="position:relative;width:100%;height:100%">
                <nav class="notch" data-edge="top" style="position:absolute;left:40px;top:0;width:300px;height:64px">
                  <div class="notch-finish"></div>
                </nav>
                <section class="notch-detail" style="position:absolute;left:40px;top:74px;width:260px;height:380px;--detail-radius:16px">
                  <div class="detail-body">
                    <div id="rows"></div>
                  </div>
                </section>
              </div>
            </body>
            </html>"""
        ))

        page.goto(f"http://localhost:{port}/perf-audit-harness")

        # Initialize mock Tauri internals and real application modules
        page.evaluate("""async () => {
            await import('/src/styles/fonts.css');
            await import('/src/styles/tokens.css');
            await import('/src/styles/app.css');
            await import('/src/styles/notch.css');

            window.__ipc_calls = [];
            window.__TAURI_INTERNALS__ = {
                invoke: async (command, args) => {
                    window.__ipc_calls.push({ command, args, time: performance.now() });
                    if (command === 'begin_liquid_backdrop') return 101;
                    if (command === 'capture_liquid_backdrop') return new ArrayBuffer(0);
                    if (command === 'sync_liquid_backdrop') return;
                    if (command === 'release_liquid_backdrop') return;
                }
            };

            const { mountProviders } = await import('/src/ui/providers.ts');
            const { mountLiquidGlass } = await import('/src/ui/liquid-glass.ts');

            const shell = document.querySelector('.notch-shell');
            const notch = document.querySelector('.notch');
            const detail = document.querySelector('.notch-detail');

            window.liquid = mountLiquidGlass(shell, notch, detail);
            window.liquid.setEnabled(true);

            window.providersUi = mountProviders(document.querySelector('#rows'), () => {});

            // Generate synthetic 30-day token data
            const daily = [];
            const now = new Date(2026, 9, 20); // 2026-10-20
            for (let i = 29; i >= 0; i--) {
                const d = new Date(now);
                d.setDate(d.getDate() - i);
                const ds = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
                daily.push({
                    date: ds,
                    totals: { input: i * 35000, output: i * 5000, cache_read: i * 15000, cache_write: null, reasoning: null }
                });
            }

            const period = {
                models: [
                    { model: 'gpt-5', counts: { input: 1200000, output: 250000, cache_read: 850000, cache_write: null, reasoning: null } },
                    { model: 'claude-3.7-sonnet', counts: { input: 400000, output: 80000, cache_read: 200000, cache_write: null, reasoning: null } }
                ],
                totals: { input: 1600000, output: 330000, cache_read: 1050000, cache_write: null, reasoning: null }
            };

            window.sampleSnapshots = [
                {
                    provider_id: 'codex',
                    display_name: 'Codex',
                    status: 'ok',
                    source: 'vendor',
                    as_of: '2026-10-20T12:00:00Z',
                    primary_used_percent: 62,
                    primary_resets_at: '2026-10-21T00:00:00Z',
                    windows: [{ kind: 'rolling_5h', label: '5h', used: 62, limit: 100, unit: 'percent', used_percent: 62, resets_at: null }],
                    token_usage: {
                        source: 'local_records',
                        status: 'ready',
                        as_of: '2026-10-20T12:00:00Z',
                        today: period,
                        yesterday: period,
                        month: period,
                        daily
                    }
                }
            ];

            window.providersUi.setSnapshots(window.sampleSnapshots);
            await document.fonts.ready;
        }""")

        page.wait_for_selector(".token-details")

        # -------------------------------------------------------------
        # 1. Idle IPC Zero-Overhead Audit
        # -------------------------------------------------------------
        print("  - Testing Idle IPC Zero-Overhead...")
        time.sleep(0.3)
        initial_calls_count = page.evaluate("window.__ipc_calls.length")
        time.sleep(1.2)  # Idle for 1.2 seconds
        after_idle_calls = page.evaluate("(idx) => window.__ipc_calls.slice(idx)", initial_calls_count)
        # Any sync_liquid_backdrop during idle is an overhead bug
        idle_sync_calls = [c for c in after_idle_calls if c["command"] == "sync_liquid_backdrop"]

        # -------------------------------------------------------------
        # 2. Resource & Memory Audit (Baseline vs Stressed)
        # -------------------------------------------------------------
        print("  - Auditing Memory & DOM Footprint...")
        initial_dom_nodes = page.evaluate("document.querySelectorAll('*').length")
        initial_memory = page.evaluate("""() => {
            return window.performance?.memory ? {
                usedJSHeapSize: window.performance.memory.usedJSHeapSize,
                totalJSHeapSize: window.performance.memory.totalJSHeapSize,
            } : null;
        }""")

        # -------------------------------------------------------------
        # 3. Animation & Frame Cadence Measurement (FPS & Jank)
        # -------------------------------------------------------------
        print("  - Measuring Animation Cadence & Paint Times...")
        frame_metrics = page.evaluate("""async () => {
            return new Promise((resolve) => {
                const deltas = [];
                let lastTime = performance.now();
                let framesCount = 0;

                // Animate detail panel opacity and transform to trigger rendering & liquid sync
                const detail = document.querySelector('.notch-detail');
                const anim = detail.animate([
                    { transform: 'translateY(0px)', opacity: 1 },
                    { transform: 'translateY(12px)', opacity: 0.8 },
                    { transform: 'translateY(0px)', opacity: 1 }
                ], { duration: 600, iterations: 2 });

                function onFrame(now) {
                    const delta = now - lastTime;
                    deltas.push(delta);
                    lastTime = now;
                    framesCount++;
                    if (framesCount < 80) {
                        requestAnimationFrame(onFrame);
                    } else {
                        anim.cancel();
                        const sum = deltas.slice(1).reduce((a, b) => a + b, 0);
                        const avgDelta = sum / (deltas.length - 1);
                        const maxDelta = Math.max(...deltas.slice(1));
                        const jankFrames = deltas.slice(1).filter(d => d > 33.3).length; // >30fps dropped
                        const dropped60 = deltas.slice(1).filter(d => d > 19.0).length; // >60fps frame hitch
                        resolve({
                            total_frames: deltas.length - 1,
                            avg_delta_ms: Math.round(avgDelta * 100) / 100,
                            approx_fps: Math.round(1000 / avgDelta),
                            max_delta_ms: Math.round(maxDelta * 100) / 100,
                            jank_frames_count: jankFrames,
                            hitch_frames_count: dropped60
                        });
                    }
                }
                requestAnimationFrame(onFrame);
            });
        }""")

        # -------------------------------------------------------------
        # 4. Stress Interaction & Stability Audit (50x Accordion + 100x Hover)
        # -------------------------------------------------------------
        print("  - Running Stress Test (50x Accordion toggle + 100x Sparkline hover)...")
        stress_results = page.evaluate("""async () => {
            const summary = document.querySelector('.token-details summary');
            const details = document.querySelector('.token-details');
            const bars = document.querySelectorAll('.token-trend-day');

            const startTime = performance.now();

            // 50 toggles
            for (let i = 0; i < 50; i++) {
                summary.click();
            }

            // Ensure open
            if (!details.hasAttribute('open')) summary.click();

            // 100 hover events across 30 sparkline bars
            let hoverDispatches = 0;
            for (let cycle = 0; cycle < 4; cycle++) {
                for (let b = 0; b < bars.length; b++) {
                    bars[b].dispatchEvent(new MouseEvent('mouseenter', { bubbles: true }));
                    bars[b].dispatchEvent(new MouseEvent('mouseleave', { bubbles: true }));
                    hoverDispatches += 2;
                }
            }

            // Theme toggle stress (Liquid -> Glass -> Dark -> Liquid)
            const shell = document.querySelector('.notch-shell');
            for (const th of ['glass', 'dark', 'liquid', 'glass', 'liquid']) {
                shell.dataset.theme = th;
                window.liquid.setEnabled(th === 'liquid' || th === 'glass');
            }

            const durationMs = performance.now() - startTime;
            return {
                stress_duration_ms: Math.round(durationMs * 100) / 100,
                hover_events: hoverDispatches
            };
        }""")

        time.sleep(0.5)

        post_stress_dom_nodes = page.evaluate("document.querySelectorAll('*').length")
        post_stress_memory = page.evaluate("""() => {
            return window.performance?.memory ? {
                usedJSHeapSize: window.performance.memory.usedJSHeapSize,
                totalJSHeapSize: window.performance.memory.totalJSHeapSize,
            } : null;
        }""")

        # -------------------------------------------------------------
        # 5. Liquid Canvas Bounds & Offset Flicker Integrity
        # -------------------------------------------------------------
        print("  - Verifying 0ms Flicker & Liquid Canvas Offset Integrity...")
        liquid_flicker_check = page.evaluate("""() => {
            const notch = document.querySelector('.notch');
            const detail = document.querySelector('.notch-detail');
            const notchCanvas = document.querySelector('.notch .liquid-layer canvas');
            const detailCanvas = document.querySelector('.notch-detail .liquid-layer canvas');

            return {
                notch_has_canvas: !!notchCanvas,
                detail_has_canvas: !!detailCanvas,
                notch_width: notch.offsetWidth,
                notch_height: notch.offsetHeight,
                detail_width: detail.offsetWidth,
                detail_height: detail.offsetHeight,
                notch_canvas_w: notchCanvas ? notchCanvas.width : 0,
                notch_canvas_h: notchCanvas ? notchCanvas.height : 0,
            };
        }""")

        # -------------------------------------------------------------
        # 6. Verify Correct Unit & Label Rendering
        # -------------------------------------------------------------
        print("  - Verifying Unit Standards ('Tok', 'Hit', compact units)...")
        unit_check = page.evaluate("""() => {
            const overviewValues = Array.from(document.querySelectorAll('.token-overview td')).map(td => td.textContent);
            const firstBarTip = document.querySelector('.token-trend-day')?.dataset?.tip || '';
            const lastBarTip = document.querySelectorAll('.token-trend-day')[29]?.dataset?.tip || '';

            return {
                overview_values: overviewValues,
                first_bar_tip: firstBarTip,
                last_bar_tip: lastBarTip,
                has_capital_tok: overviewValues.every(v => v.includes('Tok') || v.includes('—')),
                has_capital_hit: overviewValues.every(v => v.includes('Hit') || v.includes('—')),
            };
        }""")

        browser.close()

    # Compile results
    results["bundle"] = audit_bundle_footprint()
    results["idle_ipc"] = {
        "idle_sync_calls": len(idle_sync_calls),
        "status": "PASS" if len(idle_sync_calls) == 0 else "FAIL"
    }
    results["memory"] = {
        "initial_dom_nodes": initial_dom_nodes,
        "post_stress_dom_nodes": post_stress_dom_nodes,
        "dom_leak_nodes": post_stress_dom_nodes - initial_dom_nodes,
        "initial_heap_mb": round(initial_memory["usedJSHeapSize"] / (1024 * 1024), 2) if initial_memory else "N/A",
        "post_stress_heap_mb": round(post_stress_memory["usedJSHeapSize"] / (1024 * 1024), 2) if post_stress_memory else "N/A",
        "heap_growth_mb": round((post_stress_memory["usedJSHeapSize"] - initial_memory["usedJSHeapSize"]) / (1024 * 1024), 2) if initial_memory and post_stress_memory else "N/A",
    }
    results["frames"] = frame_metrics
    results["stress"] = stress_results
    results["liquid_integrity"] = liquid_flicker_check
    results["unit_standards"] = unit_check
    results["errors"] = errors
    results["error_count"] = len(errors)

    return results


def run_rust_test_audit() -> Dict[str, Any]:
    print("[Audit] 3. Verifying Backend & Integration Test Metrics...")
    start = time.perf_counter()
    cmd = ["cargo", "test", "--lib"]
    res = subprocess.run(cmd, cwd=str(ROOT / "src-tauri"), capture_output=True, text=True)
    duration = time.perf_counter() - start

    passed = 0
    failed = 0
    for line in res.stdout.splitlines():
        if "test result:" in line:
            # e.g. test result: ok. 121 passed; 0 failed;
            import re
            m_pass = re.search(r"(\d+) passed", line)
            m_fail = re.search(r"(\d+) failed", line)
            if m_pass: passed = int(m_pass.group(1))
            if m_fail: failed = int(m_fail.group(1))

    return {
        "lib_tests_passed": passed,
        "lib_tests_failed": failed,
        "duration_sec": round(duration, 2),
        "status": "PASS" if failed == 0 and res.returncode == 0 else "FAIL"
    }


def main():
    parser = argparse.ArgumentParser(description="TokenUsage Comprehensive Performance & Stability Audit")
    parser.add_argument("--port", type=int, default=1420)
    args = parser.parse_args()

    dev_proc = None
    try:
        dev_proc = ensure_dev_server(args.port)
        browser_report = run_browser_perf_audit(args.port)
        rust_report = run_rust_test_audit()

        final_report = {
            "timestamp": time.strftime("%Y-%m-%d %H:%M:%S"),
            "rust_backend": rust_report,
            "frontend_metrics": browser_report,
        }

        with open(REPORT_PATH, "w", encoding="utf-8") as f:
            json.dump(final_report, f, indent=2, ensure_ascii=False)

        print("\n" + "=" * 60)
        print(" [TOKENUSAGE PERFORMANCE AUDIT SUMMARY] ")
        print("=" * 60)
        print(f"1. Bundle Footprint: {browser_report['bundle']['total_raw_kb']} KB raw ({browser_report['bundle']['total_gzip_kb']} KB gzip)")
        print(f"2. Idle IPC Overhead: {browser_report['idle_ipc']['idle_sync_calls']} sync calls in 1.2s -> {browser_report['idle_ipc']['status']}")
        print(f"3. Frame Rate & Cadence: ~{browser_report['frames']['approx_fps']} FPS (avg delta: {browser_report['frames']['avg_delta_ms']}ms, jank frames: {browser_report['frames']['jank_frames_count']})")
        print(f"4. Stress & Leak Stability: DOM leak = {browser_report['memory']['dom_leak_nodes']} nodes, JS heap growth = {browser_report['memory']['heap_growth_mb']} MB")
        print(f"5. Runtime Errors: {browser_report['error_count']} unhandled errors")
        print(f"6. Rust Backend Tests: {rust_report['lib_tests_passed']} passed, {rust_report['lib_tests_failed']} failed ({rust_report['duration_sec']}s)")
        print(f"7. Unit Standards Check: Tok={browser_report['unit_standards']['has_capital_tok']}, Hit={browser_report['unit_standards']['has_capital_hit']}")
        print("=" * 60)
        print(f"Detailed JSON results written to: {REPORT_PATH}")

    finally:
        if dev_proc:
            print("[Vite] Terminating dev server process...")
            dev_proc.terminate()


if __name__ == "__main__":
    main()

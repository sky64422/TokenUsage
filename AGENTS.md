# Agent instructions (TokenUsage)

공통 규칙: Rules clone의 `ENGINEERING.md` / `RELEASE.md` (Windows `C:\dev\Rules`, WSL `/mnt/c/dev/Rules`). 이 파일은 **이 제품만**. 충돌하면 여기가 이긴다.

새 세션이면 이 순서로 연다:

1. **Code map / product shape:** [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)
2. **Product brief (Operate mode):** [`PRODUCT.md`](PRODUCT.md)
3. **Visual system / UI contracts:** [`DESIGN.md`](DESIGN.md) — [ui.shadcn.com](https://ui.shadcn.com), [impeccable.style](https://impeccable.style)
4. **On Windows:** [`docs/windows-dev.md`](docs/windows-dev.md)
5. **Releases / updater:** [`docs/release.md`](docs/release.md)
6. **Tests:** [`docs/testing.md`](docs/testing.md)
7. **Providers & Quotas:** [`docs/providers.md`](docs/providers.md)

## Product constraints

- Screen-edge **usage monitor notch**, not a billing dashboard or team admin console.
- **Primary data:** vendor quota: direct OAuth (Claude / Codex / Grok) and official CLI (Antigravity). No tokscale, no local JSONL estimates.
- **No** browser scraping of vendor dashboards without an explicit design decision.
- **Notifications** are out of scope until requested.
- **Antigravity (AGY):** official installed `agy --sandbox --print-timeout 30s --print /usage` only (added by user request 2026-10-02). Cached to disk (`quota-cache/agy_snapshot.json`) for 0ms cold start; background CLI completion notifies immediately via `poll::notify_refresh()`. Gemini and Claude/GPT 5h/week quotas remain separate. No legacy cloudcode-pa API, browser scraping, or token estimates.
- **Window shape & geometry:** 64 DIP depth, 72 DIP provider cells, two tangent 32 DIP circular arcs, 44 DIP content insets, 40 DIP rings (12 DIP lateral margins). Custom concave SVG notch; `DWMWCP_DONOTROUND`, transparent HWND and no native shadow. Do not reintroduce the old 240px minimum or content-hug resize loop. Rust owns physical placement; invisible areas pass input to other apps.
- **Placement:** four physical edges; bottom explicitly overlays the taskbar; other taskbar-blocked edges fall back visibly. Store monitor hint/normalized offset; drag previews do not write settings. Details open inward; hover must not activate the window.
- **Detail layout:** inside the detail keep **fixed column geometry** — row 1 name · period · refill; row 2 `1fr` bar · `2.9em` % (2px gutter). Dual/multi-group rows use compact `--usage-row-gap: 8px`.
- **Progress:** rings for the summary. Details retain Quiet Luxury 6px bars. Missing/auth-required values must not look like 0%. Preserve the backend primary percentage (currently max of windows) and its matching period.
- **Refresh:** interval is **fixed at 5s**. Do not add a settings control; `refresh_secs()` ignores persisted values.
- **Grok:** map primary period credit only; do **not** surface `productUsage` product rows (GrokBuild / GrokChat). Omitted % with period present stays unknown/degraded (does not claim 0%).
- **Settings UI:** 3-tab layout (`모양`, `서비스`, `일반`) with fixed header and tab navigation while the body scrolls. Standardized Korean terms: `불투명도` (opacity), `노치 항상 표시`, `마우스 올릴 때 상세 열기`, `작업 중 강조 효과`, `로그 복사`, `종료`. Keep item titles concise and strictly single-line.
- **Opacity slider:** neutral chrome + small off-white thumb — not accent/cyan. Preserve readability floors in `applyPanelOpacity` + token `max(...)` alphas; do not let glass wipe out meta/reset text.
- **Autostart:** never call OS `enable` from debug/`tauri dev` (see `sync_os_autostart`).
- Prefer thin `commands.rs`; put logic in `application` / `domain` / provider adapters.
- Keep `tmp/updater.key` **out of git** (signing private key).

## Verify before claiming done

```text
npm test
npm run build
# optional: npm run test:coverage   (needs cargo-tarpaulin; bash)
# UI: npm run tauri dev  (Windows preferred)
```

Default branch: **`main`**.

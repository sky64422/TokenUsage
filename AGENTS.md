# Agent instructions (TokenUsage)

If you are an automated coding agent in a new session:

1. **Code map / product shape:** [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)
2. **Product brief (Operate mode):** [`PRODUCT.md`](PRODUCT.md)
3. **Visual system / UI contracts:** [`DESIGN.md`](DESIGN.md) — external refs: [ui.shadcn.com](https://ui.shadcn.com), [impeccable.style](https://impeccable.style)
4. **On Windows:** [`docs/windows-dev.md`](docs/windows-dev.md)
5. **Releases / updater:** [`docs/release.md`](docs/release.md)
6. **Tests:** [`docs/testing.md`](docs/testing.md)

## Product constraints

- Floating **usage monitor widget**, not a billing dashboard or team admin console.
- **Primary data:** direct vendor OAuth quota (Claude / Codex / Grok) only. No tokscale, no local JSONL estimates.
- **No** browser scraping of vendor dashboards without an explicit design decision.
- **Notifications** are out of scope until requested.
- **Antigravity (AGY)** deferred in-app.
- UI: keep **fixed column geometry** — row 1 name · period · refill; row 2 `1fr` bar · `2.9em` % (2px bar→% gutter). Dual vs single layouts may differ.
- **Progress:** Quiet Luxury 6px pill bars (glow / sheen / end-cap) — sheen & critical breathe only on **live** fills (`.is-active`); prefer glanceable bars over experimental gauges unless explicitly requested.
- **Refresh:** interval is **fixed at 5s**. Do not add a settings control; `refresh_secs()` ignores persisted values.
- **Panel radius:** `--radius` 8px to match Win11 `DWMWCP_ROUND`; panel `height: 100%` so CSS does not draw a square frame inside the DWM clip.
- **Opacity slider:** neutral chrome + small off-white thumb — not accent/cyan.
- **Opacity:** preserve readability floors in `applyPanelOpacity` + token `max(...)` alphas; do not let glass wipe out meta/reset text.
- **Autostart:** never call OS `enable` from debug/`tauri dev` (see `sync_os_autostart`).
- **Grok:** map primary period credit only; do **not** surface `productUsage` product rows (GrokBuild / GrokChat).
- Prefer thin `commands.rs`; put logic in `application` / `domain` / provider adapters.
- Keep `tmp/updater.key` **out of git** (signing private key).

## Verify before claiming done

```text
npm test
npm run build
# optional: npm run test:coverage   (needs cargo-tarpaulin; bash)
# UI: npm run tauri dev  (Windows preferred)
```

Default branch: **`main`**. See also [`docs/testing.md`](docs/testing.md).

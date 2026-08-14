# Windows development (TokenUsage)

**Updated:** 2026-08-15  
Companion to [ARCHITECTURE.md](./ARCHITECTURE.md), [PRODUCT.md](../PRODUCT.md), [DESIGN.md](../DESIGN.md).

## Prerequisites

1. **MSVC Build Tools** — Desktop development with C++  
2. **Rust** — `stable-x86_64-pc-windows-msvc`  
3. **Node.js** 18+  
4. **WebView2** (usually present on Windows 11)  
5. Optional: Claude / Codex / Grok CLI logged in so direct vendor quota works  

## Daily commands

```powershell
cd C:\dev\TokenUsage
npm install
npm run tauri dev
npm test
npm run build
```

## Autostart note

- Settings **Launch at login** is stored always. Refresh interval is fixed (5s) — no in-app control.
- OS `HKCU\...\Run` is updated **only by release builds** so `npm run tauri dev` does not point boot at `target\debug\token-usage.exe` (no Vite → blank UI).
- To fix a machine that already has a bad Run key: run the **installed** app once with autostart on, or set Run to `%LocalAppData%\TokenUsage\token-usage.exe`.

## Updater signing (local)

```powershell
# key already at tmp/updater.key (gitignored)
$env:TAURI_SIGNING_PRIVATE_KEY_PATH = "C:\dev\TokenUsage\tmp\updater.key"
npm run release:publish -- --dry-run
```

## Notes

- Transparent + always-on-top chrome behaves best on real Windows (not WSL GUI).
- Hotkey and autostart need a packaged/dev Tauri process, not plain `vite` alone.
- Design tokens: `src/styles/tokens.css`; UI primitives: `src/styles/app.css`.

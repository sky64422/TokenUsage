# Windows development (TokenUsage)

**Updated:** 2026-10-05  
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

- Settings **Windows 시작 시 실행** (Autostart under `일반` tab) is stored always. Refresh interval is fixed (5s) — no in-app control.
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
- Autostart and native window behaviors need a packaged/dev Tauri process, not plain `vite` alone.
- Design tokens: `src/styles/tokens.css`; Notch chrome: `src/styles/notch.css`; UI primitives: `src/styles/app.css`.

## Background quota reads

Quota-reader AGY processes receive `AGY_CLI_DISABLE_AUTO_UPDATE=true` in their own
Unicode environment block. Usage reads must not launch the CLI's self-updater.
The widget does not change the user/system environment or interactive AGY sessions.

## Isolated notch smoke test

Use a separate identifier to avoid touching installed preferences. With the Vite dev server already running, create `tmp/notch-preview.json`:

```json
{"identifier":"com.tokenusage.notch-preview","build":{"beforeDevCommand":""}}
```

```powershell
$env:TOKENUSAGE_SKIP_DIRECT_QUOTA = '1'
$env:WEBVIEW2_USER_DATA_FOLDER = "$PWD/tmp/notch-webview"
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = '--remote-debugging-port=9223'
npm run tauri dev -- --no-watch --config tmp/notch-preview.json
# In another terminal; Python Playwright, Pillow, pyautogui required for this optional smoke:
python -X utf8 scripts/notch-smoke.py
python -X utf8 scripts/notch-regression.py
python -X utf8 scripts/notch-edge-drag.py
python -X utf8 scripts/notch-seam-drag.py
python -X utf8 scripts/notch-polish.py
python -X utf8 scripts/notch-hover-stability.py --port 9223 --fixture
```

These smoke scripts refuse the production identifier. They use synthetic quota data,
move the pointer and exercise notch interactions. Close the preview before Rust rebuilds
(Windows locks its running executable). Remove the three environment variables before
normal development. Actual vendor data is not validated by these fixture runs.

For manual theme review with actual usage, keep the separate identifier but omit
`TOKENUSAGE_SKIP_DIRECT_QUOTA`. Use the normal backend quota stream without injected
snapshot timers. A fixture timer racing the five-second backend refresh alternates
sample percentages with unavailable values; reload or restart after fixture checks
before handing the preview to a user.

Select a theme in settings to compare Dark, Glass and Liquid. Selection survives
reload/restart. Glass/Liquid use Windows 11 host-backdrop composition and native
Gaussian blur behind the WebView, clipped to the visible notch/detail silhouettes.
The widget stays included in normal Windows screenshots and screen sharing; no
`WDA_EXCLUDEFROMCAPTURE` or desktop readback is used for these materials. Glass's
former CPU refraction lens is replaced by native blur. Unsupported composition
reports an error and keeps a readable tint fallback.

For native capture regression, run `python scripts/liquid-capture-smoke.py` against
the isolated `com.tokenusage.notch-preview` on CDP 9223 with Glass/Liquid selected.
It checks a running material, display affinity 0 and saves an actual OS screenshot
for inspection. Renderer/CDP screenshots alone cannot detect capture exclusion.

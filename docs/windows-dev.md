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
python -X utf8 scripts/native-quality-smoke.py --port 9223
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
reload/restart. The current Glass/Liquid renderer captures desktop pixels into
canvas surfaces and temporarily changes window capture affinity while a capture
is in flight. Browser fixtures cannot validate actual desktop capture or screen
sharing behavior. The older `liquid-capture-smoke.py` assumes a different native
material implementation and is not a valid current screenshot-compatibility gate.


### Capture diagnostics

Per-frame capture file logging is off by default. To diagnose capture/affinity timing,
set `$env:TOKENUSAGE_CAPTURE_DEBUG = '1'` before launching the process. It writes the
bounded `%LOCALAPPDATA%/TokenUsage/logs/capture-debug.log`. Restart without this variable
for normal performance measurements. The opt-in is sampled once per process.

### WGC capture (default since v0.5.18)

Debug and release builds use monitor WGC with per-session notch exclusion, keeping
window-wide capture affinity unchanged. The worker requests official borderless
access once per process and sets `IsBorderRequired(false)` only after `Allowed`.
Windows may ask for consent. Unsupported interfaces, denial or capture errors
leave the base material visible; there is no automatic GDI fallback or OS policy override.

For automated tests, use the isolated identifier above. No WGC flags are required.
For legacy GDI comparisons only, set `$env:TOKENUSAGE_WGC_PREVIEW = '0'` in a debug
build. `TOKENUSAGE_WGC_BORDERLESS_PREVIEW=0` keeps the OS border for debug diagnosis.
Release builds ignore both overrides. Remove them and restart to restore defaults.

After launch, run sequentially:

```powershell
python -X utf8 scripts/liquid-events-native-smoke.py --screenshots --latency-samples 20 --screen-samples 100 --require-visible
python -X utf8 scripts/liquid-environment-smoke.py --seconds 15
python -X utf8 scripts/native-quality-smoke.py --port 9223
```

Borderless access and screenshot visibility were verified on the development
machine. Real denied-policy devices, mixed DPI/HDR, sleep/device loss and complete
OBS/Teams/Zoom coverage remain outside that evidence. See
[the detailed validation report](liquid-wgc-deep-validation.md).

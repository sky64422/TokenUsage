# Windows development (TokenUsage)

Companion to [HANDOFF.md](./HANDOFF.md) and EconomyWarRoom’s windows-dev guide.

## Prerequisites

1. **MSVC Build Tools** — Desktop development with C++  
2. **Rust** — `stable-x86_64-pc-windows-msvc`  
3. **Node.js** 18+  
4. **WebView2** (usually present on Windows 11)  
5. Optional: **tokscale** (`npm i -g tokscale`) for vendor quota primary path  

## Daily commands

```powershell
cd C:\dev\TokenUsage
npm install
npm run tauri dev
npm test
npm run build
```

## Updater signing (local)

```powershell
# key already at tmp/updater.key (gitignored)
$env:TAURI_SIGNING_PRIVATE_KEY_PATH = "C:\dev\TokenUsage\tmp\updater.key"
npm run release:publish -- --dry-run
```

## Notes

- Transparent + always-on-top chrome behaves best on real Windows (not WSL GUI).
- Hotkey and autostart need a packaged/dev Tauri process, not plain `vite` alone.

# Release & in-app updates

**Updated:** 2026-08-15  
**Current public tag:** v0.1.31  

**Audience:** maintainers publishing Windows builds that clients can install **and** self-update.  
**Product:** TokenUsage (`com.tokenusage.app`)

---

## Recent releases

| Tag | Highlights |
|-----|------------|
| **v0.1.31** | Internals: drop JSONL/limits APIs; parallel vendor fetch; non-blocking boot; short window labels; vitest for format helpers |
| v0.1.30 | Settings: fixed 5s refresh + app version; DWM-matched corners; quiet opacity slider; right-aligned period labels |
| v0.1.29 | Smaller WEEK/30D + refill type; hide vendor/auth/free chips; label hugs track |
| v0.1.28 | Codex: refresh expired ChatGPT OAuth tokens; Grok: hint when unified billing omits weekly % |
| v0.1.27 | Opacity: type/status colors track glass (softer floors); shadcn/Impeccable refs in DESIGN |
| v0.1.26 | UI polish: semantic tokens, focus rings, low-opacity contrast floors, 11px meta, live-only track motion, settings copy distill; PRODUCT/DESIGN docs |
| v0.1.25 | Autostart: never register `tauri dev` / debug exe as Windows login item |
| v0.1.24 | Opacity meter ticks; denser refresh chips |

---

## What “publish” means

In-app **Check for updates** (header **↻**) does **not** read git `main`.  
It downloads:

```text
https://github.com/sky64422/TokenUsage/releases/latest/download/latest.json
```

That file must list a **higher semver** than the installed app, a signed installer URL, and a matching signature.

| Step | Purpose |
|------|---------|
| Bump version | `package.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` (+ `Cargo.lock` package version) |
| Signed `tauri build` | Produces NSIS/MSI + `.sig` (`createUpdaterArtifacts`) |
| GitHub Release | Hosts installer + **`latest.json`** as release assets |
| Users on prior release builds | Header ↻ / startup check installs the new package |

`npm run tauri dev` **skips** startup auto-check (`debug_assertions`). Prefer a **release** install when testing updates.

---

## One-time: signing keys

Already generated for this repo (local only):

```text
tmp/updater.key      — private (gitignored under tmp/)
tmp/updater.key.pub  — public (also embedded in tauri.conf.json)
```

Regenerate if needed:

```powershell
npx tauri signer generate -w tmp/updater.key --ci -f
```

Put the new public key into `src-tauri/tauri.conf.json` → `plugins.updater.pubkey`.

| Variable | Meaning |
|----------|---------|
| `TAURI_SIGNING_PRIVATE_KEY` | Key file **contents** |
| `TAURI_SIGNING_PRIVATE_KEY_PATH` | Path to key file |
| (fallback) | `tmp/updater.key` if present |

---

## Publish with the script

```powershell
cd C:\dev\TokenUsage

# 1) Bump version in package.json + tauri.conf.json + Cargo.toml (+ Cargo.lock)
# 2) Commit & push main

$env:TAURI_SIGNING_PRIVATE_KEY_PATH = "C:\dev\TokenUsage\tmp\updater.key"
# optional: $env:GITHUB_TOKEN = "ghp_..."

npm run release:publish
```

### Options

```text
npm run release:publish -- --dry-run       # build + write tmp/latest.json, no GitHub
npm run release:publish -- --skip-build    # reuse existing bundle/ + .sig
npm run release:publish -- --notes "..."   # release body
```

### Local signed run

```powershell
npm run run:exe
```

---

## Client behavior

| Path | Behavior |
|------|----------|
| Startup (release) | After ~30s, check + **background download**; badge ↻; ready → “click to restart” |
| Header **↻** (ready) | Install cached package + restart |
| Header **↻** (idle) | Full check → download → install if newer |
| `tauri dev` | Startup check skipped; manual check may still fail without a published `latest.json` |

## Pre-release verification matrix

| Check | Command / action |
|-------|------------------|
| Unit + risk | `npm test` |
| Coverage gate | `npm run test:coverage` (bash + tarpaulin) |
| Frontend | `npm run build` |
| Clippy | `cd src-tauri && cargo clippy --all-targets -- -D warnings` |
| Signed dry-run | `npm run release:publish -- --dry-run` |
| Publish | `npm run release:publish` (GitHub token + key) |
| Updater smoke | Install older signed NSIS → ↻ / wait for auto-check |

**Note:** Full NSIS/MSI CI is not on GitHub Actions (signing key must stay local). Windows workflow runs `cargo test` + `cargo build --release` only.

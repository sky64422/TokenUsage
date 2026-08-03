# Release & in-app updates

**Updated:** 2026-08-03  
**Current public tag:** v0.1.18  

**Audience:** maintainers publishing Windows builds that clients can install **and** self-update.  
**Product:** TokenUsage (`com.tokenusage.app`)

---

## What “publish” means

In-app **Check for updates** (header **⬆**) does **not** read git `main`.  
It downloads:

```text
https://github.com/sky64422/TokenUsage/releases/latest/download/latest.json
```

That file must list a **higher semver** than the installed app, a signed installer URL, and a matching signature.

| Step | Purpose |
|------|---------|
| Bump version | `package.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` |
| Signed `tauri build` | Produces NSIS/MSI + `.sig` (`createUpdaterArtifacts`) |
| GitHub Release | Hosts installer + **`latest.json`** as release assets |
| Users on prior release builds | Header ⬆ / startup check installs the new package |

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

# 1) Bump version in package.json + tauri.conf.json + Cargo.toml
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
| Startup (release) | After ~30s, check + download/install if newer |
| Header **⬆** | Manual `check_for_updates` (same install path) |
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
| Updater smoke | Install older signed NSIS → ⬆ / wait for auto-check |

**Note:** Full NSIS/MSI CI is not on GitHub Actions (signing key must stay local). Windows workflow runs `cargo test` + `cargo build --release` only.

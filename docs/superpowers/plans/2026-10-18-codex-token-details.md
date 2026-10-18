# Codex Token Details Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Show recorded Codex model usage and cache details in the existing detail panel, with provider icons, for user feedback before extending other providers.

**Architecture:** Keep vendor quota independent. A bounded background reader indexes Codex numeric usage metadata and attaches an optional token summary to existing snapshots. The detail renderer preserves expansion, selected period, focus and native geometry across refreshes.

**Tech Stack:** Existing Rust/serde/chrono, TypeScript/Vite, Vitest, Python Playwright and Windows WebView2. No new dependencies.

**Spec:** `docs/superpowers/specs/2026-10-18-token-details-design.md`; its latest Codex-only scope amendment takes precedence over the historical AGY investigation.

## Global Constraints

- Codex only for token collection. Do not add AGY/Claude/Grok token readers or modify CLI settings.
- Use `이 기기의 기록`, `오늘 / 이번 주 / 이번 달`; week starts Monday at local midnight, month starts on the first date of the current month.
- Read actual usage metadata only. No conversation/tool/thinking content in retained state, emitted snapshots, fixtures or diagnostics.
- Quota source, ring percentage, fixed 5-second refresh, activity and auth semantics remain independent.
- `TOKENUSAGE_SKIP_DIRECT_QUOTA=1` disables real token reads.
- Preserve 260 DIP width / intrinsic height bounded by work area, scaled content, fixed quota columns, native canvas, click-through and settings first-open shared height.
- Reuse `PROVIDER_CATALOG` marks before Provider names, including missing-quota cards. Grouped quotas show the icon only in the service header.
- User authorized proceeding with the approved design and narrowed implementation to Codex. Execute inline in an isolated feature worktree; leave release/install for a separate request.

## Review Focus

- A long-running session created months ago emits usage today: discover the file outside recent date directories (Task 2).
- Fork/resume copies historical counters: deduplicate logical session/events, exclude inherited baselines, mark uncertain contributions partial (Tasks 1–2).
- Multi-megabyte content lines, symlinks and a growing directory cannot monopolize scanning: bound work, retain progress, report partial coverage (Task 2).
- A quota authentication failure does not hide valid local token details, and a token read failure never changes quota status (Task 3).
- A focused period button survives a refresh and provider switches; expanded content fits at small scale and does not resize the native canvas (Task 4).

## Data contract

`domain/token_usage.rs` defines serializable `TokenUsageSnapshot { source, as_of, status, today, week, month }`, `TokenPeriod { models, totals }`, `ModelTokenUsage { model, counts }` and `TokenCounts` with optional input/cache_read/cache_write/output/reasoning counts. Status is `loading | ready | partial | unavailable`; source is `local_records`. Missing counts remain null. `model` is nullable. Totals use input+output; cache and reasoning remain subsets. CacheWrite is a separate reported value, not an extra total bucket. Period fields contain metadata totals only, with no file/session identifiers.

`TokenLedger` ingests per-session structural metadata and cumulative token events, retains validated contributions keyed for replacement/deduplication, and provides `summarize(now: DateTime<Local>, status: TokenUsageStatus) -> TokenUsageSnapshot`. Include future timestamps neither in today nor in month; flag invalid numeric contributions partial without fabricating zeros.

`infrastructure/token_usage::snapshot() -> TokenUsageSnapshot` returns cached/loading data immediately and schedules bounded work when due. `TokenReader::new(root: PathBuf)` and `TokenReader::scan(now: DateTime<Local>)` enable filesystem fixture tests without env races. An internal ledger is owned by the reader. Preview detection happens before path discovery. No second IPC command/event is introduced.

`ProviderSnapshot.token_usage: Option<TokenUsageSnapshot>` defaults to None and is omitted when absent. Frontend mirrors this as optional `token_usage`. Only AppCore's Codex snapshot receives the cached summary, including when quota is unavailable.

### Task 1: Correct Codex accounting and summaries

**Files:** Create `src-tauri/src/domain/token_usage.rs`; modify `src-tauri/src/domain/mod.rs`.

**Interfaces:** Produces the contract above; consumes typed Codex metadata/counter observations, without filesystem calls.

- [ ] Add failing tests for two cumulative reports (input 100/output 20 then input 250/output 40) plus a duplicate: total 290, never 410 or 700. Verify model switch allocates the increment to the observed model; unknown model stays null.
- [ ] Add tests for repeated event identities, resumed/forked baselines, counter reset, missing fields vs explicit zero, negative/inconsistent/subset overflow, local midnight, calendar-month boundaries, future timestamps and checked addition overflow. Ambiguous baseline contributes no guessed counts and marks partial.
- [ ] Run `cargo test --lib domain::token_usage` in `src-tauri`; observe the missing-accounting behavior fail before implementing it.
- [ ] Implement pure typed parsing/accounting/summary policy and run the same tests to pass. Validate cache<=input, reasoning<=output; never add subsets to input+output.
- [ ] Commit only Task 1 files after green tests.

### Task 2: Bounded incremental background reader

**Files:** Create `src-tauri/src/infrastructure/token_usage/{mod.rs,reader.rs}`; modify `src-tauri/src/infrastructure/mod.rs`. Tests live beside reader code.

**Interfaces:** Consumes Task 1 ledger; produces `snapshot()`, `TokenReader::new`, `TokenReader::scan`.

- [ ] Add filesystem tests: old dated session with a current event, append twice, copied session, partial final line, truncate/replace, invalid JSON, permission/read error, oversized irrelevant content, discovery budget exhaustion and eventual progress.
- [ ] Add a preview test that uses a trap/unreadable fixture and proves no path discovery/read is attempted when disabled.
- [ ] Run `cargo test --lib infrastructure::token_usage` and observe failure before implementation.
- [ ] Implement a discovery queue, file cursors and metadata-only serde records; ignore unknown fields without retaining text. Bound each pass to 512 discovery entries and 8 MiB reads, preserving offsets/incomplete lines. Maintain fairness for existing files and continue discovery on later passes. Skip symlinks. A bounded recent-row index must report partial when limits exclude records.
- [ ] Cache in memory, schedule one worker at a time with the existing five-second cadence, return immediately while running, and use `poll::notify_refresh()` for new results without a refresh storm. Scan `.codex/sessions` with the existing `paths::codex_home()`; include logical IDs to deduplicate copies/resume. No tail-only complete totals.
- [ ] Run filesystem tests to pass; commit Task 2 files.

### Task 3: Independent snapshot integration and policy docs

**Files:** Modify `src-tauri/src/domain/types.rs`, `src-tauri/src/application/service.rs`, `src-tauri/src/infrastructure/providers/quota/{snapshot.rs,agy.rs}`, `src-tauri/src/infrastructure/poll.rs`, `src/ui/types.ts`, `AGENTS.md`, `PRODUCT.md`, `docs/{providers.md,ARCHITECTURE.md}`. Update snapshot constructor literals as required by compilation.

**Interfaces:** Consumes `snapshot()` from Task 2; produces optional `ProviderSnapshot.token_usage` and matching TS types.

- [ ] Add failing tests for old snapshots without `token_usage`, Codex quota failure with valid local usage, and other providers having no token summary. Test that preview returns synthetic/empty token state without live reads and that diagnostics contain no token identities/content.
- [ ] Run targeted Rust tests and observe failures before wiring the new field.
- [ ] Attach the summary after vendor snapshot assembly for Codex only. Preserve vendor cache format by attaching after quota fetching, and keep all quota constructors initialized with `token_usage: None`.
- [ ] Add the user-authorized Codex metadata exception to docs, preserving the ban on local quota estimation. Run `cargo test --lib`; commit Task 3 files after passing.

### Task 4: Token detail UI and Provider marks

**Files:** Modify `src/ui/providers.ts`, `src/ui/app.ts`, `src/styles/notch.css`; create `src/ui/token-details.ts`, `src/ui/token-details.test.ts`, `scripts/token-details-smoke.py`. Reuse `src/ui/provider-catalog.ts` and existing SVGs.

**Interfaces:** `mountTokenDetails(root: HTMLElement, onResize: () => void)` returns `update(snapshot: TokenUsageSnapshot | null): void`; `mountProviders(root, onResize?)` preserves its existing public methods and calls the callback after user expansion/period changes. App passes its existing `requestSurface` callback.

- [ ] Add failing pure formatting/ratio tests for input 1000/cache 750/output 200/reasoning 50 => total 1200, 75% cached, and missing vs zero. Verify weighted ratios and untrusted model label escaping through the browser test.
- [ ] Add a Playwright smoke using existing Vite fixture style: open detail, choose month, focus the button, update snapshots, switch provider and return; assert expansion/selection/focus persist. Cover missing/zero/partial/error, long model label, many models and overflow, three themes and scale 0.92/1.
- [ ] Include Provider icon assertions for Codex, grouped AGY, Claude/Grok and missing-quota rows. Icons use `alt=""`, `aria-hidden="true"`, fixed 14px scaled size and neutral existing marks; no model-family icon duplication or changes to quota column layout.
- [ ] Run `npx vitest run src/ui/token-details.test.ts` and `python scripts/token-details-smoke.py`; observe failing behavior before implementing.
- [ ] Render token metadata below quota with native disclosure/button controls, provider-specific state, period tabs, model amounts/shares, input/output split, CacheRead ratio and separate CacheWrite/reasoning numbers. Patch stable DOM elements rather than replace focused controls on every snapshot. Present `이 기기의 기록`, updated time and partial/loading/error states.
- [ ] Request native surface height after layout-affecting actions through the existing serializer. Do not add a ResizeObserver/content-hug loop or frontend HWND movement. Run frontend tests, token smoke, existing detail smoke and `python scripts/settings-smoke.py`; commit Task 4 files when green.

### Task 5: Full verification and feedback preview

**Files:** Update `docs/testing.md` with token smoke and collection limits. Scratch evidence stays in ignored `tmp/` only.

**Interfaces:** Consumes completed Tasks 1–4 and the existing isolated Windows preview scripts.

- [ ] Run `npm test` and `npm run build`; expect all suites and compilation to pass.
- [ ] Run `python scripts/settings-smoke.py`, `python scripts/detail-layout-smoke.py` and `python scripts/token-details-smoke.py` against Vite; expect no page errors, first-open resize or horizontal overflow.
- [ ] Launch an isolated Windows preview with synthetic fixtures for geometry; run `python scripts/notch-hover-stability.py --port 9223 --fixture` and inspect all four edges plus token expansion, pointer travel and click-through. Then restart that preview without fixtures to compare real Codex metadata; do not install or publish.
- [ ] Review the full diff against the spec, verify no AGY reader/dependency/secret/unrelated benchmark file was introduced, and record actual test evidence and remaining native limits. Commit only requested changes; provide a usable preview path and supported-data limitations for user feedback.

## Execution result — 2026-10-18

Tasks 1–5 are complete in `feat/codex-token-details`; the checklists above preserve
the original execution sequence. Later user requests added calendar periods,
sliding tabs, ranked model shades/legend, a single notch ring and intrinsic detail
height. Collection remains Codex-only.

- `npm test`: 189 passed; one opt-in live AGY test ignored.
- `npm run build` and `cargo clippy --all-targets -- -D warnings`: passed.
- Settings, detail layout, token details, empty-track pixels and single-ring
  browser smoke: passed on isolated Vite port 1421.
- Latest Windows WebView2 preview: all four edges expanded from 161 to 702px
  without quota scrolling or canvas/notch movement; first settings tab heights
  440/440px; native click-through and nonactivating hover passed.
- Native unequal-provider hover: 260 stable frames. Independent code review
  found no remaining Critical or Important issues after accounting corrections.

Live Codex records populate model/cache data independently of official quota.
Initial indexing is loading; excluded or uncertain records are partial. No
release or installation was performed. Local screenshots and measurements stay
in ignored `tmp/`.

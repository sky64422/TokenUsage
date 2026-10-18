# Recorded token details across providers

Expand the completed Codex display to AGY, Claude and Grok using exact recorded
numeric usage. Official quota remains the primary data. Never derive token counts
from percentages, prices, text lengths or activity.

- Reuse bounded incremental reads, rewrite checks, retention and summary caches.
  Keep provider state independent and disable live discovery in preview mode.
- AGY has step input/cache/output counts and timestamps, but historical records
  do not provide model names, cache-write or reasoning counts. Preserve unknowns;
  do not attribute unknown models to Gemini/Claude quota families.
- Claude has per-response usage and models. Normalize disjoint input buckets and
  deduplicate repeated response blocks, ignoring nested progress/tool summaries.
- Grok has per-turn/per-model usage. Cache is included in input and reasoning in
  output. Validate model totals and ignore capacity metadata or mirror files.
- Reuse Today/Month tabs, icons and styling with provider-specific ARIA IDs.
  Main bars retain vendor quota width; match kind, reset, group and label before
  shading models. Unsupported period boundaries remain plain quota bars.

Verification: parser/dedup/unknown/overflow regressions, bounded incremental source
tests, group isolation, real-browser multi-provider interactions and native detail
geometry. Run npm test, production build and Clippy before commit/push.

References:
- https://www.antigravity.google/docs/hooks
- https://github.com/xai-org/grok-build
- https://platform.claude.com/docs/en/build-with-claude/prompt-caching

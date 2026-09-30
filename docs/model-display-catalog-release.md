# Callable-model display catalogue release rule

## Owner and runtime contract

`apps/desktop/src/features/console/modelDisplayCatalog.ts` is the single static
display-order table shared by provider-pool and credential cards. Array order
defines editorial company prominence, then model prominence within each company.
It is a release-time product decision, not a measured popularity score or a
runtime network service. Changing request volume never changes this order.

Only exact reviewed model names and their declared namespace aliases match.
Never classify an unknown model merely because its prefix looks like a known
company. Unmatched models appear in the final Unclassified block, in input order.
Known aliases with the same rank retain input order. Empty companies are omitted.
The table intersects current credential capabilities; it never grants access,
adds a callable model, rewrites routes or merges different models' statistics.
Full upstream names remain in accessible text and hover titles when visually truncated.

## Mandatory checklist before each release

Complete this before `tools/build-gateway-release.ps1` freezes source provenance,
for both Windows and Docker releases:

1. Retrieve current model identifiers from the providers' official model catalogues
   and release notes, then compare them with the actual configured/discovered
   model inventory. Start with OpenAI, Anthropic, Google, Meta, DeepSeek, Qwen,
   xAI, NVIDIA, Mistral, Moonshot, MiniMax and Z.ai. Do not copy user keys or
   credential files into release evidence.
2. Record retrieval date, source URLs, added/removed/renamed identifiers and the
   proposed company/model order in `target/release-evidence/<versionId>/model-catalog.md`.
   Official sources establish ownership and identity. The release owner reviews
   prominence using broad adoption, ecosystem visibility and current flagship
   relevance; no alphabetical ordering, automatic usage ranking or invented score.
3. Generate a candidate company/model array from those reviewed facts. Keep explicit
   aliases, put flagship/popular families first and retain supported legacy models
   below them. Never synthesize version suffixes or silently match whole namespaces.
   Unknown identifiers remain unclassified until reviewed. Do not infer that a
   model is callable just because it exists in the display table.
4. Review the candidate diff, update the static table and its
   `MODEL_DISPLAY_CATALOG_REVIEWED_AT` date, and record the decision. If no ordering
   change is warranted, record a reviewed no-change result. If retrieval is
   unavailable, record the stale source/date and a release-owner decision; do not
   change the date to imply a fresh online review.
5. Run `CardModelList.test.tsx`, `credentialAuditLedger.test.ts`, the NVIDIA card
   integration suite, typecheck and the effective-line ratchet. Verify alias
   uniqueness, unknown fallback, no injected capability and no cross-credential
   statistics. Inspect the packaged UI at wide/narrow widths with long names,
   keyboard expand/collapse, internal scrolling and unavailable/zero statistics.
6. Build a new immutable release after the table/docs are final. Save the table's
   revision/hash and UI evidence beside build provenance. A later table edit
   requires rebuilding; never edit a published package.

Suggested starting sources (consult their current pages at release time):

- https://platform.openai.com/docs/models
- https://docs.anthropic.com/en/docs/about-claude/models/overview
- https://ai.google.dev/gemini-api/docs/models
- https://build.nvidia.com/models
- https://huggingface.co/meta-llama
- https://huggingface.co/deepseek-ai
- https://huggingface.co/Qwen
- https://docs.x.ai/docs/models
- https://docs.mistral.ai/getting-started/models/models_overview/

## Initial table and statistics semantics

The 2026-09-27 initial ordering is editorial. NVIDIA entries were seeded from the
130-model configuration observed during native UI acceptance. This is not an
online popularity survey; the initial OpenAI example also includes the user's
`gpt-6-astra` identifier. Catalogue membership alone makes no availability claim.

Pool model call counts come from the retained cost-overview usage aggregation.
Credential model call counts come only from that provider/credential pair in the
recent audit sample (currently up to 1000 recent records across the gateway).
Success strips use the same recent hourly-window semantics as the whole card;
running/failed calls are in the denominator. Tooltips distinguish retained counts
from recent sampled counts. Missing telemetry displays unavailable; an available
sample with no matching model displays zero calls and unavailable success rate.
Refresh updates statistics without changing the release's static display order.

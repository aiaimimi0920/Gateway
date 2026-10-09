# Callable-model display catalogue release rule

## Owner and runtime contract

`apps/desktop/src/features/console/modelDisplayCatalog.ts` is the single static
display-order table shared by provider-pool and credential cards. Array order
defines editorial company prominence, then model prominence within each company.
It is a release-time product decision, not a measured popularity score or a
runtime network service. Changing request volume never changes this order.

Exact reviewed model names and their declared namespace aliases take precedence.
The user-requested display policy additionally classifies `gpt-*` as OpenAI and
`grok-*` as xAI, and all `glm`-prefixed names as Z.ai, including unlisted version/variant suffixes. Matching is case-insensitive. Z.ai namespaces include `z-ai`, `z.ai`, and `zai`. This fallback accepts
bare IDs or the matching declared company namespace only, not arbitrary proxies,
conflicting namespaces or unrelated names under a known namespace. Family matches
follow exact catalogue ranks and retain input order. This is a naming convention,
not verification of an upstream alias's actual manufacturer or availability.
Unmatched models appear in the final `other` group, in input order,
with the same expand/collapse behavior as the company groups.
Known aliases with the same rank retain input order. Empty companies are omitted.
The table intersects current credential capabilities; it never grants access,
adds a callable model, rewrites routes or replaces individual model statistics.
Company rows show `Company(n)` and the same three metrics as model rows, even
while collapsed. They sum only the configured models in that group: requests and
successful requests are summed before dividing, never averaged percentages.
Quality windows are combined by their full UTC bucket keys. Incomplete totals
remain unavailable rather than being presented as a complete company subtotal.
Indented model rows omit the organization/company namespace from the visible name,
including models in `other`; shortening does not infer company ownership.
Model-family names remain intact. Full upstream IDs remain in accessible labels,
hover titles and telemetry keys; display shortening never rewrites capabilities.

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
   Apart from the explicit GPT/Grok/GLM family display rules, unknown identifiers
   remain in `other` until reviewed. Do not infer that a
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

Model rows show three separate metrics: total calls, a service-quality heatmap,
and total success rate. The console opts into `includeModelTotals=true` on the
request-summary endpoint. Its optional `retainedModelTotals` is an unsampled SQL
aggregation over all currently retained audits, independent of the recent summary's
row/time filters, with identical SQLite/PostgreSQL provider/credential/model keys.
Pool rows sum all credential buckets (including unattributed historical rows);
credential rows include only the exact provider and explicit `realCredentialRef`.
Missing attribution is never guessed. Total success is completed calls divided by
all retained calls, including running, cancelled and failed calls. These totals
are not permanent lifetime counters: local audit maintenance retains 90 days.
The heatmap still uses the last four observed hourly windows from the recent audit
sample (up to 1000 gateway records), oldest to newest; it is not the total rate.
Missing totals display unavailable. Available totals with no matching model show
zero calls and unavailable rate, never 100%. Older backends may still supply pool
call counts through cost overview, but cannot supply a total success rate; recent
credential samples are never relabelled as total calls. No new polling is added.
Refresh updates statistics without changing the release's static display order.

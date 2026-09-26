# S06 hub inventory and reservation

This is a read-only handoff checkpoint for the two Rust hubs that remain above
the effective-line migration ceiling. It records the current ownership boundary
and a bounded inventory for the existing S06 executor. No S06 source file was
edited in this checkpoint.

## Ownership reservation

`Gateway/AGENTS.md` keeps the existing S06 executor's Rust/Gemini lane exclusive
to that executor. The board still lists lane A / S06 as
`external_in_progress, acknowledged`, with the exit target "no takeover". The
handoff request `GWP-20260912-01` still requires an explicit receipt from that
executor, or an explicit user authorization, before source ownership can move.
Historical build-window returns do not transfer the Rust source cursor.

## Current strict inventory

The fresh strict scan covers 2,335 files:

- 6 files above 1,500 effective lines;
- 8 files in the 701-1,500 tier;
- 11 files in the 501-700 tier;
- the only handwritten Rust violations are
  `src/upstream/client.rs` (15,638) and
  `src/protocol/gemini_canvas.rs` (7,784).

The other 12 strict violations are Git-ignored browser-profile/runtime payloads.
Their provenance proposal remains an evidence draft: it requires exact
path/version/hash evidence, source and license proof, checker regressions, and
explicit policy approval. They remain immutable and visible to strict auditing;
no exclusion or baseline change is authorized by this checkpoint.

## `src/upstream/client.rs` inventory

`UpstreamClient` remains the single shared resource owner. Its long-lived HTTP
clients, timeout, browser-executor credentials/policy, Redis pool, and Postgres
pool are initialized separately and are borrowed by the methods below. The
lowest-risk future extraction order is:

1. core request planning and `send_plan` wiring (`316-498`, `15515-15521`);
2. JSON and binary passthrough (`998-1309`);
3. browser-executor policy bridge (`1311-1453`);
4. Gemini Business image execution (`1455-1572`);
5. pure fallback/continuation helpers and their tests;
6. the larger Gemini Canvas orchestration regions, one coherent lifecycle at a
   time.

The Canvas region shares `self.http`, `self.plain_http`, timeout floors,
provider-account identity, Redis/Postgres references, browser-pool process
cleanup, and streaming response ownership. Splitting it by raw line range would
change those lifetimes; each owner must borrow the existing `UpstreamClient`
state and preserve method signatures used by sibling provider `impl` blocks.

## `src/protocol/gemini_canvas.rs` inventory

The protocol facade currently contains these responsibility groups:

- public constants, errors, and shared types (`1-503`);
- StreamGenerate/template/request state and batchexecute/preflight builders
  (`505-1966`), which need at least separate request and batchexecute/state
  owners;
- runtime, API-key, cookie, signaler, and Sapisid session material
  (`1968-2853`), which must preserve one mutable session lifecycle;
- response/media extraction and OpenAI response construction (`2854-3383`);
- frame parsing, media candidate ranking, and JSON helpers (`3384-4387`),
  which have a one-way parser dependency;
- the inline tests (`4388-end`), which must move with their owners rather than
  remain in the facade.

The public `crate::protocol::gemini_canvas::*` surface is used by upstream
execution, image-edit signaler/transport, official media helpers, music
helpers, credential normalization, and cross-module tests. A future split must
retain that facade or provide explicit `pub(crate)` re-exports while keeping
`GeminiCanvasPureHttpSession`, `GeminiCanvasRuntime`, and
`GeminiCanvasMediaAsset` state ownership unchanged.

## Verification at this checkpoint

- Gemini Business focused tests: 32 passed.
- Gemini auth-session focused tests: 13 passed.
- Gemini Canvas image-edit local-helper focused tests: 74 passed.
- Gemini Canvas direct-HTTP focused tests: 55 passed.
- `cargo fmt --all -- --check`: passed.
- `cargo check --offline --locked --lib`: passed with one pre-existing unused
  function warning in `gemini_canvas_music_helpers.rs`.
- `cargo check --offline --locked --all-targets`: passed with the same warning.
- Effective-line checker tests: 19 passed; adoption-baseline ratchet passed.
- Strict audit remains intentionally red for the 14 listed historical/policy
  entries and the two reserved S06 hubs.
- Scoped `git diff --check`: passed.

No Gateway release artifact, runtime deployment, or source ownership transfer
was performed.

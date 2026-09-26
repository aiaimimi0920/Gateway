# S06 hub facade verification

Date: 2026-09-24 UTC. Status: structural verification complete; S06 remains
in progress for the separate hardening and integrated acceptance gates.

This checkpoint reconciles the older S06 inventory with the current dirty
working tree. The historical inventory listed `src/upstream/client.rs` at
15,638 effective lines and `src/protocol/gemini_canvas.rs` at 7,784 effective
lines. The inherited extraction is now present in the working tree and keeps
both public module paths as facades:

| Facade | Current effective lines | Current physical lines | Extracted owner surface |
| --- | ---: | ---: | --- |
| `src/upstream/client.rs` | 411 | 438 | initialization, browser executor, passthrough, request planning, streaming, Gemini Canvas execution/follow-up/media owners, fallback tests |
| `src/protocol/gemini_canvas.rs` | 297 | 344 | API keys, request/batchexecute state, session material, stream/media parsing, response builders, image-edit upload owner and focused test owners |

The facades retain the existing public paths and delegate through explicit
`#[path]` owners and re-exports. No runtime or release ownership moved in this
verification, and no browser-profile payload was edited.

## Fresh verification

- `cargo test --locked gemini_canvas -- --test-threads=1`: 593 passed, 3
  ignored, 2,625 filtered, exit 0. The run also exercised the facade's
  `upstream::client` fallback/continuation tests and the protocol's focused
  request, session, media, and parser contracts.
- `cargo fmt --all -- --check`: exit 0.
- `npm run test:effective-lines --prefix scripts`: 19/19 passed.
- `npm run check:effective-lines --prefix scripts`: ratchet passed over 2,485
  files.
- The current report has 4 files above 1,500, 8 files in 701-1,500, and 2 in
  501-700. All 14 rows are immutable `deploy/gateway_data/browser-profiles/**`
  runtime/extension payloads; no first-party Gateway source, test, script, or
  desktop file is above 500 effective lines.
- `git diff --check` and `git diff --cached --check`: exit 0. Git emitted only
  existing line-ending conversion warnings for unrelated dirty files.

The Rust test run emitted existing unused-import/unused-variable warnings from
the decomposed surface, but no warning was promoted to an error and all
selected tests passed.

## Remaining work

There are currently **zero ordinary first-party files requiring another
mandatory size split**. The remaining count of 14 is a strict-audit governance
count, not a source-extraction count. Those payloads require the separately
reviewed provenance/license policy and explicit S20 approval; they must not be
deleted, minified, or blanket-excluded.

S06/S18 behavioral hardening, native single-body capture bounds, the provider
line matrix, S20 policy activation, and S21 release/runtime/UI/Docker
acceptance remain open. This document therefore closes only the hub facade
split verification and does not mark the overall refactor complete.

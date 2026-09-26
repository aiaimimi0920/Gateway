# Gemini auth session owner checkpoint

This batch continues the [large-file governance checkpoint](2026-09-22-large-file-governance.md)
after the Gemini helper and media owners. The Gemini auth session implementation
is now split by state ownership, helper execution, output parsing, credential
draft construction, and tests. Repository-wide migration and the verified
release goal remain open.

## Ownership and line counts

Counts use the repository's authoritative effective-line lexer. The before value
is the working-tree value recorded in the preceding governance inventory.

| Owner | Before | After |
| --- | ---: | ---: |
| `src/console/gemini_auth_sessions.rs` | 1,648 | 498 |
| `src/console/gemini_auth_session_workers.rs` | New | 461 |
| `src/console/gemini_auth_session_parsing.rs` | New | 254 |
| `src/console/gemini_auth_session_drafts.rs` | New | 179 |
| `src/console/gemini_auth_session_tests.rs` | New | 304 |

The parent retains the public session family/status/input/view types, the
`GeminiAuthSessionManager`, session state transitions, manual-completion control
state ownership, and the existing console re-exports. Workers own local/remote
helper execution and environment selection. Parsing owns remote and local JSON
validation. Draft construction owns the credential payload and secret-edit
contracts. The original tests remain registered under the parent module while
living in their own owner.

No provider wire contract or public function signature changed. The extraction
uses explicit `pub(super)` boundaries and keeps control-file cleanup in the
manager/worker ownership path.

## Preservation and regression evidence

The source projection preserves all original production functions and all 13
auth-session tests. The only source changes are module placement, imports, and
the visibility required for the parent manager and child test module to access
the extracted owners. No credential values, bearer tokens, or browser payloads
were added to source or logs.

The review covered helper process timeouts, remote executor authentication,
control-file creation/removal, session state transitions, JSON validation,
secret-edit construction, and bounded output summarization. Existing behavior
and cleanup paths remain unchanged.

## Verification

- `cargo fmt --all -- --check`: passed; the extracted files have no formatter
  drift.
- `cargo check --offline --locked --lib`: passed. Existing unrelated warnings
  remain in other Gemini helper files.
- `cargo check --offline --locked --all-targets`: passed.
- `cargo test --offline --locked console::gemini_auth_sessions --lib --
  --test-threads=1`: 13 passed, 0 failed.
- `npm run test:effective-lines --prefix scripts`: 19 passed, 0 failed.
- `npm run check:effective-lines --prefix scripts`: passed.
- `git diff --check` for the scoped Gateway paths: passed.

The current strict inventory reports 2,305 scanned files with 8 above 1,500
effective lines, 9 in the 701-1,500 tier, and 14 in the 501-700 tier. The strict
audit still exits 1 for the remaining historical Rust owners and browser
profile/runtime payloads; this batch introduces no new exception requirement.

No release package was built or deployed for this atomic source extraction.

## Remaining scope

The remaining Rust large-file owners in the current strict audit are
`src/upstream/client.rs`, `src/protocol/gemini_canvas.rs`,
`src/upstream/gemini_canvas_image_edit_local_helpers.rs`,
`src/upstream/gemini_canvas_direct_http_helpers.rs`, and
`src/protocol/gemini_business.rs`. The two first hubs remain reserved S06 work.
Browser-profile/runtime payload provenance still requires a separate policy
decision. Full provider/UI/runtime/release acceptance remains pending.

Gateway remains a dirty, uncommitted working tree with inherited changes
preserved. This batch owns the modified auth-session parent, four new source/test
owners, and this report; other subprojects received no edits or validation
claims.

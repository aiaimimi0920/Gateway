# ChatGPT HTML classification ownership and MIME, 2026-09-15

HTML challenge media-type matching is verified. Full Gateway optimization and
product/release acceptance remain open.

## Implemented boundary

The response classifier and existing tests now have dedicated owners. Public
exports remain unchanged. Browser challenge detection compares the first-semicolon
media-type essence to text/html with ASCII case-insensitive equality and SP/HTAB
trimming. Unrelated subtype suffixes, prefixes and parameter tokens no longer
produce the challenge error code merely because their Content-Type contains text/html.
The browser-header comparison no longer allocates a lowercase copy.

Challenge-before-session error priority, messages, codes, provider/status fields,
body sniffing, challenge/auth markers, status sets and generic status fallback are
preserved. MIME parameters are not fully parsed. Existing heuristic body sniffing
and redundant session-invalid HTML logic are unchanged. No live-provider or actual
browser refresh/relay behavior is claimed from these unit/loopback tests.

Effective-line measurements:

- src/protocol/chatgpt/web_reverse/response.rs: 652 -> 404.
- response/classification.rs: new 67 at structural baseline, 72 after the fix.
- response/tests.rs: new 316, containing the original tests plus seven new tests.

All completed owners are below 500. This removes one 501-700 soft-limit file;
it does not clear a file from the above-700 inventory.

## Three-phase verification

All three runs used GATEWAY_PREBUILT_WEB_UI=1 and:

    cargo test --offline --locked --lib chatgpt -- --test-threads=1

1. Original source: 163/163, 2905 filtered out.
2. Classifier/test extraction plus regressions, unchanged algorithm: 166 passed /
   4 failed, 2905 filtered out. Failures are exactly the four HTML MIME negative
   groups, first failing on status 200. All original identities and three new
   preservation tests pass.
3. Fixed, frozen candidate: 170/170, 2905 filtered out, first candidate run passed.
   Tests and response entry are byte-identical to the regression snapshot.

The candidate executes ten negative types across four statuses (40 combinations).
Baseline groups stop at their first assertion failure, so only four independent
failures were observed. Positive tests retain case/OWS/quoted parameters, real HTML
body sniffing, status and non-challenge-marker boundaries, and invalid-session
code/kind/provider/status. The broad original set comprises 65 protocol ChatGPT,
71 upstream ChatGPT, seven stage_send policy and 20 other matching library tests;
all 163 remain green after the change. No full-library-suite claim is made.

Closing gates pass: cargo check --offline --locked --all-targets; scoped official
rustfmt --check; checker tests 19/19; ratchet; Gateway and Neuro staged/unstaged Git
checks. All native phases are terminal. Existing Gemini unused HashMap warning
remains; elapsed test timings are not a performance benchmark.

Strict audit: 2094 scanned / 12 hard / 20 mandatory / 39 soft. Above-700 debt remains
32 files (20 Rust, 12 runtime-profile/vendor); clearance 113/145 (77.9%). No checker,
policy, baseline, exception or dependency changes were made.

## Preservation, corrections and evidence

Evidence: target/effective-line-evidence/20260915-chatgpt-html-mime/.
Before capture verified the SSE MIME publication, five docs, 1797 inputs, 22 assets
and exact repository statuses. Final union: 1799 inputs; 1796 unchanged neighbors;
22 unchanged web/Tauri assets. The verifier reconstructs entry, all three original
classifier functions and the complete original test module from original-response.rs,
then compares exact official-rustfmt output. Only the browser-header predicate is
changed in the candidate; all original assertions and stream code are preserved.

Original source had 699 CRLF lines. A first extraction attempt failed to locate its
LF-only test marker and made no source changes. A premature formatter invocation
then failed on the missing owner paths; regression-format.json is retained. After
explicit CRLF-aware extraction, regression2-format.json passed. A projection-script
quote was corrected before its first execution. No executed regression fixture was
changed afterward. All modified/new sources and docs are UTF-8 without BOM.

Read-only fixture/structure and final semantic reviews found no introduced defect.
The fixture reviewer observed the intentional baseline bug and noted unit-only
coverage and unchanged heuristic sniffing. Its shell-routing failure supplies no
native test evidence. Coordinator receipts alone establish the gate results.

scope.json observedAt: 2026-09-15T14:09:21.890Z.
Scope SHA-256: 9ab51c1137bf094edf1abd13f3a801d1f11edbf336757a7250c27bfa63ddb4ea.
Source SHA-256 values:

- response.rs: 4f3e6517ff7ee5944533694e6432e98783fc2af4541a405e70c9febb5e3c8ca3.
- classification.rs: abafc6cf0c40a7f90e24502d827f4202069c07006c6e091180cc0cbfe90155e9.
- tests.rs: 201cffdca139bbb77cc7bc1bc02d9686b07968d046e136e32a70d393be4e2b33.

publication.json verifies source/evidence hashes, final docs, script limits and
final Git state. Gateway: 192 modified, 1 unstaged deletion, 2 staged deletions,
2300 untracked; Neuro: 10 modified, 182 untracked. HEADs remain
4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d and
bf818f0324024634bc890585efb78cc8e603d11a. No staging, commit or release occurred.

Synchronous proof-solver scheduling, remaining strict debt and full feature/language/
provider/runtime/UI/Docker/release checks remain open. S06 original Rust/Gemini
implementation, cursor and final-build coordination remain reserved under
GWP-20260912-01. Persistent service target remains 4200, and releases belong only
under Neuro/release/Gateway.


# ChatGPT SSE media-type matching, 2026-09-15

The SSE admission repair is verified. Full Gateway optimization and release
acceptance remain open.

## Change and preservation

The existing substring check admitted unrelated media types whenever their header
contained text/event-stream. common.rs now compares the essence before the first
semicolon with text/event-stream using ASCII case-insensitive equality and HTTP
SP/HTAB trimming. It no longer allocates a lowercase copy of the header.

Only this predicate changes. Response status handling, challenge/session detection,
bounded diagnostic body reads, fixed non-SSE errors, request preparation and live
bytes_stream translation remain byte-for-byte unchanged. Parameter grammar is not
validated; this is an essence routing predicate, not a complete MIME parser.

Production owner src/upstream/chatgpt/common.rs: 25 -> 30 effective lines.
Test owner src/upstream/chatgpt/execution/tests/stream.rs: 187 -> 274.
Both remain below 500. Five tests are appended; the entire original fixture and
original test source are preserved as an exact byte prefix. No baseline/candidate
fixture edits or evidence corrections were needed.

## Fresh verification

Both phases used:

    GATEWAY_PREBUILT_WEB_UI=1
    cargo test --offline --locked --lib upstream::chatgpt:: -- --test-threads=1

- Baseline: 67 passed / 4 failed, 2997 filtered out.
- Candidate: 71/71, 2997 filtered out; passed its first native run.
- The 66 original upstream ChatGPT identities pass in both phases. This includes
  all 34 previously accepted Official API tests. The added preservation test also
  passes in both phases. This filter excludes protocol::chatgpt tests; no fresh
  protocol-suite or repository-wide runtime-test result is claimed.
- Four negative groups fail before at the first bad type and pass after: subtype
  suffixes, type prefixes, token occurrences in parameters and comma lists. The
  candidate executes all ten negative header values. The baseline proves four
  group failures, not ten independently executed negative values.
- Two new valid header variants preserve Paris text, one stop and one DONE:
  uppercase essence with SP; HTAB with quoted charset and a semicolon inside a
  quoted parameter. The two original positive header variants also remain green.
- The fixture uses loopback ephemeral listeners, no_proxy, request/drain timeouts,
  explicit abort/await and Drop cleanup. All native suites use one test thread
  because inherited body-bound tests allocate large synthetic responses.

Closing gates pass: cargo check --offline --locked --all-targets; scoped official
rustfmt --check; checker tests 19/19; ratchet; Gateway and Neuro staged/unstaged
Git diff checks. Every native phase is terminal and the terminal process guards
observed no cargo/rustc/rustfmt processes. Existing Gemini unused HashMap warning
is unchanged. Test timings are not a performance benchmark.

Strict audit is unchanged and remains red: 2092 scanned / 12 hard / 20 mandatory /
40 soft; 32 files above 700 (20 Rust, 12 runtime-profile/vendor files). Clearance:
113/145 (77.9%). No checker, policy, baseline, exception or dependency changes.

## Evidence and review

Evidence: target/effective-line-evidence/20260915-chatgpt-sse-mime/.
The before snapshot verified the preceding Official publication, five docs,
1797 input files, 22 assets and exact Gateway/Neuro Git status. Closing source proof
verifies the single production replacement, unchanged regression tests, 1795
unchanged neighboring inputs and all 22 unchanged web/Tauri assets.

scope.json observedAt: 2026-09-15T13:32:04.475Z.
Scope SHA-256: 7754e92e8a41083c9c851dc46c67db354a65af4c4818e89a13fee238f8882d49.

Production SHA-256: 74cbad07fe2b976ab53bf87ea742265afcfd1043223dce0b97834ed301c2b32f.
Tests SHA-256: c0406dc3a7b00c8988f1801e029521c6443f3cd2ebd25f36c296dad0c3bdaa55.
Paired logs/receipts, original sources, frozen snapshots, source verifier and
independent review.md are retained. publication.json verifies these hashes,
terminal gate receipts, five final docs, script limits and exact final Git state.

Independent agents mime_fixture_review and mime_semantic_review found no
introduced defect. Coordinator verified the narrow source projection and retained
all inherited staged/unstaged/deleted/untracked state. No staging or commit occurred.
Final Gateway status after report publication: 191 modified, 1 unstaged deletion,
2 staged deletions, 2296 untracked. Neuro: 10 modified, 182 untracked. HEADs remain
4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d and
bf818f0324024634bc890585efb78cc8e603d11a, respectively.

## Remaining work

- HTML challenge classification still uses substring matching in
  protocol/chatgpt/web_reverse/response.rs:46-57. The nearby invalid-session branch
  has redundant html_like logic and is already independent of the MIME result;
  no session MIME regression is claimed.
- execution/requirements.rs calls synchronous PoW and Turnstile solvers directly
  inside async request processing. Existing work bounds limit their workload but
  do not establish Tokio scheduler fairness or cooperative cancellation. A future
  scheduling repair must bound concurrency and retain cancellation/lifetime proof;
  spawn_blocking alone cannot forcibly cancel an already running job.
- Remaining strict debt, feature/language/provider coverage and actual packaged
  runtime/UI/Docker/release checks remain open. No live-provider compatibility or
  released binary behavior was established by this loopback test lane.
- S06 original Rust/Gemini implementation, cursor and final release-build
  coordination remain reserved under GWP-20260912-01. Persistent service target
  remains 4200; releases stay under Neuro/release/Gateway.


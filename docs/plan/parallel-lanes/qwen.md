# Lane B: Qwen session worker extraction

- State: released in interim checkpoint `20260908-parallel-refactor-002354`.
- Owner: coordinator-dispatched default agent `/root/qwen_split_pilot`.
- Original: `scripts/qwen-web-session-worker.mjs`, 889 effective lines.
- Write scope: old worker, `scripts/qwen-web-session/`,
  `scripts/tests/qwen-web-session-worker.test.mjs`, and this record only.
- Target: entry and every new module <=500 effective lines, preserving the CLI,
  page-evaluate serialization boundary, profile lifecycle, credential payload,
  error mapping, and file-path behavior.
- No real profile, credential, network, provider, browser, or Cargo operations.
- Coordinator acceptance: structure and focused offline contracts accepted.
- Release acceptance: official build/package/integrity and isolated runtime
  gates passed; live Qwen/browser-success semantics remain a separate gate.

## Original agent submission (historical, not acceptance evidence)

Baseline: original worker 889 effective / 950 physical (board baseline). Safe CLI
characterization was attempted with `null` input; extraction syntax checks passed
after correction. No browser, network, provider, or credentials were used.

Extraction: `scripts/qwen-web-session-worker.mjs` now delegates page probing,
login, and credential serialization to `scripts/qwen-web-session/` modules. Native
`.mjs` entry path and stdin/stdout orchestration remain unchanged. New modules are
`page-probe.mjs`, `login.mjs`, and `credentials.mjs`; no dependencies or config
were changed. Focused command: `node --check scripts/qwen-web-session-worker.mjs`
and checks for all three extracted modules (pass).

Risks: offline test file was absent in the checkout and was not created in this
bounded pass; coordinator must add/verify fake-page/profile-temp-dir coverage and
remeasure effective lines/encoding before acceptance. Credential module uses a
dependency object for normalization/default family wiring; inspect integration.

## Coordinator acceptance: 2026-09-08

- Fixed missing `defaultFamilyDir` dependency, removed duplicate login body and
  unused imports, corrected whitespace and removed the four UTF-8 BOMs.
- Effective / physical lines: entry 412/449 (was 889/950), credentials 98/106,
  login 47/53, page probe 175/180, focused tests 143/152. No new >500 file.
- Six focused tests pass: serialized page callback without module closures,
  token fallback, preferred model/chat contract, auth error, temporary credential
  paths/payload, login sequence, and structured CLI errors (some tests cover
  multiple cases). No real account, profile, browser or network used.
- The clean original HEAD was re-executed after extraction in an isolated VM
  with browser access forbidden. Malformed JSON and null input preserve complete
  JSON output and exit 1. This is reconstructed baseline verification, not a
  claim that the implementation agent ran a pre-change test suite.
- AST-normalized comparison confirms both original page callbacks equal the
  new serialized callback. Node syntax, encoding, checker tests, ratchet and
  diff checks pass. See the shared verification report for exact commands.
- Remaining: full successful-worker/profile-clone orchestration, real browser
  semantics, integrated release/runtime gates. Existing `process.exit` before
  async `finally` is unchanged; this extraction does not certify cleanup on
  successful process exit or silently fix that separate lifecycle concern.

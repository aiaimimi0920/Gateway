# Lane H: AI Studio worker (structural green, hardening open)

Coordinator owns scripts/aistudio-web-browser-worker.mjs, its new sibling
aistudio-web-browser directory and focused worker boundary tests. S06 Rust,
probe AI Studio, and Udio lanes are separate scopes. No live browser was used.

## First bounded extraction

Original entry1759 effective; first checkpoint entry1432. That intermediate
checkpoint was not a completed split. The structural closure below supersedes it.

| New module | Effective lines | Responsibility |
| --- | ---: | --- |
| settings.mjs |41 | Normalization, browser discovery and default WS port |
| storage.mjs |125 | Runtime-state source resolution and S3 singleton |
| websocket.mjs |169 | Loopback WebSocket framing and owned socket lifecycle |

The implementation agent minified the extracted functions and did not provide
its required AST proof. That submission was not accepted as complete. Coordinator
restored every extracted body verbatim from the saved original, retained original
parameter/local names and formatting, and removed the dead entry S3 import.
Fresh AST comparison:51 original functions,51 current,51 unique; zero changed
or missing bodies, including main and embedded callbacks. No behavior hardening
is mixed into this pure-move checkpoint.

Untouched baseline:target/effective-line-evidence/aistudio-worker-part1/before.mjs.
Eight focused tests pass against that baseline and the extracted implementation:
normalization, five WebSocket payload/header boundaries, local storage source
classification, and real CLI invalid-input response/exit1 before browser launch.
The first test draft expected exit0 incorrectly; current and baseline both proved
exit1, and the assertion now matches the existing contract. Windows command-line
length rejected an inline full-source baseline; a short data-module loader now
loads the same source and resolves package imports without executing a browser.

Node syntax checks, UTF8-without-BOM checks and scoped diff check pass. Ratchet
passes; no baseline/policy changes. Evidence logs for the paired test runs remain
in target/effective-line-evidence/udio-worker-split/aistudio-part1-{baseline,current}.log.
They are not live provider or full orchestration proof.

## Next extraction and audit

Move request/response transport, Gemini prompt/tool construction, CodeAssistant
parsing, UI session actions and generation orchestration into cohesive owners.
Retain CLI/browser/context/WebSocket ownership in main. Continue exact body proof
before separate security/resource fixes. Existing storage path/body bounds,
WebSocket framing limits/handshake validation and session cleanup require audit;
this checkpoint deliberately preserves them rather than claiming hardening.
The last immutable release predates this lane. No new build is warranted until
the integrated batch reaches its next agreed source/docs freeze.

## Structural closure checkpoint

Coordinator completed the remaining exact-body extraction. Entry now435 effective
lines and retains generation orchestration, CLI output selection and owned
browser/context/WebSocket lifecycle. All10 module owners are below500:

| Module | Effective lines | Responsibility |
| --- | ---: | --- |
| settings |42 | Shared defaults, normalization and browser discovery |
| storage |125 | Runtime-state source and storage singleton |
| websocket |169 | Framing and owned loopback socket lifecycle |
| payload |89 | Worker input and Gemini request-shape interpretation |
| diagnostics |64 | Debug snapshot sinks |
| output |49 | Stdin, JSON result and externalized text-file output |
| transport |106 | Context-request transport and page-fetch fallback |
| prompts |306 | Gemini prompt/tool bridging |
| code-assistant-parser |109 | CodeAssistant text/tool response contracts |
| ui-session |298 | Account/onboarding and prompt-surface actions |

All51 original/current/unique function bodies match exactly, including main and
browser callbacks. DEFAULT_TIMEOUT_MS has one shared owner. Independent import
review found no missing bindings or cycles; all modules import and syntax-check.
No Python contract reads this old worker monolith; probe tests remain separate.

Paired baseline/current focused suites11/11 each pass; new cases protect media
routing exclusion, prompt/stream ordering and deterministic tool response schema.
Full Node matrix176/176 and checker19/19 pass; ratchet passes. Strict now reports
1050 scanned,43 hard+77 mandatory=120 above700,38 soft. Original debt cleared
25/145 (17.2%). Evidence: extracted-node.log, baseline.log, all-node.log,
checker.log, ratchet.log and strict.log under the part1 evidence directory.

This clears one structural debt file, not all S11 hardening or release acceptance.
Raw response collection, secret-bearing debug snapshots, storage path containment,
WebSocket limits, exact-origin authentication and cancellation remain unreviewed
or unfinished. No live provider, browser profile or shared Cargo was used.

# Browser-pool transport hint ownership checkpoint

Accepted at 2026-09-14 04:04:17.864 UTC. S18 and whole-plan release remain open.
Entry 8,791 -> 8,703 effective lines. Seven functions move to a 94-line owner
with indentation-only body changes. The factory imports the input normalizer and
captures the existing hoisted uniqueStrings declaration. It returns three used
root bindings; the existing unused sanitizer stays private in the pure move.
Exact source reconstruction preserves all other root code, capture state, public
exports and request/response/websocket wiring. The separate probe is unchanged.

Four pre-extraction private fixture exports grow the fixture 72 -> 76 lines.
Nine new tests (75 lines) cover URL/endpoint boundaries, query/fragment removal,
ordered deduplication and lane presence. Three execute production capture listeners
using an EventEmitter and synthetic records, await response processing, verify
captured hints/events and detach only owned listeners. They verify wiring without
opening browsers/sockets or contacting providers. Existing offline-browser tests
are included unchanged; cleanup also runs after test failures.

Paired serialized Node suites pass 122/122, identical identities/warnings and no
skips. Package tests pass 1/1, adding only the owner path to existing byte/manifest/
checksum assertions (163 -> 164 lines). Source/encoding/syntax, checker 19/19,
ratchet and separate repository Git checks pass; all gates are terminal. No Node
formatter is configured. Independent review found no blocker. Authoritative lexer
counts take precedence over the scout's approximate physical-line count.

Evidence: target/effective-line-evidence/20260914-browser-pool-transport-owners/scope.json.
SHA-256: e0612235099f98cb9bc8e9cdc2ac5a9be4bd1107982a658b9c2d32f5e394925c.
Union 721; unchanged neighbors 718; published web assets preserved. Strict:
1,878 scanned, 16 hard, 24 mandatory, 40 soft; 40 above 700. Clearance stays
105/145 (72.4%). No Rust/dependency/policy/baseline/exception/release changes.
Gateway HEAD 4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d: 2,116 entries
(182 modified, one unstaged deletion, two staged deletions, 1,931 untracked).
Neuro HEAD bf818f0324024634bc890585efb78cc8e603d11a: 192 entries
(ten modified, 182 untracked). Census precedes documentation publication.

Next: action-contract merge/extraction, scalar/duration parsing and UI-state
inference. Keep protocol/bootstrap/media orchestration separate and establish new
paired contracts before moving code. S06 scope/cursor and final build transfer
remain reserved under GWP-20260912-01. Full strict/language/provider/release,
runtime-profile and packaged runtime/UI/Docker gates remain open. No deployment;
final target remains persistent 4200 with no persistent 4226.

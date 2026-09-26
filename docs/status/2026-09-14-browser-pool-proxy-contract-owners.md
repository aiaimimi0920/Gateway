# Browser-pool proxy discovery and HTML preparation checkpoint

Accepted at 2026-09-14 05:17:22.582 UTC. S18 and whole-plan release remain open.
Entry 8,091 -> 7,881 effective lines. Four discovery functions move to a 73-line
factory owner with indentation only; three HTML preparation functions move to a
146-line module with export modifiers only. Injected JavaScript bytes are intact.
Discovery captures the hoisted app-path resolver and initialized RPC model resolver.
Root bindings, callers, invoke assembly and launch/preview code remain unchanged
under exact source reconstruction. No new startup I/O, reverse import or registry.

Five pre-extraction exports grow fixture 87 -> 92. New discovery/HTML tests have
63/123 effective lines and add sixteen contracts. Discovery covers endpoint-form
precedence, text filtering/order, frozen inputs, first eligible text, API style,
app paths and image invoke metadata. HTML tests execute actual patched synthetic
JavaScript in node:vm with fake fetch/timers: decode/client selection, auth globals
and readiness, loopback WS upgrade, credentials, success/rejection timer cleanup,
pre-existing/in-flight cancellation, 30-second timeout, body guard/reader identity
and no-head bootstrap. New cases open no browser/network resources. Existing
offline-browser suites and cleanup remain in both complete runs.

Accepted serialized baseline/final Node suites pass 195/195 with identical test
identities/warnings and no skips. Package 1/1; two owner paths are the only contract
change (168 -> 170 lines). Source/encoding/syntax, checker 19/19, ratchet and both
Git checks pass; all gates terminal. No Node formatter is configured.

Initial baseline also passed but is excluded: a read-only reviewer reported an
unassigned test run, so possible overlap was conservatively resolved by rerunning
the complete baseline after all reviewers terminated. Original receipts remain;
accepted receipts use baseline-serialized. HTML review confirms exact bodies and
executable coverage. Claimed missing projected-test/path blockers were rejected:
projection exists, and the same fixture tests exercise monolith before extraction
and applied root/imported owners after extraction. See review and rejected-baseline
notes in the evidence directory. No synthetic/provider/deployment equivalence claim.

Evidence: target/effective-line-evidence/20260914-browser-pool-proxy-contract-owners/scope.json.
SHA-256: f9489a0a221c3a1979d1fe2f1ff11ede4968c91510552c52f610c81d6772073b.
Union 734; unchanged neighbors 730; web assets preserved. Strict: 1,891 scanned;
16 hard, 24 mandatory, 40 soft; 40 above 700. Clearance 105/145 (72.4%).
Gateway HEAD 4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d: 2,139 entries
(182 modified, one unstaged deletion, two staged deletions, 1,954 untracked).
Neuro HEAD bf818f0324024634bc890585efb78cc8e603d11a: 192 entries
(ten modified, 182 untracked). Census precedes documentation publication.

Next: invoke-contract assembly, then proxy launch/page/listener lifecycle. Measure
exact boundaries and preserve dependency initialization order before extracting.
Settled fetch caller-abort listener removal and merged cookie metadata omission
remain investigation leads. Timer cleanup tests do not prove listener removal.
Legacy permissive endpoint parsing and decoder grammar remain unchanged.
S06 scope/cursor and final build transfer remain reserved under GWP-20260912-01.
No Rust/dependency/policy/baseline/exception/release change. Full strict/language/
provider/release/runtime-profile/packaged runtime/UI/Docker gates remain open.
No deployment; persistent target stays 4200, no persistent 4226.

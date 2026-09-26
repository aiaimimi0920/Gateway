# Browser-pool injected auth bridge checkpoint

Accepted at 2026-09-14 06:31:20.297 UTC. Entry 7,221 -> 7,082 effective lines;
the injected account bridge moves to a 140-line owner with one export modifier.
It has no Node dependencies. Existing init/evaluate order, first-install account,
WebSocket wrapping, browser message handling and root factory wiring stay intact.

Fixture grows 97 -> 98; a 126-line test adds seven contracts covering account and
chrome state, duplicate/DOMContentLoaded install, one message listener, init/evaluate
failures, socket constructor/prototype, source replies, bounded previews and cyclic
messages. A real offline Chromium reload proves the registered init script and
request/reply exchange. Its test-only response listener/timer and context are cleaned.

Paired serialized Node 240/240, identical identities/warnings, no skips; package
1/1, with one owner path added (174 -> 175). Source/encoding/syntax, checker 19/19,
ratchet and both Git checks pass. All gates terminal; no Node formatter configured.
This continuation accepted header, preview and bridge owners (55, 294, 140 lines),
reducing the entry 7,560 -> 7,082, a net 478 lines. No provider/release runtime claim.

Evidence: target/effective-line-evidence/20260914-browser-pool-auth-bridge-owners/scope.json.
SHA-256: cff2c7fc24cf04da29f427f0de66a4a63cf5106a2a199ced41775700b0f0a1ad.
Union 744; unchanged neighbors 741; web assets preserved. Strict: 1,901 scanned;
16 hard, 24 mandatory, 40 soft; 40 above 700. Clearance 105/145 (72.4%).
Gateway HEAD 4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d: 182 modified, one
unstaged deletion, two staged deletions, 1,974 untracked before docs publication.
Neuro HEAD bf818f0324024634bc890585efb78cc8e603d11a: ten modified, 182 untracked.

Next: Google auth identity/header helpers, then a separately reviewed connected-client
registry/protocol owner. The existing bridge event array remains unbounded and its
message-origin policy unchanged. Probe timer/header/URL and other runtime leads
remain separate. S06 scope/cursor and final build transfer stay reserved under
GWP-20260912-01. Full strict/language/provider/runtime-profile/packaged runtime/UI/
Docker/release gates remain open. Persistent target stays 4200, no persistent 4226;
no deployment or whole-plan completion.

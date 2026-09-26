# Browser-pool preview opening and frame stamping checkpoint

Accepted at 2026-09-14 06:20:35.152 UTC. Entry 7,507 -> 7,221 effective lines;
opening and stamping move to a 294-line owner with indentation only. Capture the
existing bridge/auth callbacks and import existing normalizers. Preserve locator
order, auth/WS injection, per-frame errors, probe behavior and all root callers.

Fixture grows 95 -> 97; a 216-line test adds fourteen contracts, including a real
offline Chromium click/frame/auth/probe flow with locally fulfilled credentialed
CORS. VM callbacks check input/frame selection, bounded diagnostics, retained header
casing, URL keys, WebSocket construction/prototypes, connection patch and reinit
idempotence. UI fallback and error paths are covered. Rejected-fetch timer retention
is reproduced with a fake timer and abort assertion; no behavior fix is included.

Paired serialized Node 233/233, identical identities/warnings, no skips; package
1/1, with one owner path added (173 -> 174). Source/encoding/syntax, checker 19/19,
ratchet and both Git checks pass. Read-only review found no extraction regression.
All gates terminal; no Node formatter. No provider or packaged runtime claim.

Evidence: target/effective-line-evidence/20260914-browser-pool-preview-owners/scope.json.
SHA-256: 66f0fa67103589f2c40c89365620855604aab29cae43bb43eaccff0940b9a68b.
Union 742; unchanged neighbors 739; web assets preserved. Strict: 1,899 scanned;
16 hard, 24 mandatory, 40 soft; 40 above 700. Clearance 105/145 (72.4%).
Gateway HEAD 4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d: 182 modified, one
unstaged deletion, two staged deletions, 1,970 untracked before docs publication.
Neuro HEAD bf818f0324024634bc890585efb78cc8e603d11a: ten modified, 182 untracked.

Next: injected auth bridge, then Google auth identity/header helpers; connected-client
registries/protocol and media page leasing need separate ownership work. Existing
probe timer/header/URL and async-response/listener/cookie leads remain separate.
S06 scope/cursor and final build transfer stay reserved under GWP-20260912-01. Full
strict/language/provider/runtime-profile/packaged runtime/UI/Docker/release gates
remain open. Persistent target stays 4200, no persistent 4226; no deployment.

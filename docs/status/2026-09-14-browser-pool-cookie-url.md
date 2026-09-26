# Browser-pool cookie URL compatibility checkpoint

Accepted at 2026-09-14 02:59:35.208 UTC. This narrow fix precedes ownership
extraction; S18 and whole-plan release acceptance remain open.

Playwright rejects cookie objects containing both url and path. The embedded
storage-state normalizer previously added path before choosing URL or domain.
It now adds path only in the domain branch. URL precedence and explicit/default
domain paths are preserved. Exact projection preserves all other entry bytes.
Entry remains 9,382 effective lines; the new real-browser test has 37 lines.

Three offline browser regressions cover URL-only import, URL precedence over
legacy domain/path fields and domain cookie attributes/expiry. They verify
source-file preservation using synthetic cookies. Context/browser/temp-root
cleanup is inherited from the accepted fixtures; no provider account is used.

Initial baseline is rejected and retained: 79/88 passed. Seven app-navigation
tests failed in their shared setup when Edge launch timed out after 180000 ms;
two URL-cookie failures were intended. Subsequent observation found that launch's
PID 26400 and temporary profile absent. The timeout's broader cause is unproven.
Accepted baseline uses test-concurrency=1 with unchanged source/before.json:
86/88 pass and exactly two intended URL/path errors. Final suite passes 88/88,
with identical complete identities/warnings and no skipped tests. Use
baseline-attempt2 receipts; initial-run-gates.mjs and initial receipts remain.

Package contracts pass 1/1 before and after, with package test bytes unchanged.
Source/encoding/syntax, checker 19/19, ratchet and separate Gateway/Neuro diff
checks pass. Independent review found no blocker. All gates are terminal.
Union 713; unchanged neighbors 712; published web assets remain unchanged.
Strict: 1,870 scanned, 16 hard, 24 mandatory, 40 soft; 40 above 700. Clearance
remains 105/145 (72.4%). No Node formatter is configured.

Evidence: target/effective-line-evidence/20260914-browser-pool-cookie-url/scope.json.
SHA-256: e73994efe770872f267db2f90c8bd62edfd9fbf1fd86b8bb29e6c2976da000bb.
Gateway HEAD 4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d: 2,102 entries
(182 modified, one unstaged deletion, two staged deletions, 1,917 untracked).
Neuro HEAD bf818f0324024634bc890585efb78cc8e603d11a: 192 entries
(ten modified, 182 untracked). Census precedes documentation publication.

Next: cookie synchronization and embedded storage-state ownership extraction,
with a new structural baseline. S06 scope/cursor and final build transfer remain
reserved under GWP-20260912-01. Full strict/language/provider/release, runtime-profile
and packaged runtime/UI/Docker gates remain open. No deployment occurred; final
target remains persistent 4200 with no persistent 4226.

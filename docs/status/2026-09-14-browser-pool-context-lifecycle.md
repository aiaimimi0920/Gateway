# Browser-pool creation cleanup checkpoint

Accepted at 2026-09-14 01:44:36.897 UTC. This correctness checkpoint precedes
context ownership extraction; S18 and whole-plan release acceptance remain open.

Baseline reproduction used the unchanged 10,084-line entry. Of 61 Node tests,
58 passed and three failed: owned/CDP newContext failures omitted browser.close,
and a real headless Edge remained connected after invalid storageState JSON.
The test always closed its owned browser in finally, including on baseline failure.

The existing cleanup try/catch now encloses setup as well as page acquisition.
The inner profile retry cleanup catch is removed, so launch/page failures share
one cleanup path. Original errors survive context/browser cleanup rejections;
managed clones are removed exactly once while source profile bytes remain intact.
Entry decreases 10,084 -> 10,074 effective lines. The focused in-place change
repairs the lifecycle before extracting its owner and adds no oversized-file debt.
Four fixture exports add four lines; new test files contain 81 and 32 lines.

Final complete Node suite passes 61/61, no skips. Paired synthetic nested-package
contracts pass 1/1 with the original package test unchanged. Source projection,
UTF-8 without BOM, syntax, checker 19/19, ratchet and separate Gateway/Neuro diff
checks pass. There is no configured Node formatter. Strict remains red: 1,861
scanned, 16 hard, 24 mandatory, 40 soft; 40 above 700, clearance 105/145 (72.4%).

An independent review raised a CDP termination concern. The installed Playwright
CDP implementation closes its transport, and a separate real CDP failure gate
confirms the attached handle disconnects while the original owned browser stays
connected and its page evaluates 1 + 1 to 2. Both browser handles and the dedicated
user-data directory are closed/removed in finally. No external browser was used.
Borrowed-CDP page-failure semantics are unchanged and are not redesigned here.

The 704-input union preserves 703 neighbors and published web assets. Storage/CDP
fixture census is zero; all owned gates are terminal. No Rust, live service,
dependency, checker policy, adoption baseline, exception or release was changed.
Evidence capture initially hit a generated-string escaping error; the duplicate
tail was removed and its acceptance script then completed successfully. Production
source and completed test results were unaffected.

Evidence: target/effective-line-evidence/20260914-browser-pool-context-lifecycle/scope.json.
SHA-256: 2d62f639510281f90d1d6e7cd3a6b5b299549f28a086b99eafbc76cbe2b6653a.
Gateway HEAD 4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d: 2,087 entries
(182 modified, one unstaged deletion, two staged deletions, 1,902 untracked).
Neuro HEAD bf818f0324024634bc890585efb78cc8e603d11a: 192 entries
(ten modified, 182 untracked). Census precedes documentation publication.

Next: context creation/reuse/registry/disposal/capacity ownership, with registry
and concurrency regression coverage. S06 scope/cursor and final build transfer
remain reserved under GWP-20260912-01. Full language/provider/release and packaged
runtime/UI/Docker gates remain open. Final persistent target remains 4200 with no
persistent 4226; no deployment occurred in this checkpoint.

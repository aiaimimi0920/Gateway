# Browser-pool preview probe timer cleanup checkpoint

Accepted at 2026-09-14T07:41:19.192Z. Entry 6717 -> 6717 effective lines;
owner 295, fixture 108, test 246, package contract 179.

Two focused regressions reproduce retained timers on rejected and synchronous
fetch failures. Settlement now clears both paths without changing failure
diagnostics. The existing early clear before response.text remains intact; new
contracts protect active abort and successful headers with a deferred body.
Owner grows 294 -> 295 effective lines; root and package contract are unchanged.
Independent design review and coordinator source review found no regression.

Focused regressions 0/2 before -> 2/2 after; full Node 274/274, no skips; package
1/1. Source/encoding/syntax, checker 19/19, ratchet and both Git checks pass.
All gates terminal; no Node formatter. No provider or packaged runtime claim.

Evidence: target/effective-line-evidence/20260914-browser-pool-preview-timer-hardening/scope.json.
SHA-256: 1be849ef41164e8ac9cbf4da4b43976bbf53c4b3aa225bbd18a22e103888b8f7.
Union 751; unchanged neighbors 750; web assets preserved. Strict: 1907 scanned;
16 hard, 24 mandatory, 40 soft; 40 above 700. Clearance 105/145 (72.4%).

Next: media page lease ownership; retain close ordering and the existing bounded-wait
contract. Other URL admission and protocol lifecycle leads remain separate.
S06 scope/cursor and final build transfer stay reserved under GWP-20260912-01.
Full strict/language/provider/runtime-profile/packaged runtime/UI/Docker/release
gates remain open. Persistent target stays 4200, no persistent 4226; no deployment.

# Browser-pool input ownership

Owner: resumed coordinator. State: accepted at 2026-09-14 00:15:37.332 UTC.
Entry 10,462 -> 10,424 effective lines; pure owner 44, fixture 54, new contracts
74 and package contract 155. Paired browser-pool tests pass 40/40; paired nested
package tests pass 1/1. Exact source/encoding/syntax, checker 19/19, ratchet and
both Git checks pass. The 693-input union preserves 690 neighbors and web assets.
Strict: 1,850 scanned, 16 hard, 24 mandatory, 40 soft; 40 above 700. Clearance
remains 105/145 (72.4%). S18 entry and whole-plan release remain open.

Evidence: target/effective-line-evidence/20260914-browser-pool-input-owners/scope.json.
SHA-256: 735aa41474724457612dfdfbb1d0bad4b2d20f8e43e5508153dc53e8d484ee30.
[Checkpoint report](../../status/2026-09-14-browser-pool-input-owners.md).

Accepted preparation/design:
Predecessor: accepted pipeline-send scope, SHA-256
4ee7494e5c7cbfce0c8cbe3a284add51292f1ac35915a922c4b50013378c2e7f.

Scope: move normalizeString, normalizeObject, scopeGeminiUrlToAuthUser and
applyGeminiAccountScope intact from scripts/gemini-canvas-browser-pool.mjs to
adjacent gemini-canvas-browser-pool-input.mjs. The root imports all four names;
shared string normalization has one definition, with no backwards dependency.
normalizeObject retains its connected-client input semantics. It is not used to
replace applyGeminiAccountScope's different array/object acceptance behavior.
All root bytes outside the exact four-function block and import wiring stay fixed.

The original entry is 10,462 effective lines. This is the first incremental
ownership boundary; the residual entry remains debt and cannot be reported as
S18 complete. New/extracted owners must be at most 500 effective lines.

Before production edits, extend the existing private-export test fixture only
to expose the two normalization functions. Freeze new input boundary contracts
and run all browser-pool Node tests plus the existing nested package contract.
Retain existing fixture reexports and call sites; paired tests must demonstrate
ESM import/reexport resolution without changing entry startup behavior.

Docker recursively copies scripts; the release packager recursively copies and
hashes ordinary .mjs modules. Add root/resource/input modules to the existing
nested package contract and execute the packaged pure input module. This uses
temporary fixture artifacts with synthetic binaries, not a production release.
No Dockerfile, packager, dependency, manifest schema or runtime config change.

Capture prior input hashes, web assets, exact source projection, tests and fresh
syntax/checker/ratchet/strict/Git evidence. No Node formatter is configured in
scripts/package.json; preserve original formatting and check syntax/hygiene.
No Rust edits, Cargo operation or provider/browser launch is needed for this pure
boundary. S06 and final build transfer remain reserved under GWP-20260912-01.

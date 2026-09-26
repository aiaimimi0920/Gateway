# Browser-pool program/share navigation ownership checkpoint

Accepted at 2026-09-14 03:40:55.040 UTC. Incremental S18 and whole-plan release
acceptance remain open.

Entry 9,143 -> 8,791 effective lines. Five functions from three disjoint ranges
move to a 359-line owner: program URL resolution, program navigation, share
navigation, share-surface wait and share-entry following. Bodies change only
indentation. The factory captures root log/button-snapshot functions; input/app
helpers are direct imports without reverse dependency or startup I/O. Exact source
reconstruction preserves root exports and all remaining callers. Loopback client
bootstrap, media-page acquisition, page adoption, old-page close and network
capture ownership stay in existing callers.

One pre-extraction private fixture export grows the fixture 71 -> 72 lines.
New navigation/share-follow tests have 61/42 lines. Fourteen new contracts include
twelve real-browser cases: closed pages, same-path draft preservation, concrete
navigation, program auth errors, missing share ID, account slots, incidental
sign-in text, popup/same-page following and bounded diagnostics. Two input tests
cover program target precedence and missing target behavior. The auth case uses
an already-loaded local sign-in document, not a provider-triggered redirect.

The real popup test leaves entry.page and the original page intact, then closes
the returned popup. Context teardown covers assertion failures. Higher-level
adoption/capture transfer is outside these new tests. Offline contexts, blocked
service workers and local route fulfillment isolate all requests. Node time and
explicit waits are simulated; DOM, locators, navigation and popup events are real.
This is not latency, provider account or production package validation.

Paired complete Node suites pass 113/113 with identical identities/warnings and
no skips, using test-concurrency=1. Existing navigation timeout/error/retry and
delayed share-follow contracts are included. Package contracts pass 1/1; the
contract grows 162 -> 163 solely by adding the owner to existing byte/manifest/
checksum assertions. Source/encoding/syntax, checker 19/19, ratchet and both Git
checks pass. No Node formatter is configured. All gates are terminal, and owned
context/browser/temporary-module cleanup passes. Independent reviews found no blocker.

Evidence: target/effective-line-evidence/20260914-browser-pool-navigation-owners/scope.json.
SHA-256: 711f13242b2266a526b9a0fd1d5440569bb09f6947a7083a802b254bf685c946.
Union 719; unchanged neighbors 716; published web assets preserved. Strict:
1,876 scanned, 16 hard, 24 mandatory, 40 soft; 40 above 700. Clearance remains
105/145 (72.4%). No Rust/dependency/policy/baseline/exception/release changes.
Gateway HEAD 4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d: 2,112 entries
(182 modified, one unstaged deletion, two staged deletions, 1,927 untracked).
Neuro HEAD bf818f0324024634bc890585efb78cc8e603d11a: 192 entries
(ten modified, 182 untracked). Census precedes documentation publication.

Next: inspect transport-hint URL parsing/merging and action/UI contract parsing
as small pure boundaries. Measure exact definitions and search untracked tests
with direct file tools before deciding scope; a tracked-only search is insufficient.
Keep browser-side bridge/bootstrap and higher-level media orchestration separate.
S06 scope/cursor and final build transfer remain reserved under GWP-20260912-01.
Full strict/language/provider/release, runtime-profile and packaged runtime/UI/
Docker gates remain open. No deployment occurred; final target remains persistent
4200 with no persistent 4226.

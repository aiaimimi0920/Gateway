# Browser-pool app preparation ownership

Owner: resumed coordinator. Accepted at 2026-09-14 02:31:29.269 UTC.
Entry 9,783 -> 9,382; owner 414, fixture 69, new navigation/auth tests 59/70,
shared fixture 52 and package contract 161 effective lines. Paired accepted Node
suites pass 85/85 and package tests 1/1. Source/syntax/checker/ratchet and both
Git checks pass. All gates are terminal; owned contexts and browsers close.

Move ensureAppPage, hasPromptTextbox, clickFirstVisible, consent/signed-out/app
body predicates, shouldTreatGeminiPageAsAuthBlocked, reusable-surface inspection,
attached app discovery and tryResolveGoogleConsent into one adjacent owner.
Preserve bodies with indentation/export modifiers only. One factory captures
existing root log/auth-user/cookie-sync/button-snapshot callbacks; retain three
public root exports and all remaining program/share/media/context/cookie owners.

Add pre-extraction real-browser contracts for navigation/account/reuse and
consent/auth/cookie/grace behavior. Use locally fulfilled HTML, offline contexts
and blocked service workers. DOM, locators, navigation and cookies remain real;
virtualize only Node time and explicit waits. Use synthetic cookies only. Context
and browser teardown and mock restoration must run after tests.

Rejected baseline 84/85 remains as evidence: fixture HTTP 302 bypassed later
route handling and timed out. Accepted before2/attempt2 replaces that fixture
with client document navigation and offline mode, without production changes.
Use before2.json and web-assets2.json when consuming this checkpoint's chain.

Evidence: target/effective-line-evidence/20260914-browser-pool-app-owners/scope.json.
SHA-256: 8121919a38fa798ca002ee5efe457daf19cfc94fa1a33ab7528becadb6587150.
Predecessor: 5341b8c4caa7be4bcccc0ab51edf5eea4cb6d080f93da55a635381837ab29d42.
Union 712; unchanged neighbors 709. Strict: 1,869 scanned; 16 hard, 24 mandatory,
40 soft; 40 above 700. Clearance stays 105/145 (72.4%). S18/release remain open.
[Checkpoint report](../../status/2026-09-14-browser-pool-app-owners.md).

Next proposed scope is cookie synchronization/storage-state hydration, followed
by concrete program/share navigation. S06 and final build transfer remain
reserved under GWP-20260912-01; no deployment is performed in this lane.

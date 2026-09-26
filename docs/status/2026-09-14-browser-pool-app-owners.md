# Browser-pool app preparation ownership checkpoint

Accepted at 2026-09-14 02:31:29.269 UTC. Incremental S18 and whole-plan release
acceptance remain open.

Entry 9,783 -> 9,382 effective lines. Ten app preparation/surface/auth/consent
functions move into a 414-line owner. Bodies change only indentation and four
surface helper export modifiers. A single factory captures the root logger,
auth-user inference, cookie synchronization and button snapshot functions. Those
callbacks are hoisted declarations. The owner has no reverse import, startup
I/O or mutable registry. Three root public exports and remaining program/share/
media/context/cookie implementations and call sites are preserved exactly.

Two private fixture exports are added before baseline (67 -> 69 lines). New
navigation/auth contracts have 59/70 lines; their shared fixture has 52 lines.
Fifteen new contracts use real headless browser DOM, locators, evaluation,
navigation and cookies. They cover closed pages, same-path draft preservation,
account slots, attached app reuse/discovery, initial/grace sign-in redirects,
consent locator and DOM fallback clicks, the 120-button diagnostic bound, real
synthetic cookie hydration, one-attempt recovery and bounded/delayed auth grace.
Node time and explicit waits are simulated; this is not latency evidence.

The initial baseline is rejected and retained: 84/85 tests passed, while a
fixture-generated HTTP 302 bypassed interception for its destination and reached
the two-second navigation timeout. Installed Playwright network code confirms
that redirected requests bypass user routes. The initial isolated context had
no account data; no offline-isolation claim is made for that rejected attempt.
Production code was unchanged. The corrected fixture uses offline contexts,
blocked service workers, locally fulfilled HTML and client document navigation;
the redirect test asserts both local route paths. Initial evidence remains in
before.json/before/ and browser-pool-baseline.*. Accepted evidence uses
before2.json/before2/, web-assets2.json and baseline-attempt2 gates. Two evidence
path errors while preparing that second snapshot were corrected without replacing
the initial source or logs.

Paired accepted complete Node suites pass 85/85 with identical identities/warnings
and no skips. All per-test contexts and owned browsers close in hooks, and mocks
restore automatically. Paired synthetic nested-package tests pass 1/1; the package
contract grows 160 -> 161 lines solely by adding the app owner path, retaining
original byte/manifest/checksum assertions. This does not prove provider behavior,
production package execution or real account consent completion.

Exact source projection, UTF-8 without BOM, syntax, checker 19/19, ratchet and
separate Gateway/Neuro diff checks pass. No Node formatter is configured. The
712-input union preserves 709 neighbors and published web assets. All gates are
terminal and storage fixture/testable-module cleanup checks pass. Independent
source and fixture reviews found no blocker. No Rust, dependency, checker policy,
adoption baseline, exception, production release or sibling source changed.

Strict: 1,869 scanned, 16 hard, 24 mandatory, 40 soft; 40 above 700. Clearance
remains 105/145 (72.4%). The browser-pool entry requires further splitting.

Evidence: target/effective-line-evidence/20260914-browser-pool-app-owners/scope.json.
SHA-256: 8121919a38fa798ca002ee5efe457daf19cfc94fa1a33ab7528becadb6587150.
Gateway HEAD 4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d: 2,099 entries
(182 modified, one unstaged deletion, two staged deletions, 1,914 untracked).
Neuro HEAD bf818f0324024634bc890585efb78cc8e603d11a: 192 entries
(ten modified, 182 untracked). Census precedes documentation publication.

Next: measure and isolate cookie parsing/synchronization and embedded storage-state
hydration before concrete program/share navigation. Read exact definitions and
existing runtime tests before choosing the next write scope. S06 implementation/
cursor and final build transfer stay reserved under GWP-20260912-01. Full strict,
language/provider/release, runtime-profile governance and packaged runtime/UI/
Docker gates remain open. No deployment occurred; final persistent target remains
4200 with no persistent 4226.

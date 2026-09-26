# Browser-pool context ownership checkpoint

Accepted at 2026-09-14 01:57:17.665 UTC. Incremental S18 and whole-plan release
acceptance remain open. This structural batch follows the separately accepted
creation cleanup fix GWP-20260914-14.

Entry 10,074 -> 9,783 effective lines. The reuse inspector and five lifecycle
functions move into a 305-line context owner. Bodies change only indentation.
One factory owns the contexts/initializingContexts maps and captures the existing
root logger, constants, hoisted fixture predicate and profile callbacks. The root
public inspector export, connected-client maps, health count, eviction timer and
caller busy/lease/lastUsedAt protocol are preserved. No startup I/O is added.

Two private fixture exports are added before baseline (65 -> 67 lines). New
context/capacity contracts have 94/71 lines and their shared fixture has 47.
They cover concurrent creation deduplication, surviving-page adoption, stale
replacement, late-event registry identity, ownership removal before awaiting
cleanup, failed/idempotent disposal, idle/LRU eviction, busy entries and in-flight
capacity reservations. Fixtures restore environment/logging and require empty
context and pending maps after cleanup.

Paired complete Node suites pass 70/70 with identical identities/warnings and no
skips. The inherited real invalid-storageState browser cleanup regression passes
on both sides. A separate real CDP gate verifies failure disconnection as well as
successful creation, cache reuse and disposal. The original owned headless browser
stays connected and its page evaluates 1 + 1 to 2 after attached disposal. Both
handles and the dedicated temporary user-data directory are cleaned in finally.
No external browser, account, provider or live profile is used.

The nested package contract grows 159 -> 160 lines solely by adding the context
module. Paired tests pass 1/1, preserving original byte/manifest/checksum assertions.
These synthetic packages do not establish production package or UI execution.

Exact source projection, UTF-8 without BOM, syntax, checker 19/19, ratchet and
separate Gateway/Neuro diff checks pass. No Node formatter is configured. The
708-input union preserves 705 neighbors and published web assets. All owned gates
are terminal and storage/CDP fixture census is zero. Independent read-only review
found no structural blocker. No Rust, dependency, policy, baseline, exception,
production release or sibling source changes occurred. An extra blank line in
the initial patch was rejected by projection verification and removed before the
candidate was accepted or its final tests ran.

Strict: 1,865 scanned, 16 hard, 24 mandatory, 40 soft; 40 above 700. Clearance
remains 105/145 (72.4%). The browser-pool entry still requires further splitting.

Evidence: target/effective-line-evidence/20260914-browser-pool-context-owners/scope.json.
SHA-256: 5341b8c4caa7be4bcccc0ab51edf5eea4cb6d080f93da55a635381837ab29d42.
Gateway HEAD 4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d: 2,093 entries
(182 modified, one unstaged deletion, two staged deletions, 1,908 untracked).
Neuro HEAD bf818f0324024634bc890585efb78cc8e603d11a: 192 entries
(ten modified, 182 untracked). Census precedes documentation publication.

Next proposed boundary: ensureAppPage and cohesive app-surface/auth/consent
helpers. Read-only localization places ensureAppPage at root lines 1895-2114,
surface predicates at 2374-2469 and consent handling at 2497-2607. Main must read
and measure these exact definitions with the authoritative lexer before choosing
the final scope. Keep cookie/runtime-state hydration and concrete program/share
navigation separate. Add page preparation/consent regression coverage before
moving bodies; localization estimates are not an accepted line-count measurement.

S06 implementation/cursor and final build coordination remain reserved under
GWP-20260912-01. Full strict/language/provider/release, runtime-profile governance
and packaged runtime/UI/Docker gates remain open. No deployment occurred; final
persistent target remains 4200 and no persistent 4226.

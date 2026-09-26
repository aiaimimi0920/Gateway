# Stock signal cooldown time bounds

Accepted: 2026-09-12 20:29:42 UTC. Gateway-only hardening checkpoint.

The signal cooldown helper added an arbitrary nonnegative i64 policy duration
to a parsed timestamp. The frozen runtime contract reproduced two panics at that
addition: i64::MAX seconds and a finite 40,000,000,000,000-second duration both
exceed the supported calendar. Its baseline was six passes and two failures.

The helper now uses checked_add and retains suppression when a positive deadline
cannot be represented. The clock is still read before the addition. Signal-key
admission, missing/malformed timestamp handling, negative-duration clamping and
representable future/expired decisions remain unchanged. All eight frozen tests
pass after the fix; their bytes are identical to the pre-fix contract.

src/credential_stock/signals.rs grows from 169 to 174 effective lines, including
two test-wiring lines. The test owner is 72 effective lines. Exact comparison
preserves all five function signatures and four unrelated complete bodies;
removing only the fix and test wiring reconstructs the entire original owner.
All 284 neighboring inputs are unchanged. Public views, SQL/Redis protocols and
the stock publication flow were not modified.

Fresh serialized gates pass: frozen contract 8/8; original stock units 15/15 in
both phases; cargo check --offline --locked --all-targets; scoped formatter;
checker 19/19; adoption ratchet; exact source/UTF-8/no-BOM proof; and both
repositories' git diff --check. Global formatting still reports only the two
unchanged S06 runtime-mirror files. All Cargo gates are terminal. The arithmetic
fixtures need no network, Redis, database or provider credentials.

Strict scans 1,598 files: 30 hard, 29 mandatory and 40 soft; 59 remain above 700.
Clearance stays 86/145 (59.3%). No dependency, policy, baseline or exception changed.
Immutable evidence is target/effective-line-evidence/20260912-stock-cooldown-safety/scope.json.
It binds the frozen snapshots, paired test names/results, compiler/checker logs,
inventories, source/neighbor proof and separate repository observations.

At capture, Gateway has 1,763 dirty entries: 173 modified, one unstaged deletion,
two staged deletions and 1,587 untracked. Neuro has 192: ten modified and 182
untracked. Both HEADs and inherited deletions remain unchanged. Historical
accepted scopes stay immutable.

Credential-pool automation ownership is the next structural review. Broader stock
publication bounds and refill lease/delivery/notification lifecycle remain
separate. S06 ownership, GWP-20260912-01, runtime-profile governance and the final
freeze/build transfer remain pending. No immutable release, packaged runtime/UI
acceptance or persistent deployment was performed.

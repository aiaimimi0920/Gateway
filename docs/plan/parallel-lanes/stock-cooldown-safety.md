# Stock signal cooldown time bounds

Owner: parallel coordinator. State: hardening_green. Started: 2026-09-12 UTC.
Accepted: 2026-09-12 20:29:42 UTC.

Scope: src/credential_stock/signals.rs and a frozen cooldown contract under its
private test directory. The pre-fix cooldown helper added an arbitrary nonnegative
i64 policy duration to a parsed timestamp without checking the calendar range.
Reproduce overflow before changing production behavior.

Preserve signal-key admission, absent/invalid timestamp handling, negative-duration
clamping, representable deadline decisions and all other signal functions. A
positive deadline beyond the supported calendar cannot already have expired;
overflow should retain cooldown suppression instead of terminating the sweep.
Keep all original stock tests, public signatures, fields, SQL/Redis protocols and
neighboring inputs unchanged.

Require frozen pre-fix runtime assertions, paired original units, fresh all-targets,
scoped formatter, checker 19/19, ratchet, strict inventory, exact source/neighbor/
encoding proof and both Git checks. No Redis, database or provider connection is
needed for this deterministic arithmetic boundary.

Signal publication bounds and refill lease/delivery/notification lifecycle remain
separate. S06 ownership, GWP-20260912-01, runtime-profile governance and final
freeze/build transfer are unchanged. No release or persistent deployment is in
this checkpoint.

Evidence: target/effective-line-evidence/20260912-stock-cooldown-safety/.

The frozen eight-test contract reproduced two timestamp-overflow panics in the
original addition. Replacing only that addition with checked_add keeps an
unrepresentable positive deadline suppressed. The contract now passes 8/8;
original stock tests pass 15/15 in both phases. Five signatures, four unrelated
function bodies and 284 neighboring inputs are unchanged. Removing the reviewed
fix and test wiring reconstructs the complete original owner. The production
owner is 174 effective lines and the frozen test owner is 72.

Fresh all-targets, scoped formatter, checker 19/19, ratchet, source/encoding proof
and both Git checks pass. All Cargo gates are terminal. Global formatting still
reports only the two unchanged S06 runtime-mirror files. Strict scans 1,598 files:
30 hard, 29 mandatory and 40 soft; 59 remain above 700. Clearance stays 86/145
(59.3%). No dependency, policy, baseline or exception changed.

The accepted scope binds snapshots, paired test cases/results, gate logs,
inventories, source proof and separate repository observations. See
[acceptance report](../../status/2026-09-12-stock-cooldown-safety.md).
Credential-pool automation ownership is the next structural review.

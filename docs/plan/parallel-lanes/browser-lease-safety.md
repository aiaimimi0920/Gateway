# Browser lease ownership and time bounds

Owner: parallel coordinator. State: focused hardening verified; final release pending.
Started: 2026-09-12.

The pre-fix lease release reads a lock, compares it and deletes it in separate
commands, then changes the slot even when the lock belongs to another lease.
Acquisition casts an untrusted u64 TTL to i64 and adds the retention TTL without
overflow checks. Reproduce both boundaries through the public runtime functions
against a dedicated, guarded Redis instance before changing production code.

Preserve current-owner warm/cooling release, slot metadata, released lease records,
default/minimum/representable TTLs and original error behavior for malformed slot
records. A superseded or expired lease must not change slot availability or remove
another owner's lock. Invalid time/retention values must fail before lock creation.
Keep release updates atomic with lock ownership and the observed slot version;
bound contention retries rather than overwriting concurrent heartbeat metadata.

Scope: browser_executor_runtime entry wiring, leases.rs, timestamps.rs, storage
helper visibility if required, a cohesive release-commit owner and focused runtime
contracts. Existing support fixtures and all earlier accepted scopes stay unchanged.
Capture fixed contracts before the production fix; require paired real-Redis
regressions, original browser unit tests, fresh all-targets, scoped formatter,
checker 19/19, ratchet, strict inventory, source/neighbor proof and both Git checks.

The Redis fixture uses a random loopback port and a per-run guard. It must never
flush a shared database; remove only its own container and owned temporary state.
Lease acquisition persistence atomicity, heartbeat admission, broader Redis/index
bounds and other splitter lifecycle limits remain separate. S06 ownership and
pending GWP-20260912-01 freeze/build coordination are unchanged.

Evidence: target/effective-line-evidence/20260912-browser-lease-safety/.

Red baseline captured 2026-09-12 15:51:04 UTC: public real-Redis contracts ran
10 tests, with four passing and six failing on runtime assertions or timestamp
panics. Original browser unit tests passed 3/3. The dedicated Redis container and
fixture directories were removed, the live Gateway container was unchanged, and
all 229 snapshotted inputs still matched before the production patch.

The fix adds one private atomic release owner, a four-attempt raw-slot CAS loop
and checked retention/timestamp arithmetic. Four deterministic Redis-script
contracts additionally exercise a changed heartbeat, replacement ownership,
missing-slot presence and regained ownership between the snapshot and commit.
The ten public regression contracts remain frozen at their pre-fix hashes.

Accepted 2026-09-12 16:29:22 UTC. The ten public runtime contracts pass 10/10;
the four deterministic atomic-commit contracts pass 4/4 and the original browser
unit tests pass 3/3. Fresh all-targets, scoped formatter, checker 19/19, ratchet,
source/encoding proof and both Git checks pass. All Cargo gates are terminal.
All eight Rust files are at most 215 effective lines; three public signatures,
four unrelated bodies, public entry/views and 225 neighboring inputs are preserved.
The dedicated Redis containers and fixture directories are cleaned; live Gateway
is unchanged. Global formatting still reports only the two unchanged S06 files.

Strict: 1,565 scanned; 31 hard, 31 mandatory, 40 soft; 62 above 700. Clearance
remains 83/145 (57.2%). Provider quota ownership is next. S06 ownership and final
freeze/build coordination remain unchanged. No release or persistent deployment
was performed. [Acceptance report](../../status/2026-09-12-browser-lease-safety.md).

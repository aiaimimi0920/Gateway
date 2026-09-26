# Browser lease ownership and time bounds

Accepted: 2026-09-12 16:29:22 UTC. Scope: Gateway only.

The fixed public real-Redis contract advances from four passes and six failures
to 10/10. The red failures were runtime assertions and timestamp panics, not build
or fixture failures. Both regression files retain their pre-fix hashes. The
original browser unit tests pass 3/3 before and after the production changes.

Lease release now commits the slot update, released record and owned-lock deletion
in one Redis script. Ownership and the exact observed slot value are checked before
any writes. A superseded or expired lease changes only its own release record.
Malformed owned-slot JSON fails before unlocking. Missing slots still permit
owned-lease cleanup. Up to four attempts retry slot/ownership changes; continued
contention returns conflict instead of overwriting heartbeat metadata.

Acquisition checks retained-TTL addition, u64-to-i64 conversion, timestamp addition
and RFC3339 representation before creating a lock. Invalid values return HTTP 400.
Default 300 seconds, the 30-second minimum, a valid 86,400-second TTL and the
3,600-second record retention behavior remain covered by actual Redis TTL checks.

| Rust owner | Before | After |
| --- | ---: | ---: |
| browser_executor_runtime.rs | 214 | 215 |
| browser_executor_runtime/leases.rs | 260 | 215 |
| browser_executor_runtime/timestamps.rs | 12 | 16 |
| browser_executor_runtime/storage.rs | 103 | 103 |
| browser_executor_runtime/lease_release.rs | new | 106 |
| browser_executor_runtime/lease_release/tests.rs | new | 159 |
| tests/browser_executor_runtime_contract.rs | fixed contract | 187 |
| tests/browser_executor_runtime_contract/fixture.rs | fixed fixture | 143 |

All eight owners are at most 215 effective lines. The public views and entry paths
remain intact; only one private module is wired into the entry. Three public
function signatures, four unrelated function bodies and 225 neighboring inputs
are preserved. Storage changes only the family-local visibility of deserialize_json.

The Redis script receives keys and values as arguments; it performs no string-built
commands or JSON conversion. Rust retains typed slot validation and metadata.
Retries are bounded and create no background tasks. Dedicated test Redis instances
are protected by per-run guards and random loopback ports. Their exact container
IDs and exclusively owned temporary directories are cleaned; the existing live
Gateway container ID/status/start time remain unchanged.

Fresh verification passed:

- Public runtime contract: 10/10, explicitly executed with `--ignored`.
- Deterministic Redis-script concurrency contracts: 4/4, explicitly executed.
  They cover changed heartbeat data, replacement ownership, slot appearance and
  regained ownership between observation and commit.
- Original browser unit tests: 3/3 in both phases.
- `cargo check --offline --locked --all-targets`, scoped formatter, checker 19/19,
  adoption ratchet, source/encoding proof and both repositories' `git diff --check`.

Global formatting still reports only the unchanged S06 runtime-mirror production
and test files. Strict scans 1,565 files: 31 hard, 31 mandatory and 40 soft;
62 remain above 700. Clearance remains 83/145 (57.2%). No baseline, exception,
dependency or checker policy changed. All Cargo gates are terminal.

Immutable evidence: `target/effective-line-evidence/20260912-browser-lease-safety/scope.json`.
It binds the fixed regressions, candidate hashes, before/after test logs, runtime
cleanup receipts, inventory and Git observations. At capture, Gateway has 1,721
dirty entries (172 modified, one unstaged deletion, two staged deletions, 1,546
untracked); Neuro has 192 entries (ten modified, 182 untracked). Both HEADs and
inherited staged deletions are unchanged.

Provider quota ownership is the next independent structural review. Browser lease
acquisition persistence atomicity, heartbeat admission and broader Redis/index
bounds remain separate. GWP-20260912-01, S06 ownership and final freeze/build
transfer remain pending. This checkpoint does not establish packaged runtime/UI
acceptance. No immutable release or persistent deployment was performed.

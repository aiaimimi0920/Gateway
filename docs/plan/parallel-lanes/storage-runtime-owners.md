# Object storage and provider runtime ownership

Owner: parallel coordinator. State: accepted.
Started: 2026-09-12.

Scope is src/object_storage.rs, src/provider_runtime.rs and new private owners.
The complete entries and tests were read, together with the existing body-limit
owner, provider_account_probe.rs, representative callers and operations notes.
Pre-edit entries measured 790 and 947 effective lines.

Object storage keeps its public contract/types, singleton and key/storage-mode
helpers in the entry. Extract configuration, object IO, listing, local path
validation/confinement and readiness. Preserve all eight inherent methods,
environment precedence, S3 request construction, bounded body reads, path
validation, listing order and readiness cleanup. The existing body_limit.rs
owner remains unchanged. Keep the tests module path, especially the hard-coded
Windows junction-cycle subprocess test name.

Provider runtime keeps public outcome/report types and the small shared
persistence sanitizer in the entry. Extract management/sweeps, runtime recording,
probe locks, HTTP probe policy and console payload probing. Preserve public paths,
fixed model contents, strict/legacy distinctions, error sanitization, Redis
commands/order/TTLs, lock behavior and all database side effects. The unchanged
provider_account_probe.rs owner still resolves its real parent imports.

All new/extracted owners must remain at most 500 effective lines. Changes to
private visibility are restricted to actual cross-owner uses. Preserve every
original production function/type/constant/method and all tests/fixtures.
Listing work bounds, S3 pagination progress and probe-lock lifecycle/atomicity
are separate audits; this extraction does not change their behavior.

Capture current sources and accepted neighbors before editing. Run fresh
default-feature object-storage and provider-runtime baselines, then paired final
tests, all-targets, scoped formatter, checker tests, ratchet, strict inventory,
encoding and independent Git checks. Baselines include real temporary local
storage and Windows junction cases, not live S3, Redis or provider calls.

Evidence: target/effective-line-evidence/20260912-storage-runtime-owners/.
The immutable scope was captured at 2026-09-12 04:49:10 UTC. Entries now measure
84/55 effective lines; all 14 files are at most 256. Exact comparison preserves
50 free functions, eight methods, nine types, one constant, 31 public entry paths,
the crate-local path, 19 extracted tests and two fixtures. Six unchanged body-limit
tests are included in the gate. All 159 neighboring inputs remain unchanged.

Fresh paired storage 14/14 and runtime 11/11, default-feature all-targets,
scoped formatter, checker 19/19, ratchet, source/encoding proof and both Git
checks pass. All native gates are terminal. Strict scans 1,528 files: 31 hard,
35 mandatory and 40 soft; 66 remain above 700. Clearance is 79/145 (54.5%).
Global formatting still reports only the two unchanged S06 files.
[Acceptance report](../../status/2026-09-12-storage-runtime-owners.md).

S06 retains its source and original cursor. GWP-20260912-01 and the explicit
source/docs freeze and release-build transfer remain pending. No dependency,
profile, policy, baseline or release change is included.

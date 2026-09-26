# Object storage and provider runtime ownership

Accepted structural checkpoint: 2026-09-12 04:49:10 UTC.
Owner: parallel coordinator. The overall refactor plan remains in progress.

The object-storage entry decreased from 790 to 84 effective lines and the
provider-runtime entry from 947 to 55. Both public contracts remain in their
original namespaces. The existing bounded-reader and account-probe owners stay
hash-identical, as do the protected external callers. Required parent symbols
remain available to the account-probe owner.

## Accepted structure

| File | Effective lines |
| --- | ---: |
| src/object_storage.rs | 84 |
| object_storage/configuration.rs | 86 |
| object_storage/listing.rs | 106 |
| object_storage/local_paths.rs | 118 |
| object_storage/object_io.rs | 124 |
| object_storage/readiness.rs | 78 |
| object_storage/tests.rs | 225 |
| src/provider_runtime.rs | 55 |
| provider_runtime/console_probe.rs | 127 |
| provider_runtime/http_probe.rs | 159 |
| provider_runtime/management.rs | 180 |
| provider_runtime/probe_lock.rs | 53 |
| provider_runtime/recording.rs | 256 |
| provider_runtime/tests.rs | 165 |

All 14 scoped files are at most 256 effective lines.

## Preservation and review

Exact comparison preserves 50 free functions, eight inherent methods, nine types
and one constant. All 31 public entry paths, the crate-local local_object_path
path and method signatures/visibility remain intact. Nine helpers and three
private types gain only pub(super) visibility for actual cross-owner uses.

All 19 extracted tests, two runtime fixtures and their original module paths are
preserved. The six existing body-limit tests remain in their unchanged owner.
The Windows junction-cycle subprocess still invokes its exact original test path.
The 161-input pre-edit snapshot and all 159 neighboring inputs remain intact.

Configuration preserves environment precedence, credentials construction, S3
request configuration and the singleton. Object IO retains size admission,
bounded reads, serialization, path checks and operation order. Local-path owners
retain key validation and canonical containment checks. Listing retains traversal,
cycle detection, S3 pagination and sorting. Readiness retains deadlines, probe
content and cleanup behavior.

Runtime owners retain management/sweep sequencing, status writes, strict/legacy
HTTP expectations, fixed model values, probe reporting and diagnostic redaction.
Redis key construction, command order, lock tokens and TTLs remain unchanged.
The existing account-probe module still resolves its real parent imports; no
replacement implementation or compatibility wrapper was added.

The review identified separate hardening boundaries: local listing work and
result bounds, S3 continuation progress, and probe-lock release/lifecycle
atomicity. This checkpoint preserves their existing behavior. The next focused
reproduction targets S3 replies with missing, empty or non-advancing tokens.

## Fresh verification

- Default-feature storage baseline/final: 14/14, including six body-limit cases,
  real temporary local IO/readiness, junction escape and junction-cycle tests.
- Default-feature runtime baseline/final: 11/11 with exact executed test paths.
- Separate cargo check --offline --locked --all-targets: passed.
- Official scoped formatter, checker 19/19, ratchet, exact source/test/public-path
  proof, neighbor hashes, UTF-8 without BOM and whitespace checks: passed.
- Gateway and Neuro git diff --check: passed independently.

All native gates are terminal. No live S3, Redis or provider service was used
for this structural acceptance. The global formatter still reports the two
unchanged S06 runtime-mirror files; scoped formatting passes.

Strict scans 1,528 files: 31 hard, 35 mandatory and 40 soft. There are 66 files
above 700; accepted clearance is 79/145 (54.5%). Strict still exits 1.

The acceptance snapshot records Gateway HEAD
4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d and 1,670 dirty entries: 168 modified,
one unstaged deletion, two staged deletions and 1,499 untracked. Neuro HEAD is
bf818f0324024634bc890585efb78cc8e603d11a with 192 entries: ten modified and
182 untracked. Counts precede this report; inherited changes remain preserved.

## Evidence and continuation

Immutable evidence:
target/effective-line-evidence/20260912-storage-runtime-owners/scope.json.
The directory also contains the original-source snapshot, bounded layout/proof
scripts, paired Cargo logs and non-Cargo gate receipts.
[Lane](../plan/parallel-lanes/storage-runtime-owners.md).

S06 retains its implementation and original cursor. GWP-20260912-01 and the
explicit source/docs freeze and shared release-build transfer remain pending.
No new release was built, live service changed or runtime-profile policy migrated.

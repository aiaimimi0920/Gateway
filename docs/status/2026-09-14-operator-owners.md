# Operator database ownership acceptance

Accepted on 2026-09-13 at 17:24:02 UTC (2026-09-14 local time). This closes
operator ownership; the full Gateway refactor and release plan remains open.

## Scope and preservation

The entry src/db/operator.rs decreases from 2,810 to 180 effective lines.
Fifteen private owners separate inventory, associations, cost and pressure
reports, readiness, probe and runtime state, health, identity, usage queries,
pricing, pricing editor, cost hints, public report models and existing tests.

Exact reconstruction preserves 88 parser-recognized production definitions,
45 public paths (12 functions and 33 structs), 12 SQL raw literals and all nine
original test bodies/module paths. The generic CostProviderRef impl is separately
verified because the reused item parser does not recognize generic impl headers.
Its private fields and constructors stay at the entry, along with shared SQL
records and small shared helpers. Nineteen private helpers gain only pub(super)
for sibling/test use. The associations relative list_model_aliases call resolves
through the explicit entry import; Redis keys and AsyncCommands are explicit in
their owners. All 565 neighboring inputs retain their saved hashes.

| File under src/db/operator/ | Effective lines | Responsibility |
| --- | ---: | --- |
| inventory.rs | 248 | Inventory assembly and service summary |
| associations.rs | 181 | Model/provider association projections |
| cost.rs | 320 | Cost report assembly |
| pressure.rs | 195 | Running requests and concurrency pressure |
| readiness.rs | 25 | Readiness provider status counts |
| probe_state.rs | 121 | Persistent probe/cooling state |
| runtime_state.rs | 152 | Ordered breaker and runtime state updates |
| health.rs | 60 | Health projection and runtime-state normalization |
| identity.rs | 63 | Visible provider and source identity |
| usage_queries.rs | 188 | Usage aggregate and catalog reads |
| pricing.rs | 164 | Configured pricing parsing and cost estimation |
| pricing_editor.rs | 124 | Model pricing precedence and editor rows |
| cost_hints.rs | 76 | Inventory pricing coverage and hints |
| models.rs | 388 | Existing public report contracts |
| tests.rs | 414 | Nine unchanged operator regressions |

All sixteen files stay below 500; no exception is added. Review covered query
binding, inventory hiding, pricing precedence, database-over-runtime identity,
Redis/DB side-effect order, errors, serialization and resource lifetime. This
extraction adds no task, lock, process, storage operation or allocation policy.
The existing u64-to-i64 pricing cast, unchecked cost/count addition, diagnostic
text truncation and separate Redis/database operations remain investigation
leads; no hidden behavioral correction was included.

## Verification and evidence

The same gates passed before and after extraction, with identical test identities
and warning sets: operator units 9/9, provider-runtime caller units 11/11, and
operator-summary HTTP contracts 10/10. Locked offline all-targets compilation
passed at 2026-09-13 17:22:37.027 UTC.

Fresh source proof, scoped rustfmt, checker 19/19, ratchet, UTF-8 without BOM,
idle process proof and separate Gateway/Neuro diff checks pass. Global rustfmt
still exits 1 only for the two unchanged S06 runtime-mirror files. Strict exits
1 with 1,774 scanned, 19 hard, 27 mandatory and 40 soft; 46 files remain above
700. Clearance advances to 99/145 (68.3%).

Immutable evidence: target/effective-line-evidence/20260914-operator-owners/scope.json.
SHA-256: 294ea26375415d0223ef481aa51085ef1c3ed25027690b5a52b30fb37165e59c.
The accepted source union contains 581 inputs. Acceptance verifies snapshot,
source and log hashes, exact gate identities/counts, serialized timings, inventory
and formatter findings. External Beaver Cargo/desktop builds caused earlier
process-guard rejections; those receipts remain recorded. No external process
was terminated. All owned operator native gates reached terminal success.

At acceptance, Gateway HEAD is 4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d:
1,979 status entries (182 modified, one unstaged deletion, two staged deletions,
1,794 untracked). Neuro HEAD is bf818f0324024634bc890585efb78cc8e603d11a:
192 entries (ten modified, 182 untracked). Documentation publication adds its
own status changes afterward; inherited work is preserved.

## Next boundary

The complete src/db/mod.rs implementation and its callers have been reviewed.
Continue with the database-root ownership lane and its paired caller contracts.
S06 retains its implementation/cursor pending GWP-20260912-01. Runtime-profile
governance, final freeze/build transfer, full language/provider/release gates,
immutable packaging and packaged runtime/UI/Docker acceptance remain open.
This checkpoint makes no release, dependency, baseline, policy, exception or
persistent deployment change; persistent port 4200 and no persistent 4226 remain
the eventual runtime acceptance conditions.

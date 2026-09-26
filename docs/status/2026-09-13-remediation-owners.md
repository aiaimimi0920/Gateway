# Remediation ownership acceptance

Accepted at 2026-09-13 07:26:01 UTC. This is a structural checkpoint in the
continuing Gateway plan; it does not accept the final release or deployment.

The coordinator personally inspected the complete remediation source, its
database reexports, management handlers and desktop contract/schema consumers.
The entry decreased from 6,932 to 342 effective lines. All 32 scoped source/test
files are at most 473 effective lines. The 48 public DTOs occupied 685 lines, so
they moved into five domain owners with their existing public entry paths intact.

## Ownership and preservation

Paths below are relative to src/db/remediation/. Counts are language-aware
effective lines from the accepted source manifest.

| Owner | Lines | Responsibility |
| --- | ---: | --- |
| ../remediation.rs | 342 | Public wiring, shared private records/configuration and shared filtering/buckets |
| plan.rs | 473 | Ordered action-plan construction |
| plan_context.rs | 141 | Incident, route-policy and latest-sync context reads |
| alerts.rs | 170 | Alert queue, schedule and delivery profile |
| queue.rs | 314 | Remediation queue and apply/dry-run eligibility |
| sweep.rs | 120 | Ordered remediation sweep and execution input |
| runs.rs | 297 | Durable run queries, views and summary |
| execution.rs | 332 | Execution, follow-up simulation and history writes |
| route_policy_patch.rs | 409 | Validated tightening patches and pure application |
| impact.rs | 346 | Impact windows, metrics and capture |
| effectiveness.rs | 304 | Run effectiveness classification and aggregation |
| effectiveness_snapshots.rs | 225 | Stored effectiveness snapshot lifecycle and inventory |
| effectiveness_trends.rs | 146 | Snapshot trend projection |
| effectiveness_anomalies.rs | 307 | Threshold profiles and anomaly reports |
| effectiveness_anomaly_snapshots.rs | 178 | Stored anomaly snapshot lifecycle and filtering |
| policies.rs | 287 | Policy queries, views, summary and schedule |
| policy_write.rs | 295 | Policy validation and persistence |
| policy_parameters.rs | 162 | Policy input normalization and thresholds |
| policy_sync.rs | 349 | Policy/ad-hoc sync, result serialization and sync state |
| policy_sync_filters.rs | 228 | Request-audit/export sync filters and tag parsing |
| policy_sweep.rs | 100 | Scheduled policy sweep |
| auto_remediation.rs | 257 | Provider-health observation and auto-remediation profiles |
| action_models.rs | 138 | Action, plan, queue and sweep DTOs |
| policy_models.rs | 175 | Policy and sync DTOs |
| run_models.rs | 109 | Run and impact DTOs |
| effectiveness_models.rs | 166 | Effectiveness, snapshot and trend DTOs |
| effectiveness_anomaly_models.rs | 102 | Effectiveness anomaly DTOs |
| tests.rs | 199 | Existing test imports and four private fixtures |
| tests/actions.rs | 102 | Action plan and route-policy patch contracts |
| tests/scheduling.rs | 144 | Policy, alert and remediation schedules |
| tests/sync.rs | 91 | Sync source/filter and threshold contracts |
| tests/effectiveness.rs | 141 | Run summaries, metrics, classification and snapshot filters |

Exact normalized item comparison preserves 186 production definitions: 118
functions, 61 structs, four enums, two implementations and one constant. All 73
public paths, 48 public DTO definitions/attributes, private fields, 14 SQL
literals, 21 test bodies and four fixtures are preserved. Forty-six private
functions gain only pub(super) visibility. Private cross-owner helpers are
imported from their actual owner; sixteen original test helpers use cfg-guarded
entry imports.

The existing super::GatewayRateLimitDefinition references still resolve to the
same routing type through the entry import. The nested test modules import the
same private SupportedPolicySyncKind through their test parent. Test names and
bodies are unchanged; their new module paths are checked against an explicit
four-group mapping. No private field was widened to make the moves compile.

All 488 neighboring inputs remain byte-identical, including nine Rust caller
files and eight additional desktop/management contracts. The predecessor's
478-input union was carried forward with accepted hashes overriding older ones.

## Verification

| Gate | Before | After |
| --- | ---: | ---: |
| Original remediation unit tests | 21/21 | 21/21 |
| Incident caller unit tests | 6/6 | 6/6 |
| Complete export family, including text-policy regression | 12/12 | 12/12 |

The same locked offline test commands bracket the extraction. Remediation leaf
identities are preserved through the explicit module mapping; caller test
identities are unchanged. Unit warning sets match before/after. Fresh locked
offline all-targets completed successfully at 2026-09-13 07:22:32.557 UTC;
its warning set matches the preceding accepted checkpoint.

Scoped rustfmt, checker tests 19/19, ratchet, exact source/neighbor proof,
UTF-8/no-BOM checks, terminal Cargo cleanup and both repository git diff --check
gates pass. The new owners were applied first and verified before switching the
entry wiring. Fresh process guards preceded each write batch, formatting and
Cargo gate. All owned native operations are terminal; no process was terminated.

Strict scans 1,739 files: 21 hard, 27 mandatory and 40 soft findings. Forty-eight
files remain above 700 lines, giving 97/145 (66.9%) structural clearance. Global
formatting still reports only the unchanged S06 files:

- src/upstream/gemini_canvas_runtime_mirror.rs
- src/upstream/gemini_canvas_runtime_mirror_tests.rs

## Scoped safety and lifecycle review

Every production owner was reviewed against its original definitions. SQL text,
bind order, filters, limits, sorting, normalization and serialized fields are
unchanged. Query/context/run/policy owners retain the original connection and
transaction boundaries. Execution, sweeps, impact capture and snapshot owners
retain side-effect order, error propagation, object-store operations and
dry-run behavior. Auto-remediation retains its original provider-health and
Redis failure handling. Pure plan, patch, metric, trend and DTO owners introduce
no new task, handle, network request or resource lifetime.

Three observations remain outside this behavior-preserving checkpoint: early
execution error propagation can bypass later run-record handling; run insertion
binds no policy_id; extreme stored timestamps reach unchecked arithmetic. No
behavioral fix or runtime reproduction is claimed for these observations.
The paired unit gates do not establish live PostgreSQL, Redis/object-store,
packaged runtime, UI or Docker acceptance.

## Evidence and continuation

Immutable evidence directory:
target/effective-line-evidence/20260913-remediation-owners/.
Scope SHA-256:
f23df37b01945aa7b572bb792a0ba3562bc90b1a687da73303e39b9737def063.
Original raw/canonical source SHA-256:
e06b8589f3a40b15da7a4d33cb9f775d179e69c9638a18d8c72caf90c4e2320d.

The scope records snapshots, exact ownership/item proof, full test logs,
serialized gate receipts, process guards, strict/ratchet inventories and helper
hashes. At source acceptance, Gateway HEAD is
4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d with 1,938 status entries: 181 modified,
one unstaged deletion, two staged deletions and 1,754 untracked. Neuro HEAD is
bf818f0324024634bc890585efb78cc8e603d11a with 192 entries: ten modified and 182
untracked. These counts precede publication of this report. Inherited changes
and staged deletions were preserved.

Access database ownership is the next independent candidate; its source has
only been read so far. S06 retains its reserved implementation and original
cursor. GWP-20260912-01, runtime-profile governance and the final source/docs
freeze and build transfer remain pending. No dependency, baseline, exception,
checker-policy, release or persistent deployment change was made.

# Lane V: browser console production ownership

## Controller/render boundary completion

The original entry's structural debt is cleared: the rendering root is 549
effective lines and the controller assembly is 698. Earlier statements below
about all hooks remaining in the entry describe historical checkpoints. Domain
hooks now retain their previously extracted ownership; `useConsoleController`
assembles their state, effects and action contracts without adding behavior.
The root retains navigation, shared account-card bridging, workspace gates and
global dialog composition.

Both files have current source-hash-bound 501-700 exceptions in the existing
exception registry, approved by `controller_soft_limit_review`. For the rendering
root, more branch wrappers would duplicate shared state/slot wiring and obscure
dialog mounting and route gates. For the controller, separating assembly from its
refs, hydration effects and return contract would make lifecycle order harder to
verify. These exceptions do not permit growth above 700 or new unrelated duties.
Future business behavior belongs in the focused domain owners.

`verify-console-controller.mjs` compares the original snapshot's pre-render
statements against the hook and its renderer suffix against the remaining root;
the 124 returned names also match the 124 destructured names. Typecheck and web
build passed. Full desktop tests passed 311/311 across 61 files. Ratchet initially
required explicit exceptions, then passed after the two independently reviewed,
hash-bound entries were added. No baseline or existing exception regeneration
was used. Independent import/return review found no lost bindings or effect-order
change. Native release remains pending its existing Cargo handoff.


Coordinator owns BrowserConsoleApp.tsx and new cohesive pure owners in its
directory. Initial extraction moves the statistics view builder, formatter helpers
and private return types to pilotStatsView.ts. The component retains its hooks,
polling, state, resource cleanup, JSX and current public interface.

The builder reads provider-account aggregates separately from credential health.
Preserve its null/missing-data behavior, decided-request success denominator,
timestamp locale formatting and shared-credential count. It introduces no I/O,
state owner, API request or dependency package. Existing full desktop baseline is
245/245, with focused typed statistics data-boundary coverage in lane U.

Capture exact source before editing, verify moved declarations and unchanged
component body, then run focused console tests, typecheck, web build and ratchet.
The entry remains debt until its entire decomposition is complete. No temporary
extraction is counted as cleared merely because one child owner is small.

## Statistics owner extraction

Measured entry before extraction: 6139 effective lines. Entry is now 5973 and
pilotStatsView.ts is 171. The extracted builder, three formatters and two private
types match their original declarations exactly except for the builder export.
An AST comparison of every top-level declaration also proves the complete
BrowserConsoleApp component body is unchanged. The new owner depends on leaf
account types, telemetry helpers and existing metric formatting; no dependency
points back to BrowserConsoleApp. The data-scope comment moved with the builder.

The eight Codex library tests pass, including typed statistics granularity and
multi-edit coverage. Typecheck, web build, ratchet and diff pass. Evidence:
target/effective-line-evidence/browser-console-owners/before.tsx,
verify-stats.mjs, stats-proof.log, stats-tests.json, stats-typecheck.log,
stats-web-build.log and stats-ratchet.log. Both source files are UTF-8 without
BOM. No resource lifetime, input validation or allocation algorithm changed.

Entry decomposition remains incomplete; no added debt-clearance credit. Native
release still requires the shared build transfer. The rebuilt web assets do not
constitute a new immutable native release. All owned sessions are terminal.

Independent next-boundary audit identified an unreachable manual validation flow
in the entry. Its handler is the sole non-null setter of draft validation and has
no rendered invocation. Any later removal must preserve active-route diagnostics,
save/error handling, API endpoint support and its independent API tests.

## Unreachable validation cleanup

Removed the unreferenced manual-validation callback, its null-initialized state,
private diagnostic adapter, unreachable feedback JSX and five null-only reset
calls. The callback was the only setter of a non-null validation value and had
no caller. Busy state now permits only save/null. Active-route diagnostics,
autosave, shared error handling and the validation API/client tests remain.

Entry 5973 -> 5874 effective lines. An exact source transform proof verifies that
only this dead flow was removed, ignoring blank lines while preserving every
remaining line. The earlier statistics extraction proof is historical after this
separate cleanup; its new owner was not modified.

Fresh full desktop tests 245/245 across 47 files, typecheck, web build, ratchet and
diff pass. Evidence: before-validation-cleanup.tsx,
verify-validation-cleanup.mjs, validation-proof.log, validation-tests.json,
validation-typecheck.log, validation-web-build.log and validation-ratchet.log
under target/effective-line-evidence/browser-console-owners. No new resource or
trust boundary was introduced. Entry decomposition and native release remain
incomplete; structural debt clearance is unchanged. All owned sessions terminal.

## Route-document and group-draft owners

Extracted routeDocument.ts (37 effective lines) for runtime document-shape guards,
JSON parsing and canonical formatting, plus accountGroupDraft.ts (70) for group
draft shape, missing-ID and billing validation. These small cohesive owners are
kept separate because document decoding and editable group policy have different
consumers. The group owner imports the document guard directly; neither imports
the console entry. Private shape predicates remain private.

Entry 5874 -> 5779 effective lines. AST comparison preserves every declaration
and the complete component body exactly except moved exports/imports. The 50
console integration tests pass, including invalid billing, incomplete IDs,
refresh cancellation and multi-edit round trips. Typecheck, web build, ratchet and
diff pass. Evidence: before-route-drafts.tsx, verify-route-drafts.mjs and
route-drafts-{proof,tests,typecheck,web-build,ratchet} artifacts in this lane's
evidence directory. Files are UTF-8 without BOM. Parsing errors, numeric bounds,
secret-patch handling and state/resource lifetimes are unchanged.

The entry remains oversized and receives no completed-debt credit. The last full
desktop run remains 245/245 from the preceding cleanup; this extraction used the
matching 50-test console scope. No native release or shared Cargo transfer ran.

## Model-route drafts and prototype-named mapping repair

Model and alias draft types, row conversion, cross-document model reference
rewriting, provider declarations and route-priority lookup moved to
modelRouteDraft.ts. Initial extraction measured entry 5779 -> 5647 and owner 147;
the exact declaration/component-body proof passed before the separate fix below.
Existing random row-ID generation, mutation order, absent supported-model
inheritance and route matching were preserved.

Review found a concrete mapping loss when renaming a model to __proto__: ordinary
property assignment invoked the inherited prototype setter rather than creating
a model-map entry. A regression test failed with the missing upstream mapping.
The owner now defines the target as an enumerable own data property, preserving
the object's prototype and JSON serialization. The test also verifies provider
and credential supported-model lists and alias references stay synchronized.
This is a targeted data-key fix, not a claim of global prototype pollution.

Final owner is 149 effective lines. Console plus regression tests 51/51,
typecheck, web build, ratchet and diff pass. Evidence: before-model-drafts.tsx,
model-drafts-proof.log (historical pre-fix), model-map-before.json and
model-drafts-{tests,typecheck,web-build,ratchet} artifacts. No new asynchronous
resource or API boundary. Entry remains oversized, with no added clearance credit
or native release. All owned sessions terminal.

## Secret patch and Gemini credential draft owners

Moved secret draft initialization, patch projection and credential-edit merging
to secretPatchDraft.ts; entry 5647 -> 5587. Exact declaration/component proof
passed before the separate merge optimization. Merging now indexes the first
existing tuple identity instead of repeatedly scanning the accumulated array.
JSON tuple keys avoid delimiter collisions and contain no secret values. Existing
duplicate ordering, latest incoming replacement, append order and input arrays
are preserved by the same contract test before and after the optimization.
Expected lookup work is linear in entry count plus identity encoding, with a
temporary identity index; no measured latency improvement is claimed.

The initial console run passed 48/51 with three multi-phase scenarios hitting the
5-second overall deadline. Four previously observed long scenarios now have
explicit 10-second budgets: credential creation, schedule then duplicate, group
creation/member management, and incomplete-ID repair. Their 3-second commit waits,
negative debounce waits and functional assertions remain unchanged. This is a
subsequent adjustment to the earlier worker-cap-only checkpoint. Fresh bounded
console plus secret regression tests passed 51/51.

Then extracted Gemini terminal/manual-completion predicates and generated
credential document application into geminiCredentialDraft.ts (79 effective
lines). Entry 5587 -> 5524. Exact AST proof preserves all declarations and the
complete component body; API calls, polling, cancellation, commit and dialog
state remain in the entry. Stable-ID checks, provider lookup and supported secret
edit filtering are unchanged. No new resource lifecycle or trust boundary.

Fresh combined console and both draft regressions pass 52/52; typecheck, web build,
ratchet and diff pass. Evidence: secret-drafts-proof.log (pre-optimization),
secret-merge-before.json, secret-drafts-bounded-tests.json and gemini-drafts-*
under this lane's evidence directory. New source files are UTF-8 without BOM.
Entry debt remains open; no clearance credit or native release. Shared Cargo
transfer is still required. All owned test/build sessions are terminal.

## Account catalog owner

Extracted routeAccountCatalog.ts (452 effective lines): document and backend
summary projections, account/group/provider bucket types, membership and enabled
filters, vendor identity and display normalization. Entry 5524 -> 5089. The
module depends only on API types, route guards and the existing provider index;
no import back into the console entry. Private vendor metadata and summary mode
normalization remain private. Editor callbacks, API/polling state and rendering
remain in the entry.

Exact AST proof preserves every declaration and the entire component body.
Existing default-account behavior, model inheritance, membership joins, ordering
and filter semantics are unchanged. Fresh console integration tests 50/50,
typecheck, web build, ratchet and diff pass. After import/private-export cleanup,
the exact proof and typecheck/ratchet were rerun; no behavior changed. Evidence:
before-account-catalog.tsx and account-catalog-{proof,tests,typecheck,web-build,
ratchet} in this lane's evidence directory. UTF-8 without BOM verified by proof.
No new resource lifecycle; repeated group/account transformations remain bounded
by input size. Entry debt and integrated native release are still open.

## Pilot pool policy parsing and category deduplication

Extracted pilotPoolPolicy.ts: pool defaults, provider policy normalization,
identity-category parsing and Codex defaults, with typed policy contracts.
Entry 5089 -> 4982; initial owner 120 effective lines. Exact declaration and
component proof passed before the separate optimization. Telemetry and Gemini
manual-add classification remain outside this owner. The telemetry comment was
restored beside its original function after extraction review.

Category deduplication now uses a Set of normalized IDs instead of findIndex for
each category, retaining the first policy and original order. Expected identity
lookup work changes from quadratic to linear, with linear temporary storage;
no measured speedup is claimed. The same focused contract passes before and
after, covering normalized duplicate IDs, prototype-named IDs, conflicting
policy flags and frozen inputs. Provider defaults and destructive-operation
opt-in semantics are unchanged; no API or asynchronous resource was introduced.

Fresh console plus policy contract tests 51/51; typecheck, web build, ratchet and
diff pass. Evidence: before-pilot-policy.tsx, pilot-policy-proof.log (historical
pre-optimization), pilot-policy-before.json and pilot-policy-{tests,typecheck,
web-build,ratchet} artifacts. Entry remains oversized; no clearance credit or
native release. Shared Cargo transfer is still pending; owned sessions terminal.

## Account ledger section projection

Extracted accountLedgerSections.ts (422 effective lines), with one public builder
and private account-card projection, inventory display metadata, Gemini section
merge/classification and section telemetry aggregation. Entry 4982 -> 4576.
All inputs remain explicit: route document, ledger rows, inventory, telemetry,
locale and account-group summary. No component state or callback lifecycle moved.

The AST-driven extraction preserves complete declarations and the component body,
including comments explaining provider-only telemetry attribution and post-merge
rollups. Provider totals remain attributed to an account only for a single route
identity; filtering does not change that identity count. Existing Gemini merging,
category ordering, billing resolution, credential health and scheduling defaults
are unchanged. Leaf dependencies do not import the console entry.

Fresh console, telemetry and account-view-model tests pass 60/60; typecheck,
web build, ratchet and diff pass. Evidence: before-ledger-sections.tsx,
ledger-extraction-patch.mjs and ledger-sections-{proof,tests,typecheck,web-build,
ratchet}. UTF-8 without BOM verified by proof. This is a structural checkpoint;
the 4576-line entry remains debt and gains no clearance credit. Native release
and accepted shared build transfer remain pending. Owned sessions terminal.

## Provider and credential editor drafts

Extracted providerCredentialDraft.ts (124 effective lines): provider row data,
supported-model text parsing and credential add/edit/duplicate form projections.
Entry 4576 -> 4467. Exact AST proof preserves all declarations and the complete
component body. Dialog state and apply callbacks remain in the entry. Credential
duplication still clears the secret value and requires replacement; edit still
uses keep with an empty value. Existing random row IDs, default values and model
text normalization remain unchanged.

Console integration tests pass 50/50, including credential creation/secret patch,
duplicate scheduling preservation and provider creation. Typecheck, web build,
ratchet and diff pass. Evidence: before-credential-drafts.tsx,
credential-drafts-patch.mjs and credential-drafts-{proof,tests,typecheck,web-build,
ratchet}. Source is UTF-8 without BOM; no new resource or API boundary. Entry debt
remains open with no clearance credit or native release. All owned sessions terminal.

Independent next-boundary audit located the Gemini manual-add lifecycle callbacks
and polling effect, plus the pilot action dialog JSX. The former requires explicit
request-generation/recovery and timer ownership; no cancellation change should
be smuggled into a structural move. These remain pending extraction work.

## Gemini manual-add session lifecycle hook

Extracted useGeminiManualAddSession.ts (175 effective lines), owning dialog state,
poll timer, open/close/complete/refresh callbacks and the polling effect. Entry
4467 -> 4322. Draft application, applied-session deduplication and commit/recovery
generation remain parent-owned; the hook receives a result callback. This keeps
route save ownership separate from authentication-session presentation.

Statement-level AST comparison proves every callback/effect body, dependency
array, state/ref declaration and remaining component statement preserved. The
fixed unconditional hook call follows the parent result callback. Timer cleanup
and 1500-ms interval are unchanged. Independent review found no introduced
ordering or ref-lifetime regression. It also identified pre-existing races:
late creation can reopen/replace a dialog, and late refresh can apply a result
after closure even when its dialog-state update is rejected. Those races require
separate deferred-response regression tests and lifecycle invalidation work.

Console tests 50/50, typecheck, web build, ratchet and diff pass. Evidence:
before-gemini-session.tsx, gemini-session-patch.mjs, verify-gemini-session.mjs and
gemini-session-{proof,tests,typecheck,web-build,ratchet}. Source UTF-8 without BOM.
This checkpoint does not claim race hardening or native release. Entry debt and
shared build transfer remain open. Owned tests/build sessions terminal.

## Gemini late-result lifecycle repair

Six deferred-response regressions failed against the extracted hook: create after
close/unmount/token replacement, earlier create resolving after a newer dialog,
refresh success after close and completion error after close. Lifecycle generation
now invalidates pending results on close, a new open, API/token replacement and
unmount. Every asynchronous create/refresh/complete success/error path checks the
captured generation before applying results or reporting errors. Layout cleanup
also clears the poll timer. Remote requests continue; this is result invalidation,
not transport cancellation.

Refresh failures are caught inside the fire-and-forget callback and reported only
while current. Added checks for live refresh error handling and no further polls
after unmount. Fresh final lifecycle plus Gemini integration tests 11/11,
typecheck, web build, ratchet and diff pass. Web build preceded only a type
annotation and the two final test additions; runtime source logic is identical.
Evidence: gemini-races-before.json (0/6), gemini-races-tests.json (9/9),
gemini-races-final-tests.json (11/11), corresponding logs/typechecks, web-build
and ratchet logs. The structural extraction proof is historical after this fix.

Independent review confirmed the guards and identified a remaining separate
same-session response-ordering risk: overlapping refresh/complete responses have
no sequence/version guard. That requires a dedicated out-of-order regression.
Entry debt, transport cancellation and integrated native release remain open.

## Gemini response ordering

A deferred regression reproduced an older waiting-user refresh overwriting a
successful completion: initial run 8/9. Responses now carry a monotonically
increasing local request sequence as well as lifecycle generation. Starting a
completion supersedes earlier refreshes; polling and duplicate completion calls
wait while that completion is pending. Nonterminal completion resumes refresh,
while stale successes and errors cannot overwrite the current result.

Regression plus Gemini integration tests pass 12/12. An additional fake-timer
contract verifies no polling/duplicate completion during the pending request and
refresh resumption afterward; final focused hook tests 10/10. Typecheck, web
build, ratchet and diff pass. Web build covers the final production logic; only
the last focused test was added afterward. Evidence: gemini-order-before.json,
gemini-order-tests.json, gemini-order-final-tests.json and corresponding build,
typecheck and ratchet logs. No transport cancellation or native release claimed.
Sequence checks order local requests; they do not impose backend timestamp or
replication consistency guarantees. Entry remains 4322 effective lines.

## Pilot action dialog view

Extracted PilotActionDialog.tsx (483 effective lines), owning the four pilot
dialog variants, title selection and discriminated state contract. Entry
4322 -> 3918. The view receives explicit typed state and callbacks; probing,
scheduling, telemetry and request lifecycles remain parent-owned. The probe result
prop exposes only the message/probe-point fields consumed by the view.

Exact JSX comparison preserves every class, label, disabled condition, input bound,
metric and event binding. The remaining component body is byte-identical except
the typed component invocation. No portal, focus behavior, layout or style was
changed. Console tests 50/50 cover account/provider probing, scheduling and stats;
typecheck, web build, ratchet and diff pass. Evidence: before-pilot-dialog.tsx,
pilot-dialog-patch.mjs, verify-pilot-dialog.mjs and pilot-dialog-{proof,tests,
typecheck,web-build,ratchet}. New source UTF-8 without BOM verified by proof.
Entry debt remains open and receives no clearance credit. No native release or
shared build transfer occurred; owned test/build sessions terminal.

## Route data loading hook

Extracted useConsoleRouteData.ts (226 effective lines), owning route/telemetry
snapshots, their loading/error states, refresh request ID and single-flight
coordinator. Entry 3918 -> 3754. API/token, translation and probe invalidation
are explicit inputs. Only the route config, automation and shared error setters
needed by parent actions are exposed; other snapshot setters remain private.
Route-to-editor hydration and initial refresh effects stay in the component,
preserving their existing order and editor ownership.

Statement-level proof preserves every moved state/ref/callback statement and
remaining component statement. Parallel fetches, optional endpoint fallback,
partial telemetry errors and request-ID suppression are unchanged. The telemetry
banner comment stays with its state. Console and single-flight tests pass;
typecheck, web build, ratchet and diff pass. Evidence: before-route-data.tsx,
route-data-patch.mjs, verify-route-data.mjs and route-data-{proof,tests,typecheck,
web-build,ratchet}. UTF-8 without BOM verified by proof. No endpoint or new
resource lifecycle was introduced. Token-loss/unmount invalidation should be
audited independently; the extraction does not claim stronger stale-response
guarantees than the previous loader. Entry debt and native release remain open.

## Route refresh lifecycle and snapshot isolation

Three deferred/microtask regressions failed against the extracted loader: an old
route response published after logout, a restored token joined its previous
pending flight, and a queued refresh started API work after unmount. Layout
cleanup now invalidates lifecycle and request IDs, marks the hook unmounted and
replaces the single-flight coordinator. A queued operation checks its captured
lifecycle before changing state or starting requests. Existing result/error/finally
request-ID checks suppress old completions.

Independent review identified already-published snapshots surviving identity
changes. Two additional tests failed after loading real fixture snapshots then
logging out or switching tokens. Layout setup now clears route, summary, inventory,
automation, refill, telemetry and error snapshots on API/token changes, before
the new refresh; busy reflects whether a management token is available. Same
identity manual refresh keeps its existing behavior.

Final loader/console/single-flight tests 58/58, typecheck, web build, ratchet and
diff pass. Evidence: route-data-races-before.json (0/3),
route-data-snapshots-before.json (3/5), route-data-lifecycle-final-tests.json,
corresponding typecheck/web-build and lifecycle-ratchet logs. The extraction proof
is historical after these fixes. No transport cancellation was added: admitted
HTTP requests can still finish, but their invalidated results are ignored.
Entry debt and integrated native release remain open; owned sessions terminal.

## Route draft state and commit preparation

Extracted useConsoleRouteDraft.ts (222 effective lines), owning editor/message,
secret and structured-row state, patch projection, document parsing, dirty/revision
checks and commit preparation. Entry 3754 -> 3612. Hydration, before-unload listener,
autosave timer and asynchronous save/recovery remain parent-owned in their original
order. This separates editable draft state from active route snapshots and
request lifecycles. Group validation messages move with commit validation.

Statement proof preserves all moved logic and remaining component statements.
The structured-row converter is an explicit stable module-scope input; its new
hook dependency is included so a future caller replacing it cannot retain a stale
callback. The proof allows only that dependency addition and export modifiers.
Secret-grant checks, empty replacement rejection, group ID/billing invariants,
revision matching and probe invalidation are unchanged.

Console tests 50/50, typecheck, web build, ratchet and diff pass. After adding the
converter dependency, the proof and typecheck were rerun; the current caller's
converter identity is constant. Evidence: before-route-draft.tsx,
route-draft-patch.mjs, verify-route-draft.mjs and route-draft-{proof,tests,typecheck,
final-typecheck,web-build,ratchet}. Source UTF-8 without BOM verified by proof.
Entry debt and native release remain open; no clearance credit or Cargo transfer.

## Obsolete structured-editor callback cleanup

Independent reachability audit found no rendered consumers for the old alias row
editor, provider row CRUD or model-route row CRUD. Removed 12 dead callbacks/
derived statements and two private error messages. Alias row state, initialization,
sync wiring and its unused row type/converters were removed from the draft hook
and modelRouteDraft owner. Root 3612 -> 3461; draft hook 222 -> 218; model owner
149 -> 132. This also eliminates unnecessary alias row allocation/random IDs
during hydration and structured synchronization.

Preserved live model-route apply/state used by model-pool reorder/reset/toggle,
provider row projections used by mappings/order, provider catalog creation and
all route document aliases/model-reference rewriting. Removing the unused alias
editor representation does not remove the underlying alias data or API contract.
Repo-wide source search finds no references to the removed alias row symbols.

Exact expected-transform proof verifies root changes are limited to the listed
dead statements and alias wiring. Fresh full desktop suite, typecheck, web build,
ratchet and diff pass. Evidence: before-dead-editors.tsx, dead-editors-patch.mjs,
verify-dead-editors.mjs and dead-editors-{proof,tests,typecheck,web-build,ratchet}.
Existing extraction proofs for touched owners are historical after this cleanup.
Entry remains oversized; no debt clearance or native release. Sessions terminal.

## Account group editing actions

Extracted useAccountGroupEditor.ts (196 effective lines): group construction,
validation/application, row edits, enablement, membership toggle, exclusive routing
assignment and removal. Entry 3461 -> 3314. Editable rows and selection/query state
remain with their existing owners; the hook receives typed React setters and
document replacement. It introduces no effect, resource or API lifecycle.

Exact statement proof preserves all callbacks, dependency arrays, group constructor
and error text, plus remaining component statements. Missing-ID/billing rejection,
member deduplication, selection fallback and existing notifications are unchanged.
Focused groups/draft/account-actions tests pass, with typecheck, web build, ratchet
and diff. Full desktop baseline remains 263/263 across 52 files from the preceding
cleanup; no unrelated full-suite rerun. Evidence: before-group-actions.tsx,
group-actions-patch.mjs, verify-group-actions.mjs and group-actions-{proof,tests,
typecheck,web-build,ratchet}. New source UTF-8 without BOM verified by proof.
Entry debt and native release remain open; owned test/build sessions terminal.

## Model pool editing and provider model-map repair

Extracted useModelPoolEditor.ts: chain reorder/reset, model enablement, model
add/edit/delete and provider mappings. Entry 3314 -> 3112; initial owner 264
effective lines. Dialog state and route draft ownership remain outside the hook.
Exact statement proof preserves callbacks and remaining component statements;
comments describing wildcard priority, inherited chains and cross-document model
references moved with their callbacks. The initial extraction gates passed.

Review found another prototype-named data-key loss in mapping submission: assigning
model_map["__proto__"] invoked the inherited setter and lost the upstream mapping.
A focused hook regression failed before the fix. Mapping construction now uses
Object.fromEntries, which creates own data properties and retains duplicate-key
last-write behavior. Empty mappings still delete the field. The regression checks
__proto__/constructor keys, unchanged prototype, JSON persistence, original input
preservation and dialog closure. This is separate from the earlier model rename
repair. Final owner 263 effective lines.

Console plus both model regressions pass 52/52, typecheck, web build, ratchet and
diff pass. The test's unknown provider value was narrowed explicitly after the
first typecheck identified unsafe property access; its focused test and typecheck
then passed. Evidence: before-model-actions.tsx, model-actions-proof.log
(pre-fix), model-mapping-before.json, model-actions-final-tests.json,
model-mapping-final-test.json and corresponding typecheck/build/ratchet logs.
Entry debt and native release remain open; no clearance or Cargo transfer.

## Pilot policy editing and category serialization repair

Extracted usePilotPolicyEditor.ts: identity category creation, category/provider
policy edits and guarded automation toggles. Entry 3112 -> 2875; initial owner
279 effective lines. Exact statement proof preserves callbacks and remaining
component statements. Draft state, data loading and destructive API operations
remain outside this hook. Trusted-driver/refill-queue guards are unchanged.

Review found category creation wrote camelCase view-model fields directly into
the route document. Adding a category therefore reset existing configured target
sizes and automation flags when the document was read again. A regression failed
with an existing size 73 and enabled flags. Creation now serializes all categories
using pool_target_size, auto_refill_enabled and auto_prune_enabled, matching the
existing edit path and route parser. The test verifies exact persisted fields,
read-back policy, new-category defaults and unchanged input document.

Console plus policy regressions pass 52/52, final typecheck, web build, ratchet and
diff pass. Evidence: before-policy-actions.tsx, policy-actions-proof.log
(pre-fix), policy-add-before.json and policy-actions-{tests,final-typecheck,
web-build,ratchet}. Source UTF-8 without BOM verified at extraction. No new
asynchronous resource or API boundary. Entry debt/native release remain open;
no clearance or shared Cargo transfer. Owned sessions terminal.

## Credential dialog draft actions

Extracted useCredentialDialogEditor.ts (261 effective lines). BrowserConsoleApp
decreased from 2875 to 2668. Seven callbacks retain exact bodies and dependencies:
dialog add/edit/duplicate, secret edit staging, credential apply/remove and dispatch
toggle. Dialog and secret draft state remain in their existing owners. Provider
catalog provisioning stays in the component and consumes the returned secret edit
callback. No asynchronous request, effect, lifecycle or secret serialization moved.

The exact statement proof includes every other component statement and top-level
declaration. Initial typecheck exposed nullable dialog initialValue in the existing
contract; the new option type was corrected without changing that contract.
Before and after focused console/credential/secret tests pass 64/64. Final typecheck,
web build, ratchet and diff checks pass. Evidence: before-credential-actions.tsx,
verify-credential-actions.mjs and credential-actions-{before,tests}.json plus
final-typecheck, web-build and ratchet logs in this lane's evidence directory.
Source reviewed and UTF-8 without BOM checked. Structural debt and native release
remain open; no shared Cargo ownership transfer or new release is claimed.

## Probe schedule draft editor

Extracted useProbeScheduleEditor.ts (198 effective lines), reducing the entry
from 2668 to 2522. Four exact callbacks own schedule dialog initialization and
credential/provider schedule draft application. The existing dialog and interval
state remain in the component. Integer validation from 1 to 10080, provider
fallback scheduling, mixed-interval default 60, account mode and save semantics
are preserved. No timer or live probe request moved into this draft editor.

Exact statement proof and UTF-8 without BOM checks pass. The same 64 console,
credential and secret tests passed immediately before and after extraction;
typecheck, web build and ratchet pass. Evidence: before-schedule-actions.tsx,
verify-schedule-actions.mjs, schedule-actions-tests.json and corresponding logs;
the before suite is credential-actions-tests.json. Native release and entry debt
remain open. Probe lifecycle extraction requires care: invalidation is consumed
by route data and draft hooks before those hooks produce error and dirty state,
so a combined hook cannot simply accept all those values at its current position.

## Probe request actions and unmount protection

Extracted the two async request handlers into useConsoleProbeActions.ts. Entry
2522 -> 2357; owner initially 241, now 261 effective lines after hardening.
State, generation refs, identity refs, refresh/draft invalidation, dialog close and
shared secret recovery remain in their existing positions. This avoids a cycle
between probe invalidation and the route data/draft hooks. The option contract
retains probePoint in credential results; the similarly named account view type
does not require that field and was rejected by typecheck rather than substituted.

Exact initial callback/body proof and 64/64 focused tests passed. Three new
regressions then failed: late success/error after unmount still published state or
called secret recovery, and retained callbacks could start new requests. Layout
cleanup now invalidates both generation refs; mounted guards prevent new requests
and recovery after unmount. Network requests themselves are not aborted.

Combined console/credential/secret/unmount suite passed 67/67. Independent review
found no implementation defect and identified provider and StrictMode coverage
gaps. Added those cases; final hook tests pass 7/7, including both provider outcomes,
retained provider callback and successful probes after StrictMode effect replay.
The StrictMode assertion was corrected to compare the actual API response shape.
Final typecheck/ratchet/diff pass; web build passed after the lifecycle fix (later
changes only narrow a type parameter and extend tests). No formatter configured.

Evidence: before-probe-actions.tsx, verify-probe-actions.mjs (historical pre-fix
proof), probe-actions-*, probe-unmount-before.json (0/3), probe-unmount-tests.json
(67/67), probe-unmount-final-tests.json (7/7) and final typecheck/ratchet logs.
No native release, clearance credit or Cargo transfer. Owned gate sessions terminal.

## Credential pool operation owner

Extracted useCredentialPoolActions.ts, 188 effective lines; entry 2357 -> 2216.
The three prune/archive-purge/refill callbacks and their three busy states move
together. No other root consumer uses those setters. Draft guards, trusted prune
driver requirement, user-requested refill availability, API arguments, automation
snapshot update and refresh/toast ordering retain exact statement bodies.

Statement proof includes all six moved declarations and all remaining component
statements. UTF-8 without BOM, typecheck, console tests 50/50, web build, ratchet
and diff pass. Evidence: before-pool-actions.tsx, verify-pool-actions.mjs and
pool-actions-{tests,typecheck,web-build,ratchet}. Initial ownership extraction
adds no lifecycle behavior. Follow-up hardening remains necessary: these actions
currently have no identity/unmount response guards, and an older request can
clear a newer busy marker. Native release and structural clearance remain open.

## Credential pool action identity repair

Seven regressions failed against the extracted implementation: late purge success
after token/API/unmount transitions, late error after unmount, older completion
clearing newer busy state, and retained callbacks issuing stale requests. The hook
now tracks active API/token identity, a lifecycle epoch and a separate sequence
for each operation. Identity changes reset busy state; cleanup invalidates response
publication. Every action guards success, error and final busy cleanup. Existing
driver/draft/refill gates remain intact. Remote operations already issued continue;
this change isolates local results and does not cancel or undo remote mutations.

Combined console plus initial regressions pass 57/57. Independent review found no
implementation defect and requested additional coverage. Final hook tests pass
11/11 including A -> B -> A, prune/refill late errors and independent concurrent
purge/refill busy markers. Map input types now require only the capability fields
actually read; runtime caller data is unchanged. Final typecheck/ratchet/diff pass;
web build passed after the lifecycle repair, before test-only and type-only additions.

Evidence: pool-races-before.json (0/7), pool-races-tests.json (57/57),
pool-races-final-tests.json (11/11), final-typecheck/final-ratchet and web-build logs.
The initial pool-actions exact proof is historical after this behavior repair.
No entry clearance, native release or Cargo transfer; owned sessions terminal.

## Model directory selectors

Extracted seven memoized model selectors into useConsoleModelSelectors.ts,
113 effective lines; entry 2216 -> 2150. Model chains, provider order, card
directory, dialog provider options, model names, mapping counts and mapping target
retain exact expression bodies and memo dependencies. Builder input types are
derived from its actual signature. No state, mutation, effect or network work moved.
The provider-option ordering comment moved beside its selector.

Exact statement proof and UTF-8 without BOM checks pass. Console plus model draft
and model editor regressions pass 52/52; typecheck, web build, ratchet and diff pass.
Evidence: before-model-selectors.tsx, verify-model-selectors.mjs and
model-selectors-{tests,typecheck,web-build,ratchet}. Entry debt, native release and
shared Cargo transfer remain open; no structural clearance credit.

## Account ledger selectors

Extracted 18 memoized account/catalog/telemetry selectors into
useConsoleAccountSelectors.ts (293 effective lines initially, plus one type import
after checking the probe contract). Entry 2150 -> 1971. Before-unload protection,
selection state and all mutation effects stay in the component. Revision-matched
active summaries, draft fallback, filtered account rows and unfiltered group
totals retain exact bodies/dependencies. The selected statistics view reads the
same polled snapshot; no new request or copied state is introduced.

Initial typecheck caught two input contract mismatches: the account row builder
accepts probe results without probePoint while the dialog requires it, and the
catalog filter's optional enabled argument is mandatory for the ledger filter.
The hook now reuses the existing probe action result type and requires a defined
enabled filter. No runtime behavior changed. Explanatory comments moved beside
their owning selectors. Exact statement proof and UTF-8 without BOM checks pass.

Console/telemetry/account view regressions pass 60/60. Final typecheck, web build,
ratchet and diff pass. Evidence: before-account-selectors.tsx,
verify-account-selectors.mjs, account-selectors-tests.json and final-typecheck,
web-build, ratchet logs. No clearance/native release/Cargo transfer; sessions terminal.

## Entry import and dead-function cleanup

TypeScript noUnusedLocals diagnostics identified imports left behind by owner
extraction and two unused pure functions, normalizePilotCapacityLabel and
optionalNonNegativeNumber. Removed only those imports/functions. Every remaining
non-import statement is byte-exact against the snapshot; component state, JSX,
effects and callbacks are untouched. Multi-line import formatting is retained
for multi-name groups; line-count progress does not rely on collapsing imports.
Entry 1971 -> 1854 effective lines. No active model/provider/filter behavior removed.

Console tests pass 50/50; typecheck and web build pass. Final ratchet and diff pass
after import formatting. Evidence: before-import-cleanup.tsx,
clean-console-imports.mjs --verify, import-cleanup-{tests,typecheck,web-build} and
final-ratchet logs. Source re-read, UTF-8 without BOM verified. Entry debt and
native release remain open; no Cargo transfer or structural clearance credit.

## Provider metrics ownership

Extracted providerModelMapEntries and providerMetricsResolver into
useConsoleProviderMetrics.ts, 79 effective lines; entry 1854 -> 1799. The cohesive
owner translates pool model names to upstream overrides and resolves provider or
provider/model metrics with billing multipliers. Missing telemetry remains null;
pooled credentials still use provider rollups without duplicate traffic counting.
Both memo expressions/dependencies and all remaining root statements are exact.
Comments now live beside the owning projections. No API, state or effect moved.

Console/telemetry/provider metrics tests pass 61/61; typecheck, web build, ratchet,
diff and UTF-8 without BOM checks pass. Evidence: before-provider-metrics.tsx,
verify-provider-metrics.mjs and provider-metrics-{tests,typecheck,web-build,ratchet}.
Entry debt and native release remain open; no Cargo transfer or clearance credit.

## Group selection ownership

Extracted useConsoleGroupSelection.ts, 92 effective lines. The owner contains
group directory/membership projections, selected row/query/mode state and the
existing selection-normalization effect. Draft mutations stay in the group editor.
Entry 1799 -> 1758 (the pre-extraction snapshot is 1800 after separating two
declarations previously on one line). All 11 moved statements retain exact bodies;
every other root statement is unchanged. The relocated effect crosses only the
model selector's pure memo calls, preserving ordering relative to other effects.

Group/draft/probe-action/account-view tests pass 28/28; typecheck, web build,
ratchet, diff and UTF-8 without BOM checks pass. Evidence:
before-group-selection.tsx, verify-group-selection.mjs and
group-selection-{tests,typecheck,web-build,ratchet}. No new API, timer or resource
lifecycle. Entry debt/native release/Cargo transfer remain open; no clearance credit.

## Console action identity and save lifecycle

Extracted useConsoleActionIdentity.ts with the request type, shared identity refs,
grant epoch and request/recovery guards. Initial owner87 effective lines and entry
1758 ->1702; exact statement/type-transfer proof passed. Three new regressions
then failed: an action remained current after unmount or after API/token A -> B -> A.
Cleanup now increments the action generation and currentness requires a mounted
owner. Immediate grant revocation recovery remains allowed; grant restoration and
direct grant replacement reject recovery. Independent review confirmed that boundary.

Save busy state now shares this lifecycle and resets on API/token identity change,
so suppressing an obsolete finally block cannot leave the new session stuck saving.
Final owner100 and entry1703 effective lines. Combined console/identity/probe tests
pass 64/64, including busy reset and direct grant replacement. Final typecheck,
web build, ratchet and diff pass; source re-read and UTF-8 without BOM checked.

Evidence: before-action-identity.tsx, verify-action-identity.mjs (pre-hardening
proof), action-identity-races-before.json, action-identity-final-tests.json and
final-typecheck/web-build/ratchet logs. This protects publication of in-flight save
and Gemini actions; it does not cancel requests or prohibit every retained action
callback from starting transport work. Probe-specific API/token restoration uses
separate generations and remains a follow-up audit. No native release, clearance
or Cargo transfer; owned gate sessions terminal.

## Probe identity restoration repair

Four additional regressions demonstrated that probe-specific generations could
revive a provider success or credential error after API/token A -> B -> A.
The existing probe cleanup now runs on API/token changes as well as unmount.
It invalidates both probe generations; setup clears obsolete probe snapshots and
busy/error state. Catch paths check request identity before invoking shared secret
recovery. Retained callbacks from another API/token are rejected before transport.
Grant epoch/revocation recovery logic and the enabled/draft revision guards remain.

Before: 7 pass, 4 fail. Combined console/probe/action-identity tests pass 68/68;
final hook tests pass 13/13 including retained credential/provider callbacks and
busy/snapshot reset. Final typecheck/ratchet/diff pass, web build post-repair passes.
Tests model the root's actual API/token ref updates during rerender. Evidence:
probe-restoration-{before,tests,final-tests}.json and final-typecheck, final-ratchet,
web-build logs. Source re-read. Already-issued requests still run remotely; no
transport cancellation or secret-grant callback reuse guarantee is claimed.
Entry remains1703; no native release, clearance or Cargo transfer. Sessions terminal.

## Gemini credential import ownership

Extracted useGeminiCredentialImport.ts, 200 effective lines; entry1703 ->1579.
The owner contains generated-session deduplication, route/secret draft application
and the existing auto-persist path. Session creation/polling remains in its separate
hook. Auto-persist still excludes Business and requires no non-keep secret patches;
request/currentness/recovery checks remain delegated to shared action identity.
Both callback bodies and the dedupe ref declaration are exact; all other root
statements and effect ordering are unchanged. No new behavior introduced.

Gemini console/session/draft/secret tests pass 14/14; typecheck, web build, ratchet,
diff and UTF-8 without BOM checks pass. Evidence: before-gemini-import.tsx,
verify-gemini-import.mjs and gemini-import-{tests,typecheck,web-build,ratchet}.
Follow-up audit remains for dedupe lifetime, retained apply callbacks and fallback
toast publication when auto-persist reports stale recovery. No such hardening is
claimed by this extraction. Entry debt/native release/Cargo transfer remain open;
owned gate sessions terminal.

## Gemini fallback notification lifecycle

### Subsequent save and autosave extraction

`useConsoleDraftPersistence.ts` owns the save callback, attempted-signature ref,
readiness/pending projection and cancellable 1200 ms debounce. Its 141 effective
lines use explicit typed contracts from the existing draft, route-data and action
identity owners; those imports are type-only. The root retains hydration and dirty
beforeunload effects in their existing order. The root decreased from 1523 to 1466
effective lines. The callback body and debounce behavior were moved intact, with
the stable ref and state setters added to the callback dependency array.

Paired draft/group/action-identity suites passed 20/20 before and after. Typecheck,
web build, ratchet and diff checks passed, and both source files have no BOM.
Evidence: `target/effective-line-evidence/browser-console-owners/draft-persistence-*`.
No new transport, secret snapshot, retry loop or resource owner was introduced.

Independent review confirmed the effect order and highlighted existing attempted
signature semantics: a rejected commit waits for another edit. The source already
documents that policy, so this extraction does not silently enable retries after
network failure or across credentials. Recovery after a renewed secret grant and
refresh/save overlap still need focused runtime-contract tests; the review alone
does not establish a stale overwrite or authorize cross-identity resubmission.
All owned gates are terminal. Structural clearance and native-release status are
unchanged.


### Subsequent provider catalog extraction

The synchronous provider-catalog submit action was moved to
`useProviderCatalogEditor.ts` with typed draft/secret/workspace/error contracts.
Template lookup, JSON validation, first-account and route creation, secret staging,
draft publication and notification ordering are preserved. No API, asynchronous
lifecycle or additional secret copy was introduced. Setter dependencies are now
explicit. The new owner has 88 effective lines; the entry decreased from 1579 to
1523, so this does not clear the entry's remaining structural debt.

Existing integration coverage checks the committed provider, credential, model
routes and API-key patch path. The same targeted suites passed 6/6 before and
after extraction. Typecheck, web build, effective-line ratchet and diff checks
passed; both source files are UTF-8 without BOM. Gate evidence is recorded under
`target/effective-line-evidence/browser-console-owners/provider-catalog-*`.
All gate sessions are terminal. Native packaging awaits the existing Cargo
handoff; no release or ownership transfer is claimed.


Three regression cases reproduced stale global fallback notifications: deferred
secret-recovery failure after unmount/token replacement, and draft-only Business
fallback whose microtask ran after unmount. The import continuation now captures
the shared action generation after auto-persist starts (which may itself advance
that generation), and publishes fallback only while that generation remains current.
This reuses the verified action identity teardown without another lifecycle owner.

Tests compose the real route draft and action identity hooks with the console API
fixture. They also preserve once-per-session draft application after ordinary
commit failure, leaving the valid draft available for normal save; repeated polling
does not silently retry an old document over newer edits. Initial 1 pass/3 fail;
final Gemini/session/draft/secret suite18/18, typecheck/web build/ratchet/diff pass.
Evidence: gemini-import-races-{before,tests}.json and associated gate logs.
Source re-read. Original extraction proof is historical after this focused fix.
No automatic retry or remote cancellation added; entry1579 remains debt, native
release/Cargo transfer remain open. Owned sessions terminal.

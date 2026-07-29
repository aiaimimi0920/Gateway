# Gateway Credential Management UI Redesign

**Date:** 2026-07-29  
**Status:** Drafted for review  
**Scope:** `C:\Users\Public\nas_home\AI\GameEditor\Neuro\Gateway`  
**Primary frontend file today:** `C:\Users\Public\nas_home\AI\GameEditor\Neuro\Gateway\apps\desktop\src\features\console\BrowserConsoleApp.tsx`  
**Reference release root:** `C:\Users\Public\nas_home\AI\GameEditor\Neuro\release\Gateway`

## Goal

Redesign Gateway's credential and account management console so that it looks
and behaves more like a real operator admin surface, with information
architecture inspired by Sub2API's `/admin/accounts`, while still preserving
Gateway's true route-config-driven behavior.

The current Gateway console already exposes two relevant workspaces:

- `账号台账 / Accounts`
- `分组策略 / Groups`

But the current presentation is still dominated by provider buckets, nested
cards, and structured-editor semantics. The redesign must shift the operator
experience toward:

- a ledger-style account registry view;
- a distinct credential-group management view; and
- a stronger management-console visual structure.

The redesign is not a copy of Sub2API's code or database model. It is a
Gateway-native admin UI that borrows the stronger admin structure and visual
discipline of Sub2API's account console.

## Approved Product Direction

The approved direction is the "hybrid admin ledger" approach:

1. Keep Gateway's existing route-config-backed data model and draft semantics.
2. Redesign the account surface into a Sub2API-style ledger/table view.
3. Redesign credential groups into a distinct management view parallel to the
   account ledger.
4. Preserve Gateway's real behavior: edits still mutate the route-config draft
   and do not become active until the route configuration is validated and
   committed.

This is intentionally not the "full backend rewrite" option. The first
iteration prioritizes admin-quality structure and operator usability over a new
management API model.

## Reference Baseline

### Gateway today

The current console account and group UIs live in:

- `C:\Users\Public\nas_home\AI\GameEditor\Neuro\Gateway\apps\desktop\src\features\console\BrowserConsoleApp.tsx`

The current implementation already has:

- account summary metrics;
- account search, membership filtering, and enabled filtering;
- provider and vendor bucket grouping;
- explicit credential add, edit, and delete flows;
- credential probe support;
- a separate groups workspace with structured group editing; and
- route-config draft synchronization.

Relevant supporting contracts already exist in:

- `C:\Users\Public\nas_home\AI\GameEditor\Neuro\Gateway\apps\desktop\src\api\console.ts`
- `C:\Users\Public\nas_home\AI\GameEditor\Neuro\Gateway\apps\desktop\src\api\contracts.ts`
- `C:\Users\Public\nas_home\AI\GameEditor\Neuro\Gateway\apps\desktop\src\api\schemas.ts`

### Sub2API reference

The local Sub2API runtime currently redirects `/admin/accounts` to login, so
the visual and structural reference is taken from its frontend source, not from
an authenticated live capture:

- `C:\Users\Public\nas_home\AI\GameEditor\AIGateway\tmp-sub2api-weishaw\frontend\src\views\admin\AccountsView.vue`
- `C:\Users\Public\nas_home\AI\GameEditor\AIGateway\tmp-sub2api-weishaw\frontend\src\components\admin\account\AccountTableFilters.vue`
- `C:\Users\Public\nas_home\AI\GameEditor\AIGateway\tmp-sub2api-weishaw\frontend\src\router\index.ts`

The useful lessons from Sub2API are structural:

- one main account ledger rather than provider-bucket-first presentation;
- strong toolbar, summary, and table hierarchy;
- explicit filter and action areas;
- groups treated as a separate management concern; and
- denser admin-oriented layout and status presentation.

## Constraints

### Must preserve

- Gateway's current draft-based route-config editing semantics.
- Existing credential probe behavior and secret-grant handling.
- Existing explicit credential creation and removal flows.
- Existing account group to route-config synchronization.
- The ability to return to route editing and advanced JSON workspaces.

### Must not pretend

The redesign must not suggest that Gateway now owns a separate database-backed
account registry. The UI may look like an admin console, but its mutation model
must remain honest:

- account and group edits change the current draft;
- validation and commit still happen through the route-config flow; and
- probe and read-only inspection actions remain immediate API-driven actions.

### Explicitly out of scope for this redesign

- introducing a new database-backed credential model;
- replacing the current route-config document structure;
- adding broad batch-edit workflows similar to Sub2API;
- building analytics-heavy group dashboards;
- adding drag-and-drop group ordering;
- implementing automatic rule-driven grouping; or
- redesigning unrelated console workspaces.

## Information Architecture

The redesigned experience introduces two primary admin workspaces plus one
return path:

1. `账号台账 / Accounts Ledger`
2. `凭证分组 / Credential Groups`
3. `返回路由编辑 / Back to Route Editor`

The key structural change is that providers stop being the page skeleton.
Providers remain an important attribute of each account, but they become row
metadata, filters, and chips rather than the dominant layout grouping.

### Workspace responsibilities

#### Accounts Ledger

The account ledger is the primary operator surface. It is responsible for:

- viewing all managed account units;
- searching and filtering those account units;
- inspecting account status and group assignment;
- probing connectivity;
- editing or deleting explicit credentials; and
- creating explicit credentials from provider-default entries.

#### Credential Groups

The credential groups workspace is a parallel management surface. It is
responsible for:

- viewing all defined groups;
- editing group metadata;
- inspecting group membership and coverage;
- assigning and removing accounts from groups; and
- surfacing ungrouped accounts more clearly.

#### Route Editor return path

The route editor remains part of the product, but it is demoted from being the
dominant visual anchor for this area. It becomes the authoritative place to
validate and commit draft changes rather than the first surface every operator
must work through.

## Accounts Ledger Design

### Overall layout

The account ledger is a single admin page with four visual layers:

1. title and high-level description;
2. summary cards;
3. filter and action toolbar; and
4. a dense ledger table with empty states.

This replaces the current provider bucket cascade of:

- bucket summary;
- provider card;
- account list inside provider card.

### Header area

The top section contains:

- title: `账号台账 / Accounts`;
- one sentence describing the surface as the unified view of routable Gateway
  accounts, group assignment, and connectivity state; and
- actions:
  - `添加账号 / Add account`
  - `进入凭证分组 / Open credential groups`
  - `返回路由编辑 / Back to route editor`

The descriptive copy should be operator-facing, not schema-explaining. It must
stop foregrounding "route-config provider credentials" as the product message.

### Summary cards

The account ledger retains four stable summary metrics already available from
Gateway's current derived catalog:

- account total;
- group total;
- ungrouped account total; and
- vendor or provider family total.

These move from a flat inline definition list to clearly separated summary
cards so the page reads like a management console rather than a developer pane.

### Filter and action toolbar

The toolbar is split conceptually into filters on the left and actions on the
right.

#### Filters

Approved first-release filters:

- free-text search;
- grouping status:
  - all;
  - grouped;
  - ungrouped;
- enabled status:
  - all;
  - enabled;
  - disabled;
- group filter:
  - all groups;
  - individual group;
  - optionally ungrouped as an explicit filter value; and
- provider or vendor filter:
  - all;
  - one provider family or vendor bucket.

The free-text search must match current meaningful account metadata, including:

- account display name;
- provider label;
- credential identifier;
- supported model names; and
- group names.

#### Actions

Approved first-release actions:

- add account;
- open credential groups; and
- return to route editor.

The redesign deliberately does not introduce large-scale bulk actions yet.
Gateway's current data model and user goal do not justify forcing a false
Sub2API-style batch-management system into this first pass.

### Ledger table

The ledger uses a single table-like registry view, not nested provider cards.
Each row represents one `RouteManagedAccount`-style unit already produced by
the existing summary logic.

Approved first-release columns:

1. **Account**
   - primary line: `displayName`;
   - secondary metadata: account id, host label, or provider-specific context.
2. **Provider**
   - vendor name or provider label;
   - optional preset indicator as subdued metadata.
3. **Credential Mode**
   - provider default credential;
   - explicit credential.
4. **Enabled Status**
   - enabled;
   - disabled.
5. **Model Coverage**
   - model chips or a compact aggregated rendering.
6. **Credential Groups**
   - group chips;
   - explicit ungrouped label when no group is present.
7. **Probe Status**
   - not tested;
   - passed;
   - failed;
   - unsupported;
   - probe error;
   - optional last checked timestamp and short result message.
8. **Actions**
   - probe;
   - edit or delete for explicit credentials;
   - add explicit account for provider-default entries.

### Row actions

The row-action design must become tighter and more admin-like than the current
button strip. The first release should prefer:

- one or two highly visible primary actions; and
- a secondary "more" pattern for less common actions if necessary.

Behaviorally:

- explicit credentials may be edited and deleted;
- provider-default entries may offer "add explicit account";
- probe remains available where current probe preconditions allow it.

### Empty states

The account ledger needs two distinct states:

1. **No accounts exist**
   - explain that the current route configuration exposes no manageable
     credentials;
   - offer actions to add accounts or return to route editing.
2. **Accounts exist, but filters match nothing**
   - explain that the current filters produced no result;
   - offer a clear reset or view-all action.

These two states must not share one generic message.

## Credential Groups Design

### Overall layout

The groups workspace must stop rendering as a vertical stack of large form
cards. It becomes a proper management view with:

1. title and summary area;
2. a left-hand group list;
3. a right-hand detail editor and membership manager.

This is the largest structural shift in the redesign.

### Header and summary

The page header contains:

- title: `凭证分组 / Credential Groups`;
- a short operator description focused on reusable credential pools and routing
  metadata; and
- actions:
  - add group;
  - back to accounts ledger.

Approved summary cards:

- total groups;
- enabled groups;
- ungrouped account count; and
- total selectable accounts.

The old static "cross-provider enabled" summary is demoted to explanatory copy.
It does not deserve primary summary-card prominence.

### Master-detail layout

#### Left pane: group list

The left pane becomes a compact group directory. Each group entry displays:

- group name;
- group id;
- enabled or disabled status;
- member count;
- billing multiplier; and
- compact provider coverage summary.

Clicking a group selects it and loads it into the detail pane.

This solves the current problem where operators must scroll through a long,
repeating stack of oversized group forms.

#### Right pane: group detail

The right pane edits and inspects the selected group. It is divided into three
sections:

1. base metadata;
2. membership summary; and
3. membership management.

### Group base metadata section

This section edits the current group's:

- group id;
- group name;
- enabled state;
- billing multiplier;
- description; and
- notes.

Existing validation rules stay intact:

- group id remains required;
- billing multiplier must parse to a valid number under the current rules.

The improvement is primarily structural and visual: metadata editing becomes a
clear form section rather than one portion of a sprawling card.

### Membership summary section

Each selected group should show a lightweight summary of what the group
contains, including:

- member count;
- provider or vendor coverage;
- model coverage summary; and
- ungrouped-account context where useful.

This gives the groups page a stronger management feel. It is not only a form;
it also becomes a readable state view.

### Membership management section

The current provider-bucket-style checkbox cloud is replaced by a tighter
candidate-account manager:

- top row: membership filters;
- main body: compact candidate-account list.

Approved candidate-account filters:

- free-text search over account, provider, and model;
- only current members;
- only ungrouped accounts; and
- optionally all candidates.

Approved candidate-account row fields:

- account display name;
- provider;
- enabled state;
- mode;
- current group assignment summary; and
- join or remove control.

The crucial rule is the same as in the account ledger: provider remains a field
and filter, not the dominant page skeleton.

### Group creation behavior

Creating a new group adds a draft group entry to the left pane and immediately
selects it in the right-hand editor. This is preferable to appending another
large vertical card at the bottom of the page.

### Group empty states

The groups workspace needs at least two distinct empty states:

1. **No groups exist**
   - explain that no credential groups are defined yet;
   - offer a clear create-first-group action.
2. **Member filter matches nothing**
   - explain that no candidate account matches the current member filter;
   - offer filter reset.

## Shared Behavior Model

### Draft-first truth

The redesign must preserve Gateway's draft-first truth model.

The UI can feel like a polished admin console, but it must remain honest about
the fact that:

- account mutations edit the route-config draft;
- group mutations edit the route-config draft; and
- final runtime activation still depends on validation and commit elsewhere in
  the console.

### Immediate actions versus draft actions

To keep the operator model clear, actions are divided into two categories.

#### Immediate actions

These are real-time actions that do not require draft commit:

- credential probe;
- switching workspaces;
- opening dialogs;
- local filtering and sorting.

#### Draft-mutating actions

These change the route-config draft:

- add explicit credential;
- edit explicit credential;
- delete explicit credential;
- add group;
- edit group metadata;
- add or remove group members.

### Draft state messaging

The console should continue showing draft-state messaging, but in a lighter
operator-friendly form. The redesign should preserve visibility of:

- unsaved draft changes;
- stale active revision concerns when relevant; and
- the fact that users may return to route editing for validation and commit.

The draft warning should remain truthful without dominating the visual design.

## Error Handling and Status Presentation

### Account ledger statuses

The account ledger must preserve current probe semantics and present them more
cleanly:

- not tested;
- connectivity passed;
- connectivity failed;
- probe unsupported; and
- probe error.

Existing probe-disabled conditions remain authoritative. The UI should make
disabled state clearer and provide a useful explanatory title or label where
practical.

### Group form validation

Group detail editing preserves existing validation behavior:

- missing group id shows an inline field error;
- invalid billing multiplier shows an inline field error.

The redesign should keep validation local to the affected field and avoid
making the entire page feel like a raw technical form dump.

### Membership editing feedback

Frequent membership changes should prefer quiet draft-state feedback over
heavyweight toast noise. Editing group members is expected to be iterative, so
the UI should avoid excessive interruption.

## Component Boundaries

The current `BrowserConsoleApp.tsx` is already too large for this redesign to
land cleanly as one more in-place expansion. The redesign should be
implemented with targeted component extraction directly related to this work.

### Required extraction direction

At minimum, extract:

- an account-ledger workspace component; and
- a credential-groups workspace component.

### Recommended subordinate components

Suggested component boundaries:

- `AccountsLedgerWorkspace`
- `AccountsLedgerTable`
- `AccountStatusBadge`
- `AccountGroupChips`
- `AccountRowActions`
- `CredentialGroupsWorkspace`
- `GroupListPane`
- `GroupDetailForm`
- `GroupMembersPanel`

Names may vary, but the boundary goals are fixed:

- account-ledger rendering and logic should stop living entirely inline in the
  giant host file;
- group-management rendering and logic should stop living entirely inline in
  the giant host file; and
- shared derived view-model helpers may remain local or move into focused
  utility files if that keeps responsibilities clear.

This is not a general cleanup of the entire console. It is a focused structural
split required to keep this redesign maintainable.

## Data Flow

### Accounts ledger

The first implementation keeps using Gateway's current derived account catalog
pipeline rather than building a new API surface. The ledger is derived from the
same underlying summary sources already used today, including:

- account catalog rows;
- provider metadata;
- group membership metadata; and
- probe-result state.

The redesign changes how those facts are organized on screen, not where they
come from.

### Credential groups

The groups workspace keeps using the existing group draft rows and current
account catalog data. The redesign changes the presentation into:

- left-pane list items;
- right-pane selected-group detail;
- candidate-member rows; and
- group coverage summaries.

### No backend contract expansion required for phase one

This phase deliberately avoids turning the redesign into a backend rewrite.
Later iterations may choose to expose richer provider-account or
provider-credential endpoints to the desktop console, but that is not required
to deliver the approved UI direction.

## Test Strategy

The redesign must preserve existing behavior while adding structural coverage
for the new admin layout.

### Existing behavior that must remain covered

Current tests in:

- `C:\Users\Public\nas_home\AI\GameEditor\Neuro\Gateway\apps\desktop\src\features\console\BrowserConsoleApp.test.tsx`

already cover important behavior such as:

- adding explicit credentials;
- probing credentials;
- stale probe-result handling;
- secret-grant handling; and
- group-edit synchronization with the route-config draft.

Those behavior contracts must continue to pass after the redesign.

### New structure-focused tests

Add or update tests to verify:

- the account workspace renders a ledger-style table or list rather than a
  provider-bucket-first cascade;
- account filters still work, including group and enabled filtering;
- provider information is shown as account metadata rather than only through
  bucket headings;
- the groups workspace renders a master-detail structure;
- selecting a group changes the visible right-pane detail;
- group membership management uses compact candidate rows rather than the old
  provider-bucket selector as the primary structure.

### Empty-state tests

Add coverage for:

- no accounts exist;
- account filters yield no results;
- no groups exist; and
- group member filters yield no results.

## Success Criteria

The redesign is successful when all of the following are true.

### Product and UX

- The account surface looks like an operator ledger instead of a nested
  configuration viewer.
- The groups surface looks like an independent admin management page instead of
  a stack of draft cards.
- Provider information still matters, but it no longer dictates the primary
  page skeleton.
- Credential groups are visibly first-class in the operator experience.

### Behavioral correctness

- Draft-based route-config truth is preserved.
- Probe and secret-grant logic are unchanged in meaning.
- Group membership edits still synchronize with the underlying route-config
  draft.
- Account add, edit, and delete actions still map to the correct draft changes.

### Engineering quality

- The redesign lands with clearer component boundaries.
- `BrowserConsoleApp.tsx` stops absorbing all account and group complexity
  inline.
- The test suite protects both the existing behavioral contract and the new
  structural expectations.

## Follow-up Opportunities After This Design

These are explicitly deferred until after the approved redesign is shipped:

- bulk account actions;
- richer sorting and column configuration;
- account-detail side panels;
- group cloning or reordering;
- richer provider-account and provider-credential backend integration for the
  desktop console; and
- more advanced visual polish beyond the approved structural rewrite.

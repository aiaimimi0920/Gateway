# Gateway Web Console Design

**Date:** 2026-07-22  
**Status:** Approved for implementation  
**Scope:** `C:\Users\Public\nas_home\AI\GameEditor\Neuro\Gateway`  
**Release root:** `C:\Users\Public\nas_home\AI\GameEditor\Neuro\release\Gateway`

## Goal

Make the Gateway a single product with two equivalent operator surfaces:

1. the headless Rust Gateway HTTP runtime, and
2. a complete browser control console served by that same Gateway at `/ui/`.

The existing Tauri application remains useful for local process and filesystem
operations, but it must use the same React console and the same management API
contract. A user must be able to inspect and change the active standalone
`routes.yaml` configuration from a browser, validate it with the real routing
compiler, activate it without a restart, inspect revisions, and recover from a
failed or interrupted save.

This design deliberately keeps development credentials and the current
`routes.yaml` values intact. It does not delete, rotate, redact, or migrate
those values as part of implementation.

## Product Boundaries

### In scope

- loopback-first administrator bootstrap and management-token login;
- one browser administrator session model, without public multi-user RBAC;
- overview, providers, credentials, routes, aliases, revisions, models,
  playground, runtime, events, logs, and settings screens;
- a standalone configuration runtime independent of PostgreSQL;
- Redis active-revision state and a portable YAML mirror;
- deterministic revisions, optimistic concurrency, diff, rollback, import, and
  export;
- atomic YAML persistence, transaction journaling, startup recovery, and
  bounded event streaming;
- embedded `/ui/` assets that cannot expose the working directory or release
  payload;
- a browser adapter and a Tauri adapter sharing the React feature code;
- focused Rust, TypeScript, Python contract, and Playwright verification.

### Explicitly out of scope for the first release

- public Internet multi-user access, invitations, teams, or role management;
- replacing the existing PostgreSQL enterprise control plane;
- arbitrary environment-variable editing or arbitrary process execution from
  the browser;
- a browser-triggered restart of a standalone process;
- storing management credentials in cookies, local storage, URLs, HTML, or the
  JavaScript bundle;
- changing the existing development secrets in `routes.yaml`.

PostgreSQL-backed provider/account features may be exposed as an advanced
read-only or explicitly database-mode surface later, but they are not a
second write path for the standalone document described here.

## Existing Runtime Baseline

The current runtime builds one `AppState` containing an `Arc<RouteConfigStore>`.
The store is compiled once at startup. Startup checks Redis key
`gw:config:routes` first and falls back to the path in `GATEWAY_ROUTES_FILE`.
The existing compiler already understands:

- `RouteConfigYaml`;
- `ProviderConfigYaml`;
- `ProviderCredentialYaml`;
- `ModelRoute`; and
- aliases.

The console must reuse these types and compiler rules. It must not introduce a
second provider schema, a second model glob implementation, or a fake database
CRUD layer that cannot modify the active standalone configuration.

The current management token checker is handler-oriented and the global router
allows any CORS origin. The console adds a dedicated management boundary and
security headers without changing the public compatibility API contract more
than necessary.

## Architecture

```text
Browser at /ui/                         Tauri desktop shell
        | same-origin management API             | same React features
        +--------------------+-------------------+
                             |
                 Gateway management router
       bootstrap / session / config / revisions / events
                             |
                    RouteConfigRuntime
          validation -> journal -> persistence -> CAS
                             |
        +--------------------+-------------------+
        |                    |                   |
  RouteConfigStore     Redis active state     YAML mirror
  atomic in-memory     revision + CAS         portable file
        |                    |                   |
                    canonical request pipeline
```

### Runtime roles

- **Standalone:** the process owns the writable `RouteConfigRuntime`, serves
  `/ui/`, and swaps its in-memory route store after a committed transaction.
- **Splitter/manager:** the manager owns writes and publishes the committed
  revision. Workers expose the same read-only console data and subscribe to the
  revision event; each worker compiles and atomically swaps its local store.
  A worker never writes a YAML file or changes the active Redis pointer.
- **Tauri:** the shell may start, stop, and inspect a sidecar through native
  commands, but all provider and route mutations go through the management API.

The first implementation must keep standalone fully functional even when
PostgreSQL is absent. Splitter behavior must not regress; if a role cannot
perform a mutation it returns a typed `console_mutation_not_supported` response
instead of silently editing a local file.

## RouteConfigRuntime

Add `src/console/route_config_runtime.rs` and `src/console/mod.rs` as the
Gateway-owned management runtime. Keep the request-facing compiled data and
revision metadata in one immutable snapshot owned by `RouteConfigStore`; the
runtime service owns persistence, mutation serialization, and recovery around
that store.

```rust
pub struct RouteConfigRuntime {
    store: Arc<RouteConfigStore>,
    mutation_lock: Mutex<()>,
    persistence: RouteConfigPersistence,
    events: ConsoleEventBus,
}

pub struct RouteConfigSnapshot {
    pub revision: RevisionId,
    pub document: RouteConfigYaml,
    pub compiled: Arc<RouteConfigInner>,
    pub source: ActiveConfigSource,
    pub committed_at: OffsetDateTime,
}
```

`RouteConfigStore` is changed from a lock around mutable fields to an
`ArcSwap<RouteConfigSnapshot>` using the `arc-swap` crate. Its existing query
methods load one snapshot and delegate to `compiled`; `replace_snapshot` swaps
the whole object at once. `RouteConfigRuntime` never updates provider lists,
routes, aliases, or revision metadata separately. Every request observes the
complete old snapshot or the complete new snapshot.

### Canonical document and revision

The runtime keeps both the structured document and its compiled store. A
revision contains:

- a monotonic sequence scoped to the Gateway instance;
- a content SHA-256 over deterministic UTF-8 canonical JSON;
- the canonical YAML representation;
- the original raw YAML bytes when first managed;
- a redacted JSON view for API responses;
- actor (`bootstrap`, `management-token`, `environment-override`, or
  `recovery`), timestamp, parent revision, and change summary.

The public revision identifier is `r<sequence>-<sha256-prefix>`. The hash is
never derived from masked values: secret values are included in the server-side
revision archive, while API responses expose only fingerprints and previews.

### Active source reporting

Every overview and configuration response reports one of:

- `yaml`;
- `redis`;
- `database` (read-only enterprise mode); or
- `recovered`.

The response also includes the resolved YAML path, state directory, active
revision, Redis synchronization status, and whether the current process can
write. A successful UI save must never appear to succeed while the process will
silently reload an older Redis value on the next restart.

## Persistence, Transaction, and Recovery

### State layout

The writable state directory is selected in this order:

1. explicit `GATEWAY_STATE_DIR`;
2. the Tauri profile's application-data state directory;
3. a development fallback adjacent to the configured routes file.

Release launchers must always pass a writable state directory outside the
immutable release payload. A release binary must never create journals,
backups, logs, or temporary files below the signed package directory.

The state directory contains:

```text
console/
  admin.json                         # salted management-token hash metadata
  revisions/<revision-id>/
    document.json                    # full server-side document
    routes.yaml                      # canonical revision bytes
    metadata.json                    # parent, actor, timestamps, digest
  transactions/<transaction-id>.json # prepared/committing/committed/aborted
  journal.ndjson                     # append-only recovery journal
  events.ndjson                      # bounded, redacted operator events
  backups/routes-first-save.yaml     # first-save raw file backup
```

Redis stores the active pointer and the current canonical document for fast
worker convergence. Existing `gw:config:routes` remains a compatibility mirror;
new code additionally stores an active revision pointer, revision payload,
transaction record, and a pub/sub notification under a namespaced
`gw:console:route-config:*` key family. The namespace is configurable so two
development instances cannot overwrite each other's revisions.

### Commit protocol

All provider, credential, model-route, and alias mutations use this sequence:

1. Authenticate the management request and check origin/remote policy.
2. Read the current immutable snapshot and require `expectedRevision` (and, if
   present, matching `If-Match`).
3. Apply the structured document plus secret patches to a private candidate.
4. Run the real RouteConfig compiler and all console-level cross-reference
   checks before taking the mutation lock.
5. Acquire the mutation lock, re-read the active revision, and reject a stale
   candidate with `409 revision_conflict` and a server diff.
6. Allocate the next revision and write an immutable revision archive.
7. Append a `prepared` journal record and persist a prepared transaction marker
   to Redis while the old active pointer remains authoritative.
8. Write a same-directory YAML temporary file using UTF-8 without BOM; flush
   and `sync_all` it. Preserve the previous bytes and file metadata needed for
   recovery.
9. Replace the target YAML atomically using a platform-safe overwrite
   operation. Windows must use an overwrite-capable replacement primitive, not
   delete-then-rename. POSIX uses an atomic rename after directory sync.
10. Execute a Redis compare-and-swap that verifies the old active revision and
    changes the pointer/document to the new revision. Publish the revision
    notification only after the CAS succeeds.
11. Atomically swap the in-memory `RouteConfigSnapshot`.
12. Mark the transaction `committed`, append the journal record, and emit a
    redacted `config_committed` event.

If any step before the Redis CAS fails, live traffic remains on the old memory
snapshot. If YAML replacement succeeds but the CAS fails, the runtime attempts
an atomic restore of the previous bytes and marks the transaction aborted. If
the process exits after the CAS but before the memory swap or final journal
record, startup recovery treats the Redis active pointer and immutable revision
archive as authoritative, completes the YAML mirror, and loads that revision.

### Startup recovery

At startup, before serving management mutations:

1. read and validate the journal and transaction records;
2. reject malformed or path-escaping records;
3. compare the Redis active pointer, YAML digest, and last local committed
   revision;
4. finish a durable committed transaction, or roll back a prepared transaction
   whose CAS never succeeded;
5. compile the chosen document and install one initial snapshot;
6. emit a `recovery_completed` or `recovery_required` event.

Recovery is idempotent. A disk/Redis failure that cannot be repaired does not
replace the live in-memory snapshot; it marks configuration as read-only and
returns an actionable diagnostic from the console. Requests continue using the
last-known-good compiled snapshot until the operator repairs storage.

### Diff, rollback, import, and export

- Diff compares normalized structured documents and also shows raw YAML byte
  changes for operators who care about comments or formatting.
- Rollback creates a new child revision from an older immutable revision; it
  never rewrites or deletes history.
- Import validates a submitted YAML document without writing first. The UI
  shows diagnostics and a diff, then uses the normal commit protocol.
- Export returns the canonical YAML as a download response with
  `Cache-Control: no-store`; secrets are included only after a secret-access
  confirmation and are never embedded in a URL.

## Management Bootstrap and Session

### Bootstrap rules

The bootstrap status endpoint is intentionally unauthenticated but reveals no
configuration or secret material. When no stored administrator hash exists and
no environment management token is configured:

- `GET /v1/internal/gateway/console/bootstrap/status` returns
  `needsBootstrap: true` only to loopback clients;
- a remote client receives `403 console_bootstrap_loopback_only` and cannot race
  the first local administrator;
- `POST /.../bootstrap` is accepted once, from loopback, with a user-supplied
  token meeting the configured length/entropy policy;
- the token is stored as a salted Argon2id hash using the `argon2` crate, never
  as plaintext;
- the bootstrap record is fsync'd before the success response and the
  unauthenticated bootstrap path is permanently closed.

`GATEWAY_MANAGEMENT_TOKEN` remains an environment override and recovery path.
When set, it is not persisted or replaced by bootstrap. Token comparisons use
constant-time equality. The response never echoes the token.

### Browser session

The browser stores the administrator token only in `sessionStorage`. Every
management request sends it in `x-management-token`; no cookie or URL token is
used. `POST /session/verify` returns capabilities, role, active revision, and
whether secret access is currently granted, but not the token.

`POST /session/rotate` requires the current token and a new token. The new
stored hash is committed atomically; an environment override cannot be rotated
from the UI. The browser clears its session on `401`, `403`, or an explicit
logout.

### Secret access confirmation

Masked configuration reads expose `configured`, a stable fingerprint, and a
short preview only. A user who needs to reveal or edit a complete value must
submit the management token again to
`POST /session/confirm-secret-access`. The server returns a short-lived,
single-purpose secret grant stored server-side (or in namespaced Redis) and
bound to the authenticated token hash, client origin, and expiry. The grant is
not put in a URL and is never logged. Secret reads require this grant and
`Cache-Control: no-store`.

## Secret Patch Contract

The API never treats a masked value such as `***`, `sk-...`, or a preview as a
real credential. A configuration mutation contains:

```json
{
  "expectedRevision": "r18-4a21d9e8",
  "document": { "providers": [], "model_routes": [], "aliases": {} },
  "secretPatches": [
    { "path": "/providers/0/api_key", "operation": "keep" },
    { "path": "/providers/0/credentials/0/api_key", "operation": "replace", "value": "..." },
    { "path": "/providers/0/headers/Cookie", "operation": "clear" }
  ],
  "message": "enable gpt-5.4 route"
}
```

The only accepted operations are:

- `keep`: retain the value from the expected active revision;
- `replace`: set the supplied value exactly, including empty values only where
  the field schema permits them;
- `clear`: remove the value explicitly.

The server rejects a document containing a mask sentinel in a known secret
field, a patch path outside the schema, a patch whose base revision is stale, or
a `replace` operation missing its value. Sensitive keys include API keys,
tokens, cookies, passwords, client secrets, authorization headers, proxy
authorization, refresh tokens, and provider-specific session fields. The
masker and patch resolver are shared by provider and credential responses.

## HTTP API Contract

All console API responses are JSON, include `Cache-Control: no-store`, and use
the existing Gateway error envelope with stable `code` values. Management
routes are mounted under `/v1/internal/gateway/console`.

### Bootstrap and session

```text
GET  /bootstrap/status
POST /bootstrap
POST /session/verify
POST /session/confirm-secret-access
POST /session/rotate
POST /session/logout
```

### Configuration and revisions

```text
GET  /route-config
POST /route-config/validate
PUT  /route-config
POST /route-config/reload
POST /route-config/reveal

GET  /presets
GET  /runtime
GET  /revisions
GET  /revisions/:revision
GET  /revisions/:revision/diff
POST /revisions/:revision/rollback
POST /revisions/import
GET  /revisions/:revision/export
```

`GET /route-config` returns the redacted document, revision metadata, source,
write capability, secret descriptors, diagnostics, and an `ETag` containing the
revision. `POST /validate` never writes and returns normalized output plus
compiler diagnostics. `PUT` requires `expectedRevision` and follows the commit
protocol above. `reload` re-reads the persisted active revision and is only
allowed when no transaction is in progress.

### Runtime, models, playground, events, and logs

```text
GET  /runtime
GET  /readiness
GET  /models
POST /playground/chat
GET  /events?cursor=&limit=&kind=
GET  /events/stream
GET  /logs/tail?lines=&cursor=
GET  /logs/stream
```

The playground is a thin management-facing client of the public Gateway API;
it does not bypass the canonical auth, route, send, or finalize pipeline. API
keys entered for a playground request are held only in the current page state.
The logs endpoint is bounded, path-confined to the configured Gateway log
source, and passes every line through the shared redactor. Events are preferred
for configuration and runtime state because they are structured and bounded.

### Concurrency and error statuses

- `401`: missing or invalid management credential;
- `403`: remote/bootstrap/origin/policy denial;
- `409`: stale revision or transaction conflict, with current revision and
  structured diff;
- `422`: YAML/schema/compiler diagnostics;
- `423`: configuration is temporarily locked or recovering;
- `424`: dependency (Redis or filesystem) unavailable;
- `503`: management credential not configured or runtime not ready.

Error bodies never include API keys, cookies, raw request headers, or the
candidate document.

## Frontend Information Architecture

The shared React console uses a dense operational layout based on the approved
mockup: fixed left navigation, compact header, table-first lists, restrained
graphite surfaces, green success, cyan information, yellow warning, and red
failure signals. Repeated cards use a radius of at most 8px. Buttons use the
existing icon library or an approved icon package; unfamiliar icon-only actions
have accessible labels/tooltips.

Navigation sections:

- **Overview:** readiness, active revision, source synchronization, request
  health, and recent events;
- **Providers:** provider preset, endpoint, model inventory, state, and probe;
- **Credentials:** credential descriptors, expiry/cooling, secret reveal/edit;
- **Routes & aliases:** model patterns, priorities, provider order, aliases;
- **Revisions:** timeline, diff, validation, import/export, rollback;
- **Models:** effective `/v1/models` inventory and route explanation;
- **Playground:** authenticated test request through the live Gateway;
- **Events:** bounded structured event list and SSE stream;
- **Runtime:** health/readiness, drain state, worker role, dependency state;
- **Settings:** console policy, state paths, appearance, and restart-required
  values shown explicitly as read-only where appropriate.

Every feature implements loading, empty, error, retry, stale-revision,
unsaved-draft, and narrow viewport states. Secret fields default to masked
descriptors; reveal, copy, replace, and clear actions require an explicit
confirmation affordance. The UI never displays the full key by default.

### Browser/Tauri adapter boundary

Shared modules own:

- typed management/public HTTP clients;
- session state and secret grant state;
- document editor, validation diagnostics, diff, and revision workflows;
- feature components, design tokens, and accessibility behavior.

The browser adapter uses same-origin `window.location.origin` and
`sessionStorage`. The Tauri adapter adds sidecar lifecycle, profile files,
native path probes, and desktop log access through a narrow capability
interface. No shared feature component imports `@tauri-apps/api` directly.
The Tauri origin is explicitly allowlisted by the console CORS policy; it does
not inherit the public API's unrestricted development CORS layer.

The current monolithic `useGatewayDesktopState` is split into connection/session,
route-config, overview, and desktop-capability stores. Existing launcher and
profile panels remain available in Tauri but are not presented as the browser's
route editor.

## Embedded Assets and `/ui/`

The React source has two deterministic build targets:

- `npm run build:web` writes `apps/desktop/dist/web`, uses `/ui/` as the asset
  prefix, and uses `BrowserRouter` with `basename="/ui"`;
- `npm run build:tauri` writes `apps/desktop/dist/tauri`, uses relative assets,
  and uses `HashRouter` for the Tauri webview.

The browser bundle and its SHA-256 manifest are versioned build inputs so a
clean Cargo checkout contains a usable console. The Rust binary embeds
`apps/desktop/dist/web` with the `rust-embed` crate. CI and packaging rebuild
the web bundle before Rust compilation and fail if the regenerated bundle or
manifest differs from the versioned snapshot. `tauri.conf.json` points to
`../dist/tauri`.

The Axum router serves:

- `/ui/` and known static assets from the embedded asset map;
- an SPA fallback to the embedded `index.html` for `/ui/<client-route>`;
- `404` for all paths outside `/ui/` that are not Gateway APIs.

The handler must never use `ServeDir` against the Gateway working directory,
release root, routes file parent, or state directory. Asset responses set
`Content-Type`, `X-Content-Type-Options: nosniff`, a strict CSP, and immutable
caching for fingerprinted assets. HTML and management JSON use `no-store`.
Missing `/ui/static/*` assets return `404`; only recognized client-side routes
receive the SPA fallback.

The release packager builds and embeds the browser assets before compiling the
release binary. It writes all mutable state to the configured external state
directory and leaves the signed release payload untouched.

## Security and Access Policy

### Network boundary

- The default console policy is loopback-only, regardless of the public API
  bind address.
- Remote console access requires an explicit configuration flag and an exact
  trusted-origin allowlist. It is intended to sit behind TLS; the Gateway does
  not infer trust from arbitrary `X-Forwarded-*` headers.
- Remote bootstrap is always rejected.
- A missing `Origin` is accepted for authenticated CLI automation only where
  the endpoint explicitly documents it; browser mutation endpoints require a
  same-origin or allowlisted origin.
- Public API CORS remains a separate policy. The console must not inherit
  `allow_origin(Any)`.
- Authenticated event and log streams use fetch-based SSE because native
  `EventSource` cannot send the management-token header and tokens are forbidden
  in query strings.

### Response and browser headers

Console HTML and API responses include:

- `Content-Security-Policy` with no inline script or object sources except the
  build's required nonce/hash;
- `X-Frame-Options: DENY` and `frame-ancestors 'none'`;
- `X-Content-Type-Options: nosniff`;
- `Referrer-Policy: no-referrer`;
- `Permissions-Policy` disabling unused browser capabilities;
- `Cache-Control: no-store` for HTML, JSON, secret, and event responses.

### Audit and redaction

Every mutation, failed authorization, bootstrap attempt, rollback, and recovery
event records actor class, request ID, revision, outcome, and error code. It
does not record tokens, secret values, raw Authorization/Cookie headers, or
candidate documents. The redactor is shared with existing provider-error and
desktop-diagnostics code and handles non-ASCII values without byte-index panic.

## Runtime Failure Semantics

- **Compiler failure:** return `422`; active traffic and revision remain
  unchanged.
- **Stale browser:** return `409`; keep the draft in the UI and offer a diff.
- **YAML write/replace failure:** return `424`; restore the old bytes if needed,
  keep the old in-memory snapshot, and mark storage degraded.
- **Redis CAS failure:** return `409` or `424` according to whether the active
  revision changed or Redis is unavailable; never claim activation.
- **Crash during commit:** startup journal/revision recovery converges to the
  last committed active revision.
- **Worker convergence failure:** worker remains on its last-good revision and
  reports `config_sync_degraded`; it does not partially apply the document.
- **Runtime drain:** the console may request drain and show progress, but a
  standalone process cannot promise to restart itself. Tauri/supervisor owns
  restart.

## Testing Strategy

Every production behavior follows a red-green-refactor cycle. The first test
for each behavior must fail for the intended missing behavior before code is
written.

### Rust

- document masking and `keep/replace/clear` patch application;
- compiler validation and cross-reference diagnostics;
- revision hash, deterministic serialization, and ETag behavior;
- atomic snapshot replacement and concurrent old/new request visibility;
- expected-revision CAS conflicts;
- same-directory atomic file replacement and backup creation;
- Redis prepared/active CAS and pub/sub payloads;
- journal recovery for every crash point in the commit sequence;
- loopback bootstrap, remote rejection, token hash/rotation, constant-time
  authorization, secret grant expiry, and no-store headers;
- embedded asset routing, SPA fallback, path traversal rejection, CSP, and
  no release/state directory leakage;
- event ring bounds, redaction, tail limits, and SSE disconnect behavior.

### Frontend

- typed API client error decoding and session expiry;
- bootstrap/login/secret-confirm flows;
- provider, credential, route, alias, revision, rollback, and conflict forms;
- masked secrets never serialized as replacement values;
- draft persistence only in memory, revision refresh, and unsaved-change guard;
- browser and Tauri adapter capability selection;
- responsive desktop/mobile layouts and keyboard/focus behavior.

### Integration and release

- start a fresh standalone Gateway with isolated Redis and a temporary state
  directory;
- open `/ui/`, bootstrap/login, add a test provider and route through the UI;
- call `/v1/models` and `/v1/chat/completions` through the same running Gateway;
- force compiler, disk, Redis, and process-interruption failures and verify
  recovery;
- run Playwright at desktop and mobile viewports, capture screenshots, check
  non-empty pixels and element overlap;
- build the browser assets, embed them, build the Windows release, and verify
  that all mutable output is below
  `C:\Users\Public\nas_home\AI\GameEditor\Neuro\release\Gateway` while
  mutable runtime state is outside the package.

The final validation gate includes the existing Rust, Python, Node, desktop
typecheck/build, package, and release-candidate suites plus the new console
contracts. No formal Git tag or GitHub Release is created by this feature
implementation.

## Delivery Order

1. Add pure route-document, secret-patch, revision, and persistence tests;
   expose the compiler as a reusable validation service.
2. Implement `RouteConfigRuntime`, journal/recovery, YAML atomic writer, Redis
   CAS, and in-memory snapshot replacement.
3. Add bootstrap/session/secret-grant and the dedicated management router with
   security headers and event bus.
4. Add embedded `/ui/` asset serving and contract tests before expanding the
   React screens.
5. Refactor the React shell into browser/Tauri adapters, then implement the
   approved information architecture and feature workflows.
6. Add integration/Playwright/release verification and update the progress
   ledger.

Each batch must preserve the pre-existing `M routes.yaml` change and stage only
the files belonging to that batch.

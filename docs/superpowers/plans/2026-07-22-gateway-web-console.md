# Gateway Web Console Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship a browser-accessible `/ui/` control console that safely manages the standalone Gateway's real route configuration through revisioned, recoverable transactions while sharing one React application with Tauri.

**Architecture:** A new `RouteConfigRuntime` wraps the existing route compiler and request-facing store with immutable snapshots, a single mutation lock, atomic YAML persistence, Redis CAS activation, journal recovery, and a protected management router. The React application is split into shared feature code plus Browser/Tauri host adapters and is built twice: `/ui/` assets embedded in Rust and relative assets for Tauri.

**Tech Stack:** Rust, Axum, Tokio, Redis Lua CAS, `arc-swap`, `argon2`, `subtle`, `rust-embed`, React 19, TypeScript, Rsbuild, React Router, TanStack Query, Zod, React Hook Form, Lucide, Radix primitives, Vitest, Testing Library, MSW, and Playwright.

---

## File Structure

New Rust modules:

```text
src/console/
  mod.rs                    # public console runtime boundary
  document.rs               # validation, canonical JSON/YAML, diagnostics
  secrets.rs                # masking and keep/replace/clear resolution
  revision.rs               # revision IDs, metadata, deterministic digest
  persistence.rs            # state layout and platform atomic file replace
  journal.rs                # transaction records and startup recovery input
  redis_store.rs            # namespaced Redis revision storage and Lua CAS
  runtime.rs                # commit/reload/rollback/recovery orchestration
  auth.rs                   # bootstrap hash, token verification, rotation/grants
  access.rs                 # loopback, remote, origin, and Tauri-origin policy
  events.rs                 # bounded redacted event ring and broadcast stream
  logs.rs                   # path-confined bounded log tail/stream
src/http/routes/internal_console/
  mod.rs                    # console subrouter
  auth.rs                   # bootstrap/session handlers
  config.rs                 # config/validate/reveal/reload handlers
  revisions.rs              # list/detail/diff/import/export/rollback handlers
  runtime.rs                # overview/runtime/models/playground handlers
  events.rs                 # events/log tail and fetch-SSE endpoints
src/http/ui.rs              # rust-embed asset lookup and SPA fallback
```

New frontend boundaries:

```text
apps/desktop/src/
  app/                      # providers, router, shell, navigation
  api/                      # Zod contracts, clients, typed errors, fetch-SSE
  session/                  # bootstrap/login/token/secret-grant state
  platform/                 # Browser and Tauri host adapters
  config/                   # shared route document draft and secret patches
  components/               # tables, dialogs, secret/revision controls
  features/                 # approved console pages
  styles/                   # tokens, base, shell, components, responsive
apps/desktop/src/test/       # Vitest/MSW setup and handlers
apps/desktop/e2e/            # Playwright console workflow
apps/desktop/dist/web/       # versioned embedded browser build
apps/desktop/dist/tauri/     # generated Tauri build
```

## Task 1: Add Console Configuration and Test Construction Helpers

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/config.rs`
- Modify: `src/state.rs`
- Modify: `src/runtime.rs`
- Create: `src/console/mod.rs`
- Create: `tests/console_config_contract.rs`
- Modify: existing `Config` and `AppState` literals found by `rg -n "Config \{|AppState \{" src tests`

- [ ] **Step 1: Write the failing configuration contract tests**

```rust
#[test]
fn console_defaults_are_loopback_first_and_namespaced() {
    let config = ConsoleConfig::from_values(ConsoleConfigValues::default()).unwrap();
    assert!(!config.remote_access_enabled);
    assert_eq!(config.redis_namespace, "default");
    assert_eq!(config.secret_grant_ttl_secs, 300);
    assert_eq!(config.max_event_records, 2_000);
}

#[test]
fn release_state_directory_must_not_be_inside_release_payload() {
    let error = validate_state_directory(
        Path::new(r"C:\release\Gateway\V1"),
        Some(Path::new(r"C:\release\Gateway\V1")),
    ).unwrap_err();
    assert_eq!(error.code(), "console_state_inside_release_payload");
}
```

- [ ] **Step 2: Run the tests and confirm the intended failure**

Run: `cargo test --locked --test console_config_contract -- --nocapture`  
Expected: compile failure because `ConsoleConfig`, `ConsoleConfigValues`, and `validate_state_directory` do not exist.

- [ ] **Step 3: Add direct dependencies and configuration fields**

Add direct dependencies for `arc-swap`, `argon2`, `subtle`, `rust-embed`, `mime_guess`, and target-specific `windows-sys` file-system APIs. Add a `ConsoleConfig` value to `Config` with these environment-backed fields:

```rust
pub struct ConsoleConfig {
    pub state_dir: PathBuf,
    pub routes_file: PathBuf,
    pub redis_namespace: String,
    pub remote_access_enabled: bool,
    pub trusted_origins: Vec<String>,
    pub tauri_origins: Vec<String>,
    pub secret_grant_ttl_secs: u64,
    pub max_event_records: usize,
    pub log_source: Option<PathBuf>,
    pub release_payload_root: Option<PathBuf>,
}
```

Environment names are `GATEWAY_STATE_DIR`, `GATEWAY_ROUTES_FILE`, `GATEWAY_CONSOLE_REDIS_NAMESPACE`, `GATEWAY_CONSOLE_REMOTE_ACCESS`, `GATEWAY_CONSOLE_TRUSTED_ORIGINS`, `GATEWAY_CONSOLE_TAURI_ORIGINS`, `GATEWAY_CONSOLE_SECRET_GRANT_TTL_SECS`, `GATEWAY_CONSOLE_MAX_EVENTS`, `GATEWAY_CONSOLE_LOG_FILE`, and `GATEWAY_RELEASE_PAYLOAD_ROOT`.

- [ ] **Step 4: Add one reusable test-state builder**

Create a `tests/support/mod.rs` helper that returns `Arc<AppState>` and accepts `Config`, `RouteConfigStore`, and optional `RouteConfigRuntime`. Replace duplicated new-field initialization in touched tests without changing test behavior.

- [ ] **Step 5: Run focused and compile-oriented validation**

Run: `cargo test --locked --test console_config_contract -- --nocapture`  
Expected: all console configuration tests pass.

Run: `cargo check --locked --all-targets`  
Expected: success with every `Config` and `AppState` literal updated.

- [ ] **Step 6: Commit the configuration boundary**

```powershell
git add Cargo.toml Cargo.lock src/config.rs src/state.rs src/runtime.rs src/console/mod.rs tests/console_config_contract.rs tests/support
git commit -m "feat(console): add runtime configuration boundary"
```

## Task 2: Build the Canonical Route Document, Revision, and Secret Patch Core

**Files:**
- Modify: `src/routing/config.rs`
- Create: `src/console/document.rs`
- Create: `src/console/revision.rs`
- Create: `src/console/secrets.rs`
- Modify: `src/console/mod.rs`
- Create: `tests/console_document_contract.rs`

- [ ] **Step 1: Write failing document and secret tests**

```rust
#[test]
fn masked_secret_is_never_a_replacement_value() {
    let active = fixture_document_with_key("sk-real-value");
    let draft = redacted_document(&active);
    let patches = vec![SecretPatch::keep("/providers/0/api_key")];
    let resolved = resolve_secret_patches(&active, draft.document, &patches).unwrap();
    assert_eq!(resolved.providers[0].api_key.as_deref(), Some("sk-real-value"));
}

#[test]
fn replace_and_clear_are_explicit() {
    let active = fixture_document_with_key("old");
    let replaced = apply_patch(&active, SecretPatch::replace("/providers/0/api_key", "new")).unwrap();
    assert_eq!(replaced.providers[0].api_key.as_deref(), Some("new"));
    let cleared = apply_patch(&replaced, SecretPatch::clear("/providers/0/api_key")).unwrap();
    assert_eq!(cleared.providers[0].api_key, None);
}

#[test]
fn canonical_revision_is_stable_across_map_insertion_order() {
    let left = fixture_with_header_order(&[("B", "2"), ("A", "1")]);
    let right = fixture_with_header_order(&[("A", "1"), ("B", "2")]);
    assert_eq!(Revision::from_document(7, &left), Revision::from_document(7, &right));
}
```

- [ ] **Step 2: Verify the tests fail for missing console document APIs**

Run: `cargo test --locked --test console_document_contract -- --nocapture`  
Expected: compile failure for missing redaction, patch, and revision types.

- [ ] **Step 3: Expose the real compiler through a safe API**

Add these APIs without duplicating `compile_yaml`:

```rust
pub fn validate_route_document(document: RouteConfigYaml) -> Result<ValidatedRouteDocument, RouteConfigDiagnostics>;
pub struct ValidatedRouteDocument {
    pub document: RouteConfigYaml,
    pub compiled: Arc<RouteConfigInner>,
    pub canonical_json: Vec<u8>,
    pub canonical_yaml: Vec<u8>,
}
```

Validation rejects duplicate provider IDs, duplicate credential IDs after generated-ID resolution, empty IDs, non-HTTP(S) base URLs where HTTP is required, unknown `provider_ids`, empty route patterns, and alias cycles. Existing preset compilation remains the authoritative provider compiler.

Do not replace tolerant legacy startup parsing with this strict validator in
this task. The current development document contains a historical dangling
`qwen` route reference that the old runtime skips. Startup may load that
compiler-compatible snapshot with `requiresRepair` diagnostics, while every new
console commit must pass strict validation.

- [ ] **Step 4: Implement deterministic serialization and revisions**

Recursively sort JSON object keys before SHA-256 calculation and generate LF-only UTF-8 YAML without BOM. Revision IDs use `r<sequence>-<first-12-hex>` and metadata includes parent, actor, timestamp, document digest, YAML digest, and change message.

- [ ] **Step 5: Implement redaction and patch resolution**

Return non-secret document fields plus descriptors shaped as:

```rust
pub struct SecretDescriptor {
    pub path: String,
    pub configured: bool,
    pub fingerprint: Option<String>,
    pub preview: Option<String>,
}
```

Recognize API key, auth token, refresh token, cookie, authorization, proxy authorization, client secret, password, and recursively sensitive header/body keys. Reject mask sentinels and invalid JSON Pointer paths.

Top-level provider `api_key` is a `String`, so `clear` produces an empty string;
nested credential `api_key` is optional, so `clear` produces `None`. Resolve a
`keep` operation by provider ID and explicit credential ID rather than raw array
index. Reject identity changes and reordered anonymous credentials instead of
copying a secret from the wrong active entry.

- [ ] **Step 6: Run focused and routing regression tests**

Run: `cargo test --locked --test console_document_contract -- --nocapture`  
Expected: pass.

Run: `cargo test --locked routing::config --lib`  
Expected: existing route compiler tests remain green.

- [ ] **Step 7: Commit the pure document core**

```powershell
git add src/routing/config.rs src/console/document.rs src/console/revision.rs src/console/secrets.rs src/console/mod.rs tests/console_document_contract.rs
git commit -m "feat(console): add canonical route document model"
```

## Task 3: Make RouteConfigStore Atomically Replaceable

**Files:**
- Modify: `src/routing/config.rs`
- Modify: `src/state.rs`
- Create: `tests/console_hot_reload_contract.rs`
- Modify: request-path tests that construct `RouteConfigStore`

- [ ] **Step 1: Write a failing old-or-new snapshot concurrency test**

```rust
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_readers_only_observe_complete_revisions() {
    let store = Arc::new(RouteConfigStore::from_document(old_document()).unwrap());
    let readers = spawn_route_readers(store.clone(), 2_000);
    store.replace_document(new_document()).unwrap();
    let observations = join_readers(readers).await;
    assert!(observations.iter().all(|value| value == "old-provider" || value == "new-provider"));
    assert!(observations.iter().all(|value| value != "mixed"));
}
```

- [ ] **Step 2: Confirm the test fails because replacement is unavailable**

Run: `cargo test --locked --test console_hot_reload_contract -- --nocapture`  
Expected: compile failure for `from_document` and `replace_document`.

- [ ] **Step 3: Replace the mutable inner lock with one immutable snapshot pointer**

Use `arc_swap::ArcSwap<RouteConfigSnapshot>`. Every public query method loads one `Arc<RouteConfigSnapshot>` and operates on that object. Add:

```rust
pub fn snapshot(&self) -> Arc<RouteConfigSnapshot>;
pub fn replace_validated(&self, validated: ValidatedRouteDocument, revision: RevisionMetadata) -> Arc<RouteConfigSnapshot>;
```

Do not add incremental provider/route mutation methods.

- [ ] **Step 4: Run focused, pipeline, model, and token-refresh tests**

Run: `cargo test --locked --test console_hot_reload_contract -- --nocapture`  
Expected: pass.

Run: `cargo test --locked stage_route --lib`  
Expected: pass.

Run: `cargo test --locked models --lib`  
Expected: pass.

- [ ] **Step 5: Commit atomic hot replacement**

```powershell
git add src/routing/config.rs src/state.rs tests/console_hot_reload_contract.rs
git commit -m "feat(console): support atomic route config replacement"
```

## Task 4: Implement Atomic YAML Persistence and Transaction Journal

**Files:**
- Create: `src/console/persistence.rs`
- Create: `src/console/journal.rs`
- Modify: `src/console/mod.rs`
- Create: `tests/console_persistence_contract.rs`

- [ ] **Step 1: Write failing persistence and crash-point tests**

Test all of these behaviors independently:

```rust
#[test]
fn first_console_save_preserves_original_bytes_exactly() {
    let temp = tempfile::tempdir().unwrap();
    let routes = temp.path().join("routes.yaml");
    let original = b"# original comment\r\nproviders: []\r\n";
    std::fs::write(&routes, original).unwrap();
    let persistence = RouteConfigPersistence::for_test(temp.path(), &routes).unwrap();
    persistence.ensure_first_save_backup().unwrap();
    assert_eq!(std::fs::read(persistence.first_save_backup_path()).unwrap(), original);
}

#[test]
fn atomic_replace_never_exposes_a_partial_yaml_document() {
    let fixture = AtomicReplaceFixture::new(b"providers: []\n");
    fixture.replace_repeatedly(b"providers:\n  - id: new\n", 250);
    assert!(fixture.observed_files().iter().all(|bytes| {
        bytes == b"providers: []\n" || bytes == b"providers:\n  - id: new\n"
    }));
}

#[test]
fn journal_rejects_revision_paths_outside_state_root() {
    let record = TransactionRecord::prepared("tx-1", "../outside", "r1-a", "r2-b");
    let error = TransactionJournal::validate_record(&record).unwrap_err();
    assert_eq!(error.code(), "console_journal_path_escape");
}

#[test]
fn prepared_transaction_without_activation_recovers_old_yaml() {
    let mut fixture = RecoveryFixture::prepared_after_yaml_replace();
    fixture.recover().unwrap();
    assert_eq!(fixture.yaml_bytes(), fixture.old_revision_yaml());
    assert_eq!(fixture.transaction_phase(), TransactionPhase::Aborted);
}
```

- [ ] **Step 2: Verify the tests fail for missing persistence modules**

Run: `cargo test --locked --test console_persistence_contract -- --nocapture`  
Expected: compile failure for `RouteConfigPersistence` and `TransactionJournal`.

- [ ] **Step 3: Implement the state layout and immutable revision archive**

Create `console/admin.json`, `revisions/<revision>/`, `transactions/`, `journal.ndjson`, `events.ndjson`, and `backups/routes-first-save.yaml`. Use `create_new` for immutable files and reject symlinks/reparse points or canonical paths escaping the state root.

- [ ] **Step 4: Implement platform atomic replacement**

Write a same-directory temporary file, `flush`, `sync_all`, then:

- on Windows, call `ReplaceFileW`; if the destination does not exist, call `MoveFileExW` with `MOVEFILE_WRITE_THROUGH`;
- on POSIX, call `rename` and sync the parent directory.

Never delete the destination before replacement. Ensure UTF-8 without BOM and LF-only canonical YAML.

- [ ] **Step 5: Implement durable transaction phases**

Use `Prepared`, `YamlReplaced`, `RedisActivated`, `Committed`, and `Aborted` phases. Every transition writes the per-transaction JSON atomically and appends one redacted NDJSON journal record. The loader tolerates a truncated final NDJSON line but rejects malformed earlier records.

- [ ] **Step 6: Run failure-injection tests**

Run: `cargo test --locked --test console_persistence_contract -- --nocapture`  
Expected: pass, including denied-directory and interrupted-write fixtures.

- [ ] **Step 7: Commit local durability**

```powershell
git add src/console/persistence.rs src/console/journal.rs src/console/mod.rs tests/console_persistence_contract.rs Cargo.toml Cargo.lock
git commit -m "feat(console): add durable config persistence journal"
```

## Task 5: Add Redis Revision Storage, CAS Activation, and Recovery

**Files:**
- Modify: `src/redis/keys.rs`
- Create: `src/console/redis_store.rs`
- Create: `src/console/runtime.rs`
- Modify: `src/console/mod.rs`
- Modify: `src/runtime.rs`
- Modify: `src/state.rs`
- Create: `tests/console_transaction_contract.rs`
- Create: `tests/python/test_gateway_console_redis_e2e.py`

- [ ] **Step 1: Write failing Redis CAS and recovery tests**

Cover initialized and uninitialized active pointers, stale expected revision, transport ambiguity, CAS success, compatibility mirror update, publish-after-CAS, crash after YAML replacement, and crash after Redis activation.

```rust
#[tokio::test]
async fn stale_revision_cannot_activate_candidate() {
    let error = store.activate("r17-old", prepared_r19()).await.unwrap_err();
    assert_eq!(error.code(), "revision_conflict");
    assert_eq!(store.active_revision().await.unwrap(), "r18-current");
}
```

- [ ] **Step 2: Confirm the focused tests fail**

Run: `cargo test --locked --test console_transaction_contract -- --nocapture`  
Expected: compile failure for Redis revision APIs.

- [ ] **Step 3: Add namespaced Redis key builders and Lua CAS**

Use this key family:

```text
gw:console:route-config:<namespace>:active_revision
gw:console:route-config:<namespace>:active_document
gw:console:route-config:<namespace>:revisions:<revision>
gw:console:route-config:<namespace>:transactions:<transaction>
gw:console:route-config:<namespace>:events
```

The Lua script verifies the expected active revision and prepared transaction, writes the active revision/document, updates legacy `gw:config:routes`, marks the transaction activated, and publishes the new revision atomically.

- [ ] **Step 4: Implement `RouteConfigRuntime::commit`**

Follow the approved order exactly: validate, lock/recheck, archive, prepare journal/Redis, replace YAML, Redis CAS, replace in-memory snapshot, commit journal/event. On an ambiguous Redis error, read the active pointer before deciding whether to restore YAML or finish the commit.

Expose two internal roles behind this runtime boundary:
`RouteConfigCoordinator` for standalone/splitter writes and
`RouteConfigReplica` for worker fetch, subscription, and reconciliation.
Standalone constructs both. The splitter constructs only the coordinator on
its public port; a worker never receives write capability.

- [ ] **Step 5: Implement idempotent startup recovery**

Choose the last committed Redis revision when its immutable archive validates; otherwise restore the last local committed revision. A prepared transaction without successful CAS restores old YAML. An activated transaction completes YAML and memory. Irreparable storage enters read-only degraded mode while preserving the last-good route snapshot.

- [ ] **Step 6: Run isolated Redis tests**

Run: `python -m unittest tests.python.test_gateway_console_redis_e2e -v`  
Expected: isolated Redis container starts on a unique port, all CAS/recovery cases pass, and the container is removed.

Run: `cargo test --locked --test console_transaction_contract -- --nocapture`  
Expected: pass.

- [ ] **Step 7: Commit transaction activation**

```powershell
git add src/redis/keys.rs src/console/redis_store.rs src/console/runtime.rs src/console/mod.rs src/runtime.rs src/state.rs tests/console_transaction_contract.rs tests/python/test_gateway_console_redis_e2e.py
git commit -m "feat(console): activate route revisions transactionally"
```

## Task 6: Implement Bootstrap, Session, Access Policy, Events, and Logs

**Files:**
- Modify: `src/access_control.rs`
- Modify: `src/error.rs`
- Create: `src/console/auth.rs`
- Create: `src/console/access.rs`
- Create: `src/console/events.rs`
- Create: `src/console/logs.rs`
- Modify: `src/console/mod.rs`
- Modify: `src/runtime.rs`
- Create: `tests/console_security_contract.rs`

- [ ] **Step 1: Write failing security contracts**

```rust
#[tokio::test]
async fn remote_client_cannot_bootstrap_first_admin() {
    let response = bootstrap_request(test_app(), "192.0.2.10:41000", "new-admin-token").await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(error_code(response).await, "console_bootstrap_loopback_only");
}

#[tokio::test]
async fn bootstrap_hash_never_contains_plaintext_token() {
    let fixture = AuthFixture::new();
    fixture.bootstrap("a-long-development-token").await.unwrap();
    let bytes = std::fs::read(fixture.admin_path()).unwrap();
    assert!(!String::from_utf8_lossy(&bytes).contains("a-long-development-token"));
    assert!(String::from_utf8_lossy(&bytes).contains("$argon2id$"));
}

#[test]
fn stored_and_environment_tokens_are_verified_without_plaintext_persistence() {
    let verifier = ManagementTokenVerifier::from_environment("management-secret");
    assert!(verifier.verify("management-secret").unwrap());
    assert!(!verifier.verify("management-wrong").unwrap());
    assert_eq!(verifier.persisted_plaintext(), None);
}

#[tokio::test]
async fn secret_grant_expires_and_is_bound_to_origin() {
    let clock = TestClock::at_unix_seconds(1_000);
    let grants = SecretGrantStore::with_clock(clock.clone(), Duration::from_secs(300));
    let grant = grants.issue("admin-fingerprint", "http://localhost:4200", "127.0.0.1");
    assert!(grants.verify(&grant, "admin-fingerprint", "http://localhost:4200", "127.0.0.1"));
    clock.advance(Duration::from_secs(301));
    assert!(!grants.verify(&grant, "admin-fingerprint", "http://localhost:4200", "127.0.0.1"));
}

#[test]
fn log_tail_redacts_unicode_secrets_without_panicking() {
    let line = "Authorization: Bearer 密钥-secret";
    let redacted = redact_console_log_line(line);
    assert!(!redacted.contains("密钥-secret"));
    assert!(redacted.contains("[REDACTED]"));
}
```

- [ ] **Step 2: Verify the tests fail**

Run: `cargo test --locked --test console_security_contract -- --nocapture`  
Expected: compile failure for bootstrap/access/event APIs.

- [ ] **Step 3: Implement admin bootstrap and rotation**

Store Argon2id PHC strings in `admin.json` through the atomic writer. Bootstrap is single-use and loopback-only. `GATEWAY_MANAGEMENT_TOKEN` remains a non-persisted override and cannot be rotated in the UI. Use `subtle::ConstantTimeEq` where a raw environment token must be compared.

- [ ] **Step 4: Implement remote/origin policy**

Use Axum `ConnectInfo<SocketAddr>` in standalone. Ignore forwarded headers unless a trusted splitter injects `x-gateway-client-ip` after stripping any client-provided copy. Default console access is loopback-only. Remote access requires the explicit flag and exact trusted origin. Add explicit Tauri origins without using `Any`.

- [ ] **Step 5: Implement bounded events and logs**

Keep an in-memory ring plus bounded NDJSON persistence. Redact before enqueue. Limit log tail by line count and bytes, canonicalize the configured log source, reject traversal/reparse points, and stream through fetch-SSE-compatible responses.

- [ ] **Step 6: Run security and access regressions**

Run: `cargo test --locked --test console_security_contract -- --nocapture`  
Expected: pass.

Run: `cargo test --locked --test phase3_access_reliability_contract -- --nocapture`  
Expected: existing internal management/browser-executor behavior remains green.

- [ ] **Step 7: Commit the management security layer**

```powershell
git add src/access_control.rs src/error.rs src/console/auth.rs src/console/access.rs src/console/events.rs src/console/logs.rs src/console/mod.rs src/runtime.rs tests/console_security_contract.rs
git commit -m "feat(console): secure bootstrap and management sessions"
```

## Task 7: Add the Console Management API

**Files:**
- Create: `src/http/routes/internal_console/mod.rs`
- Create: `src/http/routes/internal_console/auth.rs`
- Create: `src/http/routes/internal_console/config.rs`
- Create: `src/http/routes/internal_console/revisions.rs`
- Create: `src/http/routes/internal_console/runtime.rs`
- Create: `src/http/routes/internal_console/events.rs`
- Modify: `src/http/routes/mod.rs`
- Modify: `src/http/router.rs`
- Create: `tests/console_api_contract.rs`

- [ ] **Step 1: Write failing router contracts for every endpoint group**

Use `tower::ServiceExt::oneshot` to assert bootstrap status, login errors, masked config GET, validation, update conflict, revision list/detail/diff/rollback, runtime summary, preset list, events, and no-store headers. Include one test that submits `***` and asserts `422 console_mask_sentinel_rejected`.

- [ ] **Step 2: Confirm the routes are missing**

Run: `cargo test --locked --test console_api_contract -- --nocapture`  
Expected: `404` assertions fail because the console router has not been mounted.

- [ ] **Step 3: Build one protected console subrouter**

Mount `/v1/internal/gateway/console/*` through a dedicated access middleware. Do not rely on each handler to remember `assert_management_access`. Bootstrap status/bootstrap have their own loopback-only middleware; all other routes require management auth.

- [ ] **Step 4: Implement typed handlers and response headers**

All JSON handlers return the existing Gateway error envelope and `Cache-Control: no-store`. Add `ETag` to config/revision responses. Require `expectedRevision`; if `If-Match` is present it must match. Map validation to `422`, lock/recovery to `423`, dependency failures to `424`, and stale revisions to `409` with current revision and diff.

- [ ] **Step 5: Keep Playground on the canonical public path**

The management playground handler constructs an internal HTTP request to the bound Gateway public endpoint or calls the same public handler boundary with normal auth headers; it must not invoke the upstream client or route store directly.

- [ ] **Step 6: Run API and smoke tests**

Run: `cargo test --locked --test console_api_contract -- --nocapture`  
Expected: pass.

Run: `cargo test --locked --test smoke -- --nocapture`  
Expected: existing public endpoints remain green.

- [ ] **Step 7: Commit the management API**

```powershell
git add src/http/routes/internal_console src/http/routes/mod.rs src/http/router.rs tests/console_api_contract.rs
git commit -m "feat(console): expose revisioned management API"
```

## Task 8: Embed and Serve the Browser Console at `/ui/`

**Files:**
- Modify: `Cargo.toml`
- Create: `build.rs`
- Create: `src/http/ui.rs`
- Modify: `src/http/mod.rs`
- Modify: `src/http/router.rs`
- Modify: `.gitignore`
- Create: `apps/desktop/dist/web/.gitkeep`
- Create: `tests/console_ui_assets_contract.rs`
- Create: `tests/python/test_gateway_console_asset_contract.py`

- [ ] **Step 1: Write failing asset routing tests**

Assert `/ui` redirects to `/ui/`, `/ui/` returns embedded HTML, a known fingerprinted asset has immutable caching, a client route receives HTML, a missing `/ui/static/*` returns `404`, and `/routes.yaml`, `/.env`, `/ui/../routes.yaml`, state files, and release files cannot be downloaded.

- [ ] **Step 2: Confirm `/ui/` currently fails**

Run: `cargo test --locked --test console_ui_assets_contract -- --nocapture`  
Expected: failures because `/ui/` returns `404`.

- [ ] **Step 3: Implement the embedded asset handler**

Use `#[derive(RustEmbed)] #[folder = "apps/desktop/dist/web/"]`. Serve only exact asset-map paths. Add MIME detection, `nosniff`, CSP, `DENY`, `no-referrer`, restricted permissions policy, HTML `no-store`, and immutable caching for hashed assets.

- [ ] **Step 4: Add build invalidation and source-leak contracts**

`build.rs` emits `cargo:rerun-if-changed=apps/desktop/dist/web`. The Python contract parses the asset manifest and rejects source maps, `.env`, YAML route files, state files, absolute source paths, and unreferenced bundle files.

- [ ] **Step 5: Run asset and public-router regressions**

Run: `cargo test --locked --test console_ui_assets_contract -- --nocapture`  
Expected: pass.

Run: `python -m unittest tests.python.test_gateway_console_asset_contract -v`  
Expected: pass.

- [ ] **Step 6: Commit embedded hosting**

```powershell
git add Cargo.toml Cargo.lock build.rs src/http/ui.rs src/http/mod.rs src/http/router.rs .gitignore apps/desktop/dist/web/.gitkeep tests/console_ui_assets_contract.rs tests/python/test_gateway_console_asset_contract.py
git commit -m "feat(console): embed browser assets at ui path"
```

## Task 9: Establish the Shared React Toolchain, Host Adapters, API Client, and Session

**Files:**
- Modify: `apps/desktop/package.json`
- Modify: `apps/desktop/package-lock.json`
- Modify: `apps/desktop/rsbuild.config.ts`
- Modify: `apps/desktop/tsconfig.json`
- Modify: `apps/desktop/src-tauri/tauri.conf.json`
- Create: `apps/desktop/vitest.config.ts`
- Create: `apps/desktop/playwright.config.ts`
- Create: `apps/desktop/src/test/setup.ts`
- Create: `apps/desktop/src/test/server.ts`
- Create: `apps/desktop/src/platform/types.ts`
- Create: `apps/desktop/src/platform/browserHost.ts`
- Create: `apps/desktop/src/platform/tauriHost.ts`
- Create: `apps/desktop/src/platform/createHostAdapter.ts`
- Create: `apps/desktop/src/platform/HostProvider.tsx`
- Create: `apps/desktop/src/api/contracts.ts`
- Create: `apps/desktop/src/api/schemas.ts`
- Create: `apps/desktop/src/api/errors.ts`
- Create: `apps/desktop/src/api/client.ts`
- Create: `apps/desktop/src/api/console.ts`
- Create: `apps/desktop/src/api/public.ts`
- Create: `apps/desktop/src/api/events.ts`
- Create: `apps/desktop/src/session/storage.ts`
- Create: `apps/desktop/src/session/ManagementSessionProvider.tsx`
- Create: `apps/desktop/src/session/useManagementSession.ts`
- Create: `apps/desktop/src/features/auth/BootstrapPage.tsx`
- Create: `apps/desktop/src/features/auth/LoginPage.tsx`
- Create: `apps/desktop/src/features/auth/SecretConfirmDialog.tsx`
- Create: `apps/desktop/src/features/auth/AuthBoundary.tsx`
- Modify: `apps/desktop/src/lib/http.ts`
- Modify: `apps/desktop/src/lib/tauri.ts`

- [ ] **Step 1: Add failing Vitest contracts**

Tests must assert Browser uses `window.location.origin`, Tauri uses the selected loopback sidecar URL, management tokens are headers and never URLs, Zod rejects malformed responses, `401/403` clears the session, bootstrap is attempted before login, secret grants are memory-only, and fetch-SSE sends the management header.

- [ ] **Step 2: Install the approved dependencies and verify the tests fail**

Run: `npm install --prefix apps/desktop react-router-dom @tanstack/react-query zod react-hook-form @hookform/resolvers lucide-react @microsoft/fetch-event-source @radix-ui/react-dialog @radix-ui/react-alert-dialog @radix-ui/react-select @radix-ui/react-tabs @radix-ui/react-tooltip @radix-ui/react-switch`

Run: `npm install --prefix apps/desktop --save-dev vitest jsdom @testing-library/react @testing-library/user-event @testing-library/jest-dom msw @playwright/test cross-env`

Run: `npm test --prefix apps/desktop -- --run`  
Expected: failing tests because adapters and session APIs are not implemented.

- [ ] **Step 3: Implement deterministic dual builds**

Use `GATEWAY_UI_TARGET=web|tauri`. Web outputs `dist/web`, asset prefix `/ui/`, BrowserRouter base `/ui`; Tauri outputs `dist/tauri`, relative assets, HashRouter, and `frontendDist: ../dist/tauri`. Add scripts `test`, `test:watch`, `build:web`, `build:tauri`, and `e2e`.

- [ ] **Step 4: Implement host adapters and typed clients**

`GatewayHostAdapter` exposes `kind`, `apiOrigin`, and capabilities. Shared components never import Tauri. Parse every response with Zod. Store the management token in versioned `sessionStorage`; keep the secret grant in memory only. Use fetch-SSE rather than native `EventSource`.

- [ ] **Step 5: Implement bootstrap/login/secret-confirm boundaries**

Add `AuthBoundary`, `BootstrapPage`, `LoginPage`, and `SecretConfirmDialog`. Clear credentials on auth failure and explicit logout. Rotate updates storage only after server success.

- [ ] **Step 6: Run frontend core checks**

Run: `npm test --prefix apps/desktop -- --run`  
Expected: pass.

Run: `npm run typecheck --prefix apps/desktop`  
Expected: pass without `any` additions.

- [ ] **Step 7: Commit the shared frontend runtime**

```powershell
git add apps/desktop/package.json apps/desktop/package-lock.json apps/desktop/rsbuild.config.ts apps/desktop/tsconfig.json apps/desktop/src-tauri/tauri.conf.json apps/desktop/vitest.config.ts apps/desktop/playwright.config.ts apps/desktop/src/test apps/desktop/src/platform apps/desktop/src/api apps/desktop/src/session apps/desktop/src/features/auth apps/desktop/src/lib/http.ts apps/desktop/src/lib/tauri.ts
git commit -m "feat(console): add browser and Tauri frontend adapters"
```

## Task 10: Build the Console Shell and Shared Route Configuration Draft

**Files:**
- Modify: `apps/desktop/src/main.tsx`
- Modify: `apps/desktop/src/App.tsx`
- Create: `apps/desktop/src/app/router.tsx`
- Create: `apps/desktop/src/app/routes.tsx`
- Create: `apps/desktop/src/app/AppProviders.tsx`
- Create: `apps/desktop/src/app/ConsoleShell.tsx`
- Create: `apps/desktop/src/app/Nav.tsx`
- Create: `apps/desktop/src/app/PageHeader.tsx`
- Create: `apps/desktop/src/components/DataTable.tsx`
- Create: `apps/desktop/src/components/StatusBadge.tsx`
- Create: `apps/desktop/src/components/EmptyState.tsx`
- Create: `apps/desktop/src/components/InlineError.tsx`
- Create: `apps/desktop/src/components/Dialog.tsx`
- Create: `apps/desktop/src/components/ConfirmAction.tsx`
- Create: `apps/desktop/src/components/SecretField.tsx`
- Create: `apps/desktop/src/components/RevisionSaveBar.tsx`
- Create: `apps/desktop/src/components/ConflictDialog.tsx`
- Create: `apps/desktop/src/components/Tooltip.tsx`
- Create: `apps/desktop/src/config/secretPatch.ts`
- Create: `apps/desktop/src/config/routeConfigTypes.ts`
- Create: `apps/desktop/src/config/RouteConfigDraftProvider.tsx`
- Create: `apps/desktop/src/config/useRouteConfigDraft.ts`
- Create: `apps/desktop/src/styles/tokens.css`
- Create: `apps/desktop/src/styles/base.css`
- Create: `apps/desktop/src/styles/shell.css`
- Create: `apps/desktop/src/styles/components.css`
- Create: `apps/desktop/src/styles/responsive.css`
- Modify: `apps/desktop/src/styles.css`
- Create: `apps/desktop/src/app/ConsoleShell.test.tsx`
- Create: `apps/desktop/src/config/RouteConfigDraftProvider.test.tsx`

- [ ] **Step 1: Write failing shell and draft tests**

Assert all approved navigation entries exist, browser deep links render the correct page, mobile navigation is operable, dirty draft state is shared across Providers/Credentials/Routes/Aliases, validation runs before save, `409` retains the draft and opens a conflict dialog, and every existing secret defaults to `keep`.

- [ ] **Step 2: Run the focused tests and confirm failure**

Run: `npm test --prefix apps/desktop -- --run src/app/ConsoleShell.test.tsx src/config/RouteConfigDraftProvider.test.tsx`  
Expected: failure because shell/draft modules do not exist.

- [ ] **Step 3: Implement the route tree and shell**

Use these shared routes: `/overview`, `/providers`, `/credentials`, `/routes`, `/revisions`, `/models`, `/playground`, `/events`, `/runtime`, `/settings`. Add fixed navigation, compact page headers, status bar, loading/error/empty states, keyboard focus, Lucide icon buttons, and tooltips.

- [ ] **Step 4: Implement the single document draft**

Providers, credentials, routes, and aliases edit one `RouteConfigDraft`. Save sends one `expectedRevision`, full non-secret document, and secret patches. Do not create granular backend write calls. Add global `RevisionSaveBar` and `ConflictDialog`.

- [ ] **Step 5: Apply the approved dense visual baseline**

Use graphite backgrounds with explicit foreground tokens on every surface, green/cyan/yellow/red semantic accents, WCAG AA text contrast, card radius at most 8px, stable table/grid dimensions, no marketing hero, no decorative orbs, no nested cards, and no viewport-scaled type. Below 900px the rail becomes a drawer; at 390px tables become labeled compact rows without horizontal page scroll or text overlap.

- [ ] **Step 6: Run component, type, and build checks**

Run: `npm test --prefix apps/desktop -- --run src/app/ConsoleShell.test.tsx src/config/RouteConfigDraftProvider.test.tsx`  
Expected: pass.

Run: `npm run typecheck --prefix apps/desktop`  
Expected: pass.

- [ ] **Step 7: Commit shell and draft state**

```powershell
git add apps/desktop/src/main.tsx apps/desktop/src/App.tsx apps/desktop/src/app apps/desktop/src/components apps/desktop/src/config apps/desktop/src/styles apps/desktop/src/styles.css
git commit -m "feat(console): add operational shell and config draft"
```

## Task 11: Implement Providers, Credentials, Routes, and Revisions

**Files:**
- Create: `apps/desktop/src/features/providers/ProvidersPage.tsx`
- Create: `apps/desktop/src/features/providers/ProviderDialog.tsx`
- Create: `apps/desktop/src/features/credentials/CredentialsPage.tsx`
- Create: `apps/desktop/src/features/credentials/CredentialDialog.tsx`
- Create: `apps/desktop/src/features/routes/RoutesPage.tsx`
- Create: `apps/desktop/src/features/routes/AliasesPanel.tsx`
- Create: `apps/desktop/src/features/revisions/RevisionsPage.tsx`
- Create: `apps/desktop/src/features/revisions/DiffViewer.tsx`
- Create: `apps/desktop/src/features/revisions/ImportExportDialog.tsx`
- Create: corresponding `*.test.tsx` files

- [ ] **Step 1: Write failing feature tests**

Cover provider add/edit/disable, preset selection, credential add/edit/reveal/copy/clear, route priority/provider ordering, alias cycle diagnostics, revision list/detail/diff, import validation, export confirmation, rollback confirmation, loading/empty/error states, and stale revision conflict preservation.

- [ ] **Step 2: Verify feature tests fail**

Run: `npm test --prefix apps/desktop -- --run src/features/providers src/features/credentials src/features/routes src/features/revisions`  
Expected: missing feature modules and assertions fail.

- [ ] **Step 3: Implement Providers and Credentials**

Use compact tables and Radix dialogs. Never populate a password input with a preview. Reveal/copy asks for a secret grant; edits generate `keep`, `replace`, or `clear` patches only.

- [ ] **Step 4: Implement Routes, Aliases, and Revisions**

Route rows support pattern, priority, ordered provider IDs, and disabled draft state. Alias editing shows cycle/target errors from server validation. Revision rollback creates a new revision and refreshes the shared draft only after success.

- [ ] **Step 5: Run feature and accessibility tests**

Run: `npm test --prefix apps/desktop -- --run src/features/providers src/features/credentials src/features/routes src/features/revisions`  
Expected: pass with no unhandled React warnings.

- [ ] **Step 6: Commit primary management workflows**

```powershell
git add apps/desktop/src/features/providers apps/desktop/src/features/credentials apps/desktop/src/features/routes apps/desktop/src/features/revisions
git commit -m "feat(console): manage providers routes and revisions"
```

## Task 12: Implement Overview, Models, Playground, Events, Runtime, and Settings

**Files:**
- Create: `apps/desktop/src/features/overview/OverviewPage.tsx`
- Create: `apps/desktop/src/features/overview/queries.ts`
- Refactor: `apps/desktop/src/features/models/ModelsPanel.tsx` into `apps/desktop/src/features/models/ModelsPage.tsx`
- Refactor: `apps/desktop/src/features/api-test/ApiTestPanel.tsx` into `apps/desktop/src/features/playground/PlaygroundPage.tsx`
- Create: `apps/desktop/src/features/events/EventsPage.tsx`
- Create: `apps/desktop/src/features/events/EventStream.tsx`
- Refactor: `apps/desktop/src/features/status/StatusPanel.tsx` into `apps/desktop/src/features/runtime/RuntimePage.tsx`
- Create: `apps/desktop/src/features/runtime/DrainDialog.tsx`
- Refactor: Tauri-only launcher/config/log components behind platform capabilities
- Create: `apps/desktop/src/features/settings/SettingsPage.tsx`
- Create: `apps/desktop/src/features/settings/SessionRotate.tsx`
- Create: `apps/desktop/src/features/settings/StatePath.tsx`
- Create: corresponding `*.test.tsx` files

- [ ] **Step 1: Write failing secondary feature tests**

Cover overview readiness/revision/source, models table, playground request with page-memory API key, fetch-SSE reconnect without token leakage, runtime drain confirmation, standalone restart disclaimer, Tauri-only sidecar controls, session rotation, and read-only state paths.

- [ ] **Step 2: Confirm the tests fail**

Run: `npm test --prefix apps/desktop -- --run src/features/overview src/features/models src/features/playground src/features/events src/features/runtime src/features/settings`  
Expected: missing or old feature behavior fails.

- [ ] **Step 3: Implement the feature pages**

Reuse existing Models and API Test logic through typed clients. Keep Playground API key only in component state. Events/log streams abort on navigation/logout. Runtime drain is explicit and never labels standalone shutdown as restart.

- [ ] **Step 4: Preserve Tauri capabilities through the adapter**

Move profile, sidecar, local path, and desktop log access behind `TauriHostAdapter`. Browser builds do not import Tauri modules. Delete the monolithic `useGatewayDesktopState` only after all consumers have moved.

- [ ] **Step 5: Run all frontend tests and builds**

Run: `npm test --prefix apps/desktop -- --run`  
Expected: pass.

Run: `npm run typecheck --prefix apps/desktop`  
Expected: pass.

Run: `npm run build:web --prefix apps/desktop`  
Expected: `apps/desktop/dist/web/index.html` references only `/ui/` assets.

Run: `npm run build:tauri --prefix apps/desktop`  
Expected: `apps/desktop/dist/tauri/index.html` uses relative assets.

- [ ] **Step 6: Commit the complete console feature set**

```powershell
git add apps/desktop/src apps/desktop/dist/web apps/desktop/package-lock.json
git commit -m "feat(console): complete operator console workflows"
```

## Task 13: Support Splitter Ownership and Worker Convergence

**Files:**
- Modify: `src/splitter.rs`
- Modify: `src/runtime.rs`
- Modify: `src/console/runtime.rs`
- Modify: `src/console/access.rs`
- Create: `tests/console_splitter_contract.rs`
- Modify: `tests/python/test_gateway_splitter_worker_e2e.py`

- [ ] **Step 1: Write failing splitter/worker contracts**

Assert the splitter serves `/ui/`, owns mutations, strips spoofed forwarding headers, injects trusted client IP, rejects remote bootstrap, publishes a revision, waits until the worker reports that revision, and leaves a failed worker on its previous complete revision.

- [ ] **Step 2: Verify the tests fail against the current proxy-only splitter**

Run: `cargo test --locked --test console_splitter_contract -- --nocapture`  
Expected: `/ui/` or management ownership assertions fail.

- [ ] **Step 3: Mount manager console routes and trusted forwarding**

The splitter directly serves embedded assets and mutation APIs. It strips client `x-gateway-client-ip` and all untrusted forwarded-IP variants before adding its own trusted value. Workers bind loopback in splitter mode.

- [ ] **Step 4: Add worker revision subscription and readiness reporting**

Workers first fetch the namespaced active pointer before accepting traffic,
then subscribe to Redis events and periodically reconcile the pointer because
Pub/Sub messages are not durable. Each fetched revision is validated/compiled
before an atomic store swap and is exposed in readiness. The splitter only
acknowledges convergence after the active worker reports the committed
revision. The splitter's worker-availability guard explicitly allows `/ui/`
and console API paths while a worker is unavailable so recovery remains
possible.

- [ ] **Step 5: Run focused and isolated splitter E2E**

Run: `cargo test --locked --test console_splitter_contract -- --nocapture`  
Expected: pass.

Run: `$env:GATEWAY_RUN_SPLITTER_E2E='1'; python -m unittest tests.python.test_gateway_splitter_worker_e2e -v`  
Expected: isolated manager/worker revision save and request pass; processes and Redis container are cleaned.

- [ ] **Step 6: Commit clustered convergence**

```powershell
git add src/splitter.rs src/runtime.rs src/console/runtime.rs src/console/access.rs tests/console_splitter_contract.rs tests/python/test_gateway_splitter_worker_e2e.py
git commit -m "feat(console): converge route revisions across workers"
```

## Task 14: Update Build, CI, Docker, Release, and Product Documentation

**Files:**
- Modify: `tools/build-gateway-release.ps1`
- Modify: `tools/package-gateway-release.ps1`
- Modify: `tools/smoke-gateway-ui-release.ps1`
- Modify: `.github/workflows/ci.yml`
- Modify: `.github/workflows/build-windows.yml`
- Modify: `.github/workflows/release-tag.yml`
- Modify: `Dockerfile`
- Modify: `.dockerignore`
- Modify: `apps/desktop/src-tauri/src/profile.rs`
- Modify: `apps/desktop/src-tauri/src/process.rs`
- Modify: `README.md`
- Modify: `docs/operations-manual.md`
- Modify: `docs/progress/MASTER.md`
- Modify: `tests/python/test_gateway_desktop_ui_contract.py`
- Create: `tests/python/test_gateway_console_build_contract.py`

- [ ] **Step 1: Write failing build-order and release-state contracts**

The Python tests require `npm ci -> typecheck/test -> build:web -> cargo check/test/build -> build:tauri/Tauri`, verify Docker has a Node frontend stage, require `/ui/` smoke, require `frontendDist=../dist/tauri`, and reject mutable state paths under the release payload.

- [ ] **Step 2: Confirm the current scripts fail the new contract**

Run: `python -m unittest tests.python.test_gateway_console_build_contract -v`  
Expected: failures because Rust currently builds before the web bundle and Docker has no UI stage.

- [ ] **Step 3: Reorder local, CI, and Docker builds**

Build the versioned web bundle and verify its manifest before every Rust release compile. Add a Docker Node stage and copy only the required frontend build input into the Rust builder. Keep runtime images free of Node and source maps.

- [ ] **Step 4: Move packaged mutable state outside the payload**

Tauri profiles resolve state under the OS application-data directory, copy the packaged seed `routes.yaml` there on first use, set `GATEWAY_STATE_DIR` and `GATEWAY_ROUTES_FILE`, and never edit the packaged seed. Release output remains only below `C:\Users\Public\nas_home\AI\GameEditor\Neuro\release\Gateway`.

- [ ] **Step 5: Update product and operations documentation**

Document `/ui/`, bootstrap/login, remote-access policy, state/revision paths, backup/recovery, rollback, splitter convergence, and the fact that standalone drain requires Tauri or an external supervisor for restart.

- [ ] **Step 6: Run contract and package smoke tests**

Run: `python -m unittest tests.python.test_gateway_console_build_contract tests.python.test_gateway_desktop_ui_contract -v`  
Expected: pass.

Run: `powershell -NoProfile -ExecutionPolicy Bypass -File tools/build-gateway-release.ps1 -ReleaseRoot "C:\Users\Public\nas_home\AI\GameEditor\Neuro\release\Gateway" -DryRun`  
Expected: ordered stages include web build before Rust and no output outside the approved release root.

- [ ] **Step 7: Commit delivery integration**

```powershell
git add tools .github Dockerfile .dockerignore apps/desktop/src-tauri README.md docs tests/python/test_gateway_desktop_ui_contract.py tests/python/test_gateway_console_build_contract.py
git commit -m "build(console): integrate embedded UI into releases"
```

## Task 15: Run Browser E2E, Failure Recovery, and Final Verification

**Files:**
- Create: `apps/desktop/e2e/console.spec.ts`
- Create: `tools/run-gateway-console-e2e.ps1`
- Create: `tests/python/test_gateway_console_e2e_contract.py`
- Modify: `docs/progress/MASTER.md`
- Output screenshots/logs only below ignored `.runtime/console-e2e/`
- Output release package only below `C:\Users\Public\nas_home\AI\GameEditor\Neuro\release\Gateway`

- [ ] **Step 1: Write the failing Playwright workflow**

The test starts an isolated Redis and standalone Gateway, opens `/ui/`, bootstraps an admin token, logs in, adds a disposable OpenAI-compatible provider and route, validates/saves, verifies `/v1/models`, sends one chat request through the same Gateway using a local deterministic stub upstream, views the revision diff, rolls back, and confirms the route disappears.

- [ ] **Step 2: Add failure-recovery scenarios**

Cover stale two-tab save, invalid provider reference, denied YAML write, Redis unavailable before CAS, process termination after CAS, startup recovery, wrong management token, expired secret grant, remote bootstrap rejection, missing static asset, and reload while a transaction is active.

- [ ] **Step 3: Add visual and layout assertions**

Run desktop `1440x900` and mobile `390x844` viewports. Capture Overview, Providers, Credential dialog, Routes, Revisions diff, Playground, Events, and Runtime. Assert the root screenshot has non-background pixels, no element has negative/overflowing bounds, no text overlaps the next control, and navigation remains reachable.

- [ ] **Step 4: Run the complete focused E2E**

Run: `powershell -NoProfile -ExecutionPolicy Bypass -File tools/run-gateway-console-e2e.ps1`  
Expected: all browser, transaction, recovery, and cleanup checks pass; no Gateway, browser, or Redis process/container remains.

- [ ] **Step 5: Run the full repository gate**

```powershell
cargo fmt --all -- --check
cargo test --locked --test console_config_contract --test console_document_contract --test console_hot_reload_contract --test console_persistence_contract --test console_transaction_contract --test console_security_contract --test console_api_contract --test console_ui_assets_contract --test console_splitter_contract
cargo check --locked --all-targets
python -m unittest discover -s tests/python -p "test_*.py" -v
node --test scripts/tests/*.test.mjs
npm test --prefix apps/desktop -- --run
npm run typecheck --prefix apps/desktop
npm run build:web --prefix apps/desktop
npm run build:tauri --prefix apps/desktop
cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check
cargo check --locked --manifest-path apps/desktop/src-tauri/Cargo.toml
```

Expected: every command exits zero.

- [ ] **Step 6: Build and smoke the approved release location**

Run: `powershell -NoProfile -ExecutionPolicy Bypass -File tools/build-gateway-release.ps1 -ReleaseRoot "C:\Users\Public\nas_home\AI\GameEditor\Neuro\release\Gateway"`  
Expected: a new immutable package directory is created only under the approved release root.

Run:

```powershell
$releaseDir = Get-ChildItem -LiteralPath "C:\Users\Public\nas_home\AI\GameEditor\Neuro\release\Gateway" -Directory |
    Sort-Object LastWriteTimeUtc -Descending |
    Select-Object -First 1
powershell -NoProfile -ExecutionPolicy Bypass -File tools/smoke-gateway-ui-release.ps1 -ReleaseDir $releaseDir.FullName -LaunchUi
```

Expected: headless binary serves `/ui/`, Tauri finds its sidecar/state directory, checksums match, and launched processes are cleaned.

- [ ] **Step 7: Record final evidence and commit**

Update the progress ledger with exact test counts, E2E run ID, package path, package SHA-256, screenshots, and remaining external-only risks. Do not create a `Vx.y.z` tag or GitHub Release.

```powershell
git add apps/desktop/e2e tools/run-gateway-console-e2e.ps1 tests/python/test_gateway_console_e2e_contract.py docs/progress/MASTER.md
git commit -m "test(console): verify browser management product end to end"
```

## Plan Self-Review

- Every design requirement maps to at least one task: configuration runtime (Tasks 1-5), bootstrap/security/events (Task 6), API (Task 7), embedded UI (Task 8), shared Browser/Tauri frontend (Tasks 9-12), splitter convergence (Task 13), delivery (Task 14), and complete verification (Task 15).
- Every production batch begins with a failing focused test and records the expected failure before implementation.
- All configuration mutations use one whole-document revision transaction; no granular provider or credential write path bypasses it.
- Existing development secrets and the pre-existing `routes.yaml` modification are preserved and excluded from plan commits.
- Release artifacts remain under `C:\Users\Public\nas_home\AI\GameEditor\Neuro\release\Gateway`; mutable runtime state is external to the package.

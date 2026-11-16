# Local EXE data and upgrade compatibility

Both gateway.exe and gateway-ui.exe resolve one data root using the shared
crates/gateway-local-data crate. The working directory is never used for this choice.

1. An explicit absolute GATEWAY_DATA_DIR override wins. The existing
   GATEWAY_UI_DATA_DIR override remains available for isolated desktop tests.
2. If .ng already exists beside the executable, use that directory (portable mode).
3. Otherwise use the current user's home directory plus .ng, such as
   C:/Users/vmjcv/.ng. Never automatically create the portable directory.

Replacing the executable or launching another release therefore keeps the same data.
The desktop passes the selected root to its backend, even when a separately located
backend executable is configured. Settings and console WebViews also store their
profiles under this root. Data is not copied between home and portable roots implicitly.

For an explicitly selected absolute `GATEWAY_DATA_DIR`, Windows local startup also
preserves an explicit `AI_GATEWAY_OBJECT_STORAGE_DRIVER=local` plus an absolute
`AI_GATEWAY_OBJECT_STORAGE_LOCAL_DIR`. This opt-in separates the object owner from
the SQLite/routes root without copying credentials. With no complete selection,
the existing `<data-root>/objects` default remains isolated; routes and state always
stay under the selected local data root. Selecting an existing object root is an
intentional sharing boundary, not a fully isolated object store.

The daily devtest builder opts into a separate compile-time default:
`home/.ng-devtest-4300`, ignoring executable-adjacent `.ng` unless an explicit
absolute override is supplied. Normal/release binaries retain the rules above.
See [devtest-build.md](devtest-build.md) for port, override and isolation boundaries.

## Layout

| Path below .ng | Owner |
| --- | --- |
| local/routes.yaml | Local provider and credential bootstrap/export |
| local/state/console | Active revision, transactions, secret access state and recovery |
| local/state/console/luma-client-registrations.json | Bounded public Luma MCP registration receipts: durable pre-send uncertainty and confirmed client/redirect; no access tokens; same-owner, same-state-root recovery without automatic registration replay |
| local/state/runtime.sqlite3, runtime.sqlite3-wal, runtime.sqlite3-shm | Durable refill queue, request history, pricing, credential-model health, local access keys and balances |
| local/state/runtime.sqlite3 (local_freebuff_device_keys / local_freebuff_registrations / local_freebuff_registration_replacements) | One sealed FreeBuff installation key, exact account/audience receipts and bounded replacement-attempt overlays preserving the previous receipt until confirmed |
| local/state/runtime.sqlite3 (local_freebuff_session_claims / local_freebuff_session_closeouts) | Sealed exact FreeBuff active claims and final closeout receipts; prepare-before-send, unresolved recovery, exact atomic archive/release and offline bounded history, without automatic repurchase or cleanup |
| local/state/runtime.sqlite3 (local_freebuff_runs) | 原 claim 绑定的加密 run/client 身份、发送前意图、费用观测/已知累计前缀和 FINISH 接受事实；原 claim/run preimage 原子 CAS，不自动续跑、重放、end 或重买 |
| local/state/access-key-seal.json | Existing same-root AES seal key, shared by access-key copies and FreeBuff device/registration/session-claim owners; preserve it with the matching database |
| local/state/refill-locks | Bounded OS lock stripes for cross-process credential delivery |
| objects | Runtime objects and browser-backed provider material |
| credentials | Original/imported credential files |
| connection.json, profiles | Desktop connection selection and profiles |
| logs, runtime, webview | Desktop logs, process markers, WebView profile |
| storage-version, storage-*.lock, migrations | Compiled startup compatibility runner |
| backups | Explicitly created manual migration recovery copies |

`GATEWAY_STORAGE_MODE=local` explicitly selects SQLite and a standalone runtime on
every supported OS. `GATEWAY_STORAGE_MODE=server` retains PostgreSQL + Redis and
overrides the legacy `GATEWAY_CONSOLE_STORAGE=local` setting. The desktop-managed
backend always selects local storage and rejects an explicit server-mode conflict.
A desktop UI connected to a remote URL uses that remote server's storage.

Windows gateway.exe without explicit server storage settings starts the same local
standalone mode and does not discover a parent directory's .env. Existing explicit
Redis/database/routes/state settings retain server selection unless local mode is
explicitly requested. Docker and non-Windows server defaults are unchanged.

Local initialization ignores external database URLs, installs only local auth and
route owners, and starts local maintenance/refill work without server credential-cache,
database refresh or distributed scheduler jobs. No Redis, PostgreSQL or Garnet service
is bundled or launched. The server-pool handle is closed in local mode, so an accidental
server-only path fails immediately without attempting a network connection.
Public readiness probes SQLite directly and permits first-run management with an
empty route catalog. Internal readiness reports a separate SQLite dependency and
does not require Redis, an HMAC signing secret or a public server URL in local mode.

Ephemeral affinity is bounded to 4,096 entries with a one-hour TTL. The local fixed-window
rate-limit adapter admits all configured dimensions atomically and bounds counters to
8,192 live windows; it rejects capacity overflow rather than evicting live limits.
Durable permissions and balances never depend on these caches. Local request audits
persist usage directly; server mode additionally publishes its existing Redis reports.
Provider model health stays in SQLite. Distributed provider-quota polling and advanced
server catalog/project services retain their server owners.

Local normal access-key lifecycle and request authorization also use SQLite. See
[local-access-keys.md](local-access-keys.md) for secret handling, restrictions and
the shared balance rules and the boundary with server catalog features.

FreeBuff device registration reuses this selected LocalRuntime root and seal, with
separate-purpose AAD and additive schema-1 tables. It never writes private keys to
routes, release payloads or a new plaintext file. Missing/corrupt seal or existing
device material fails closed, including a database containing only FreeBuff rows;
it does not silently create a replacement key. This same-directory AES seal is not
an OS vault or protection against access to the complete root. The fixed
`refill-locks/freebuff-device-registration.lock` uses an OS lifetime guard, not
deletion of the lock file. Explicit replacement keeps the previous sealed receipt
until atomic promotion, but uncertain replacements never use it as a signed or
unsigned fallback. Local status does not generate keys or contact upstream.
Server mode does not fall back to a local device store.
See [the management contract](provider-freebuff-login.md#51-本地安装设备的显式注册-api)
for explicit registration and single-use uncertain-retry confirmation.

The FreeBuff session-claim owner adds a separate schema-1 table and purpose-bound
sealed receipt in this same root. Its stable provider/credential slot does not
change when token/origin/client changes: a mismatched original authority blocks
recovery rather than looking absent. Existing claims are never automatically
expired, removed, renewed or repurchased. Fixed 256 OS lock stripes cover the
one-shot send and active observation; a collision returns busy. Session-only
rows, closeout-only history and run-only records also prevent silently replacing a missing seal.
Normal saved-account management can explicitly read the offline journal/history
and end only its exact original claim; final receipt archive and active release
remain one transaction with the real Grant/revision fence. A read needs no device
or upstream; an end needs the existing registration and fresh signed catalog.
History is not active authority. Catalog CLI / Local non-streaming Chat now uses
the normal request-owned per-purchase approval; other public lanes and Server
admission storage remain incomplete. See
[正常管理原claim收尾子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-07-provider-channel-expansion-freebuff-managed-closeout.md).

原 run 记录在同一 schema-1 内 additive 初始化，旧 claim 无 run 时返回缺少事实，而不补造 ID/费用。
START、模型、FINISH 的原意图必须先持久化；当前未知费用与此前已知累计分开，HTTP 接受不当结算。
正常管理 claim/history 读取与页面只展示恢复事实，不返回 remote run/client ID、body 或认证材料，
不生成恢复执行许可。实际进程/跨进程、远端 lease 与未知结果显式对账仍需后续验收；见
[原 run 持久化子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-08-provider-channel-expansion-freebuff-run-journal.md)。

Local refill workers use the existing authenticated HTTP claim/renew/complete APIs.
The status response identifies storageBackend=sqlite and streamKey=null. Provider
deduplication, idempotency and leases are durable. Credential deliveries are prepared
before route commit, so retries after a crash reuse the same credential IDs and payload.
Prepared secrets are deleted when the task terminates; they must never enter logs.
The local queue does not expose a Redis wire protocol or Redis stream.

Request audits and model health are recorded for buffered and streaming calls. Cost
reports use the same price resolution as server mode and keep unpriced costs null.
Price edits persist in SQLite. Request history has a 90-day retention window; cleanup
runs in batches of at most 500 rows. Instance heartbeats run every 10 seconds and
orphaned running requests become cancelled after a 120-second stale threshold.
Graceful shutdown drains HTTP requests before retiring the instance and closing SQLite.
Back up SQLite with its online backup API or stop every Gateway using the data root
before copying it; copying only runtime.sqlite3 while WAL writers run is unsafe.

## Compatibility policy

The first storage version is 1. It adopts the existing YAML and console-journal format
without rewriting its contents. Adding optional fields or improving compatible readers
must not bump this version. Prefer serde defaults/aliases and compatible queries over
database rewrites. The data directory is stable across ordinary binary updates.

When a genuinely incompatible local change is required, register a compiled migration
step in gateway-local-data/src/schema.rs, declare every file it owns, and advance the
version only after success. The runner snapshots declared files, serializes startups,
retains backups, restores partial writes before retries, and blocks a newer-schema
directory from being opened by an older binary. Shared lifetime leases prevent a
future incompatible step from running while an older participating process is alive.
Invalid versions fail safely; unversioned legacy data is accepted as V0.

Each new migration must include old-format fixtures, byte/secret/ID preservation tests,
repeat-startup tests, failure and interrupted-run recovery tests. Transformations must
be deterministic and touch only declared files. Database-engine-specific consistent
backup/transaction logic belongs in that migration; this runner does not migrate a
live external PostgreSQL database. No normal upgrade should require manual SQL.

## Initial manual migration

The 2026-09-26 installation migration is a copy-and-verify operation, not deletion of
old data. Keep previous AppData data, deployed routes/revisions and .neuro imports
as backups. Original credential files are retained; normalized account entries are
also placed into the local route document so the UI can display them. Preserve runtime
object keys and copy their complete referenced object tree. Do not treat archiving
files alone as successful account migration, or presence as proof of valid upstream
credentials. Never copy a Redis authority journal into local active state.

Source backups and copied data contain secrets and stay under the user's .ng, not in
Git or release evidence. Acceptance reports contain only counts and integrity results.

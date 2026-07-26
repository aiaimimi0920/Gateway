# Gateway Operations Manual

This manual covers the Gateway-owned runtime state and the operational surfaces
implemented by the Rust Gateway. It does not authorize changes to an existing
live Redis, PostgreSQL database, object-storage directory, worker, or splitter.
Every mutating command below requires an explicit endpoint, container name,
connection string, namespace, or destination path.

## Release Preconditions And Evidence

The portable Windows release has three separate prerequisite groups:

| Activity | Required tooling | Evidence location |
| --- | --- | --- |
| Build `gateway.exe` and the desktop shell | Rust/cargo, Node.js/npm, locked dependency network access | Source provenance at `target/release/gateway-build-provenance.json`, plus immutable copies in the package and `target/release-evidence/<versionId>/gateway-build-provenance.json` |
| Run browser-backed provider workers | Node.js/npm on the target host; run `npm ci` in the extracted `scripts` directory | `target/release-evidence/<versionId>/browser-workers.log` |
| Run validators and the full offline/provider evidence matrix | Source repository only; Python 3, Rust/cargo, and the repository-owned verification helpers requested by the selected matrix | `target/release-evidence/<versionId>/python-tests.log` and line evidence artifacts |

`package-gateway-release.ps1` copies only the production `routes.yaml` and
`routes.example.yaml` route files, plus `.env.example`, the verified build
provenance, and the Gateway-owned canary under `scripts/`. It excludes
`scripts/node_modules` and all build output. A package is published through a
same-volume atomic rename into
`release/Gateway/<versionId>`; an existing version is immutable. The package
manifest records the source tree SHA-256 fingerprint and dirty state, and the
build provenance binds that fingerprint to both executable hashes. Never use
`-SkipBuild` merely because an old executable happens to exist.
If the post-rename evidence provenance copy fails, the packager rolls back only
the newly published version directory and exits nonzero; existing releases and
existing evidence remain immutable.

The full `tools/run-gateway-line-evidence.ps1` matrix runner is a source repository only
tool and is not copied into the portable release. It depends on
`Cargo.toml`, `src/`, and deployment verification helpers. The package exposes
the standalone `scripts/invoke-gateway-live-provider-canary.ps1` instead; its
default invocation is a no-network dry run, and live calls still require
explicit opt-in plus Gateway URL, API key, and target definitions.

The release directory is an immutable payload. Store runtime and UI smoke
output under `target/release-evidence/<versionId>` (or a separately
managed evidence root), not inside the package, otherwise the added files would
correctly fail checksum verification on the next run. The minimum release
evidence set is:

- `manifest.json` and `checksums.sha256` from the package;
- immutable `gateway-build-provenance.json` bound to the version id;
- two independent `runtime-smoke-*.json` runs from `smoke-gateway-packaged-runtime.ps1`;
- captured UI artifact/launch smoke logs;
- Python validator and browser-worker test logs;
- the exact release version id and the SHA-256 of the two manifest files.

Before the headless executable is started, the packaged runtime smoke verifies
every checksum path and digest, the manifest byte/hash records, and the exact
file set. The UI smoke applies the same verification and, on a failed launch,
terminates only newly observed `gateway.exe` processes so an unrelated
existing Gateway instance is not touched.

Run `build-gateway-release.ps1` once, then invoke the packager with `-SkipBuild`.
Alternatively invoke the packager without `-SkipBuild` and let it perform the
build. Do not execute both build paths for the same version id.

## Safety Contract

- Run the backup and restore tools first with `-DryRun`.
- Never infer a live endpoint from the current process environment. Pass the
  intended endpoint or container explicitly to the tool.
- Use a new backup destination. `backup-gateway-state.ps1` refuses an existing
  destination and refuses overlap with local object storage.
- Restore into a new Redis namespace, empty PostgreSQL database, and empty local
  object-storage directory. In-place recovery is a disaster procedure, not the
  normal verification path.
- Do not put credentials in command transcripts, issue notes, or evidence JSON.
  The tools suppress native command stderr and never persist connection strings.
- A multi-component backup is ordered, not globally transactional. Drain request
  writers and wait for active work to settle before capturing Redis, PostgreSQL,
  and object storage as one recovery point.
- Keep `manifest.json` and `checksums.sha256` with the payload. Restore verifies
  every checksum before it writes any selected component.

Gateway Redis keys use the `gw:` prefix. The backup script uses namespace-scoped
`SCAN`, `PTTL`, and `DUMP`; restore remaps suffixes under an explicit destination
prefix and uses `RESTORE`. It never clears a Redis database.

## Runtime Surfaces

| Endpoint | Access | Expected use |
| --- | --- | --- |
| `GET /healthz` | Public | Process liveness. A running process returns HTTP 200. |
| `GET /readyz` | Public | Traffic readiness. HTTP 503 identifies dependency or draining state. |
| `GET /metrics` | Public | Prometheus metrics and build identity. |
| `GET /v1/internal/gateway/readiness` | Management | Redis, PostgreSQL, local/S3-compatible object storage, configuration, provider, drain, and active-request summary. |
| `GET /v1/internal/gateway/operations/summary` | Management | Secret-free build/runtime identity, lifecycle, dependency readiness, routing/cache counts, request metrics, and provider status. |
| `POST /v1/internal/gateway/runtime/drain` | Management | Mark a worker draining and request graceful process shutdown. |
| `GET /v1/internal/gateway/splitter/status` | Management | Active worker and worker lifecycle state. |
| `POST /v1/internal/gateway/splitter/reload` | Management | Start a replacement, wait for readiness, switch traffic, then drain the previous worker. |
| `GET /v1/internal/gateway/browser-executor/health` | Management | Browser executor capacity and health. |
| `GET /v1/internal/gateway/requests/summary` | Management | Request audit summary for incident correlation. |

Management calls accept the configured `GATEWAY_MANAGEMENT_TOKEN` through
`x-management-token`, `x-internal-api-key`, or bearer authorization. Production
must not set `GATEWAY_ALLOW_UNAUTHENTICATED_INTERNAL_ROUTES`.

```powershell
$baseUrl = "http://127.0.0.1:4200"
$managementHeaders = @{
    "x-management-token" = $env:GATEWAY_MANAGEMENT_TOKEN
}

Invoke-RestMethod -Method Get -Uri "$baseUrl/healthz"
Invoke-RestMethod -Method Get -Uri "$baseUrl/readyz"
Invoke-WebRequest -UseBasicParsing -Uri "$baseUrl/metrics"
Invoke-RestMethod `
    -Method Get `
    -Uri "$baseUrl/v1/internal/gateway/readiness" `
    -Headers $managementHeaders
.\tools\get-gateway-operator-summary.ps1 `
    -BaseUrl $baseUrl `
    -ManagementToken $env:GATEWAY_MANAGEMENT_TOKEN
```

Do not print `$managementHeaders` or enable shell tracing around these calls.

## Normal Checks

1. Confirm `/healthz` returns 200.
2. Confirm `/readyz` returns 200. During an intentional drain, 503 with
   `status=draining` is expected.
3. Run `tools/get-gateway-operator-summary.ps1` and record the secret-free build,
   role, lifecycle, dependency booleans, route/cache counts, request metrics, and
   provider counts. PostgreSQL `configured=false`, `required=false`, and
   `ready=true` is the supported DB-less mode, not a readiness failure. Summary
   `readiness.ok` also preserves the internal enterprise configuration gates;
   use `/readyz` as the narrower traffic-admission decision.
4. Read `/metrics` and compare request rate, error rate, latency, in-flight
   work, provider errors, AIMD limits, route configuration, and build version.
5. On splitter deployments, confirm the active worker through
   `/v1/internal/gateway/splitter/status`.

Alert thresholds are defined in `docs/operations-alerts.yaml`. Monitoring
loads that file directly as a Prometheus rule file. Validate it before rollout:

```powershell
.\tools\validate-gateway-alert-rules.ps1 -RequirePromtool
```

Mount or copy the file into the Prometheus rules directory and include it under
`rule_files`. The HTTP alerts require blackbox scrape jobs named
`gateway-liveness`, `gateway-readiness`, and `gateway-operator-summary`. Configure
the authenticated summary probe in secret-managed blackbox configuration; do not
commit the management token to this rule file or Prometheus configuration.
The measurable SLI definitions, longer-window objectives, metric limitations,
and dependency failure matrix are defined in `docs/sli-slo.md`.

## Incident Classification

| Class | Evidence | First action |
| --- | --- | --- |
| Process unavailable | `/healthz` connection failure | Retain process/container logs and replace the instance. |
| Redis unavailable | `/readyz` is 503 and the Redis check is false | Stop rollout activity, verify the explicitly configured Redis endpoint, and avoid restarting all workers at once. |
| PostgreSQL unavailable | PostgreSQL is configured and `/readyz` or management readiness reports its probe failed | Preserve DB logs, stop writes, and verify failover or recovery on a separate target. An unconfigured optional PostgreSQL dependency is not an incident. |
| Object storage unavailable | Management readiness reports `objectStorage=false` | Check the local directory or S3-compatible endpoint without logging access material. |
| Planned drain | `/readyz` is 503 with `status=draining` | Confirm the load balancer or splitter has moved traffic; do not restart the worker back into service. |
| Splitter replacement failure | Reload returns 502 or status has no active worker | Keep the previous package, inspect replacement logs, then reload the known-good executable. |
| Authentication | HTTP 401/403 or `authentication` failure class | Isolate the affected access key or provider credential. Do not retry globally. |
| Rate limit | HTTP 429 or `rate_limit` failure class | Back off and inspect provider/model-specific counters. |
| Quota exhaustion | `insufficient_quota` failure class | Remove the exhausted credential from routing or replenish it through the owning system. |
| Upstream/provider failure | Request audit identifies the concrete error; provider metrics use the current failure taxonomy such as `provider_transient` | Correlate provider, model, request ID, trace context, and request audit before fallback or rollback. Do not invent a metric label for timeout or retry exhaustion. |
| Browser executor unavailable | Browser executor health is degraded and browser-backed routes fail | Drain only affected capacity and validate replacement browser nodes before restoring traffic. |
| Configuration regression | `gateway_routes_configured != 1`, no providers, or build identity differs | Reject or roll back the package/configuration change. |

Use `x-request-id` and `traceparent` from the response to correlate logs. Never
copy authorization headers, provider cookies, session state, or connection
strings into incident notes.

## Worker Drain And Replacement

The splitter reload path is preferred for a rolling replacement because it
starts the new worker, waits for `/readyz`, switches the active worker, and only
then drains the old worker.

```powershell
$baseUrl = "http://127.0.0.1:4200"
$managementHeaders = @{
    "x-management-token" = $env:GATEWAY_MANAGEMENT_TOKEN
}

$before = Invoke-RestMethod `
    -Method Get `
    -Uri "$baseUrl/v1/internal/gateway/splitter/status" `
    -Headers $managementHeaders

$reloadBody = @{
    executablePath = "D:\Gateway\candidate\gateway.exe"
    readyTimeoutSecs = 60
    shutdownTimeoutSecs = 600
} | ConvertTo-Json

$after = Invoke-RestMethod `
    -Method Post `
    -Uri "$baseUrl/v1/internal/gateway/splitter/reload" `
    -Headers $managementHeaders `
    -ContentType "application/json" `
    -Body $reloadBody
```

Verify that the returned active worker is the candidate and that `/readyz`
remains 200. Continue polling splitter status until the previous worker reaches
`exited`. Investigate persistent `gateway_request_drain_rejections_total`
increases because they indicate traffic still reached the draining worker.

For a directly addressed standalone/worker process, first remove it from
traffic, then request drain:

```powershell
$drainBody = @{ reason = "planned_maintenance" } | ConvertTo-Json
Invoke-RestMethod `
    -Method Post `
    -Uri "$workerBaseUrl/v1/internal/gateway/runtime/drain" `
    -Headers $managementHeaders `
    -ContentType "application/json" `
    -Body $drainBody
```

This endpoint requests process shutdown. Do not call it as a read-only probe.

## Backup

The tool supports direct `redis-cli` access with `-RedisUrl`, `docker exec` with
`-RedisContainer`, optional PostgreSQL through a local `pg_dump` executable or
`-PostgresContainer`, and local object-storage copy. None of those sources has
a live default.

First run a side-effect-free plan. The dry run does not contact Redis or
PostgreSQL and does not create the destination:

```powershell
$backupPath = "D:\GatewayBackups\2026-07-18T120000Z"
.\tools\backup-gateway-state.ps1 `
    -DestinationPath $backupPath `
    -RedisUrl $env:GATEWAY_REDIS_URL `
    -RedisNamespace "gw:" `
    -PostgresConnection $env:GATEWAY_DATABASE_URL `
    -ObjectStoragePath $env:AI_GATEWAY_OBJECT_STORAGE_LOCAL_DIR `
    -DryRun
```

After draining writers and validating the plan, repeat without `-DryRun`.
For Redis running in a named container, replace `-RedisUrl` with:

```powershell
-RedisContainer "gateway-redis" -RedisDatabase 0
```

If `pg_dump` is available only inside the selected PostgreSQL container, add
`-PostgresContainer`. The connection string is interpreted inside that
container, so its host must be reachable from the container itself:

```powershell
-PostgresConnection $env:GATEWAY_DATABASE_URL `
    -PostgresContainer "gateway-postgres" `
    -DockerPath "docker"
```

Container-mode `pg_dump` output is streamed directly into the host backup
directory. The tool does not write a database dump into the source container.

The resulting directory contains:

```text
manifest.json
checksums.sha256
redis/*.dump
postgresql/gateway.sql
object-storage/**
```

The manifest records component counts, payload paths, TTLs, and hashes, but not
Redis or PostgreSQL connection strings. Store the directory on a destination
with access controls appropriate for provider credentials and browser state.

## Restore

Restore validates all backup checksums before any selected destination is
written. Use new isolated destinations for rehearsal:

```powershell
$backupPath = "D:\GatewayBackups\2026-07-18T120000Z"
$restoreObjectRoot = "D:\GatewayRecovery\objects-20260718"
.\tools\restore-gateway-state.ps1 `
    -BackupPath $backupPath `
    -RedisUrl $env:RECOVERY_REDIS_URL `
    -RedisDestinationNamespace "gw-recovery-20260718:" `
    -PostgresConnection $env:RECOVERY_DATABASE_URL `
    -ConfirmPostgresRestore `
    -ObjectStorageDestinationPath $restoreObjectRoot `
    -DryRun
```

Repeat without `-DryRun` only after confirming the recovery Redis and database
are disposable targets. PostgreSQL restore always requires
`-ConfirmPostgresRestore`. Redis restore rejects the source namespace by
default. Existing Redis keys and non-empty object storage are also rejected.

For a PostgreSQL destination reached through a named container, add
`-PostgresContainer "gateway-postgres"`. The checksum-verified SQL payload is
copied to a unique file under that container's `/tmp`, passed to its local
`psql`, and removed in `finally`. `-PsqlPath` remains available for a local
client when `-PostgresContainer` is omitted.

The disaster-only switches are:

- `-AllowSameRedisNamespace`: permits the source prefix as the destination.
- `-OverwriteRedisKeys`: deletes only colliding mapped keys before restore.
- `-OverwriteObjectStorage`: replaces colliding files but does not delete
  unrelated files.

These switches do not make an in-place restore transactional. Stop all writers,
capture current evidence, and obtain an explicit incident decision before use.

## Isolated Recovery Verification

`verify-gateway-recovery.ps1` creates a random `gateway-recovery-*` Redis
container with `--network none`, publishes no port, writes only synthetic keys,
backs up one source namespace, restores a different namespace, compares values
and TTLs, compares local object-storage hashes, writes evidence, and removes the
container and temporary files in `finally`.

```powershell
.\tools\verify-gateway-recovery.ps1 `
    -EvidencePath "D:\GatewayEvidence\recovery-20260718.json"
```

Use `-DryRun` to inspect the plan or `-KeepArtifacts` to retain the temporary
backup for debugging. The Docker image defaults to `redis:7-alpine`; pin an
approved digest with `-RedisImage` in controlled environments. This verifier
covers Redis and local object storage. Exercise PostgreSQL separately against
explicit disposable source and destination databases using the backup and
restore commands above. The opt-in operations contract performs that rehearsal
with a unique `gateway-recovery-postgres-*` container, two random databases,
and schema-only plus data-only dump comparison:

```powershell
$env:GATEWAY_RUN_RECOVERY_DOCKER_TESTS = "1"
python -m unittest discover `
    -s tests/python `
    -p "test_gateway_operations_contract.py" `
    -v
```

## Rollback

### Runtime or package rollback

1. Stop rollout activity and retain the candidate worker logs, `/metrics`,
   readiness summary, splitter status, and request audit evidence.
2. Call splitter reload with the previous known-good executable path.
3. Confirm the replacement becomes active and `/readyz` stays 200.
4. Wait for the failed worker to reach `exited`.
5. Confirm request error ratio, latency, in-flight work, and provider-specific
   errors return to the pre-rollout baseline.

### State rollback

1. Do not overwrite the live state first. Restore the selected backup into a
   new Redis namespace, new PostgreSQL database, and new object directory.
2. Run application readiness and representative offline requests against those
   recovered destinations.
3. Switch configuration to the recovered targets during a controlled drain.
4. Keep the prior targets read-only until request audit, quota settlement, and
   provider credential behavior are verified.
5. If validation fails, switch configuration back to the prior targets. Do not
   merge partially restored Redis or object-storage content into the live set.

## Backup Retention And Evidence

- Retain at least the latest successful recovery point and one earlier point
  outside the runtime host.
- Record the backup directory, manifest SHA-256, checksum-file SHA-256, source
  package version, and verification evidence path.
- Test recovery after changes to Redis key layout, database migrations, object
  storage layout, or the backup/restore tools.
- Treat a backup as usable only after checksum validation and an isolated
  restore. File creation alone is not recovery evidence.

# Local runtime dependency findings

Date: 2026-09-26
Status: local refill and credential-pool telemetry implemented; packaged runtime acceptance passed.

## Completed cleanup

The live desktop backend accepted the revision-checked route update. Removed 43 expired Codex pool credentials and one additional expired provider-level fallback token. The empty Codex provider and its exclusive route were removed to prevent a synthesized default account from reappearing. Account-group references were cleaned.

Remaining: 31 providers / 190 explicit credentials. All remaining credential values are preserved; saving materialized schema defaults and canonicalized header_name to headerName for two session-auth entries. Validation reports no repair requirement. The same counts are persisted in the bootstrap routes file.

The 43 corresponding files were moved out of credentials/legacy-neuro into C:/Users/vmjcv/.ng/backups/20260926-expired-codex-removal/expired-imports. The backup and receipt remain private under the user's .ng. The running user desktop was not stopped.

## Confirmed causes

- src/local_installation.rs enables local route/config storage without Redis or PostgreSQL services.
- src/http/routes/internal_credential_refill.rs still always lists Redis refill tasks. src/credential_refill/storage.rs maps connection failure to the displayed Redis error. No local refill store currently exists.
- src/http/routes/internal_requests/runtime_analysis.rs and the audit summary endpoint require PostgreSQL. Missing PostgreSQL is an expected deployment configuration, currently surfaced as a pool-wide failure.
- apps/desktop/src/features/console/useConsoleRouteData.ts requests these endpoints unconditionally and BrowserConsoleApp.tsx displays the resulting warnings.
- Existing process-local concurrency and provider metrics are available, but provider-attempt counters must not be presented as completed logical-request statistics. Local finalized-token/cost history does not yet exist.

## Confirmed implementation scope

The user selected complete local functionality. Local/desktop runtime uses an embedded SQLite database in the existing resolved console state directory. Credentials remain in their existing route document; no credential schema migration or reimport is required. Server mode retains Redis and PostgreSQL.

Implemented owners: durable refill creation/deduplication, provider-scoped idempotency, claim/renew/expiry, prepared credential delivery and terminal state; logical-request audits and summaries; live pressure; token/cost aggregation with the shared pricing projection and persisted price edits; credential-model health and route penalties. HTTP management auth is unchanged. Local refill workers poll the existing claim API; storageBackend is sqlite and streamKey is null. Redis deployments retain their existing stream.

SQLite schema initialization is transactional and rejects newer versions. Request history retains 90 days, with bounded cleanup batches. Instance heartbeats run every 10 seconds; requests belonging to an instance stale for 120 seconds are marked cancelled, without cancelling another live instance. Graceful shutdown retires the current instance after requests drain. WAL and FULL synchronous writes preserve committed local state.

Release: ../release/Gateway/gateway-product-20260926-local-full-r1. Acceptance: ../release/Gateway/gateway-product-20260926-local-full-r1-acceptance.md. Rust focused tests: 100 passed; frontend API tests: 7 passed; line-checker tests: 31 passed. Formatting, ratchet, typecheck, production audits, official build and packaging passed.

The packaged EXE passed isolated no-Redis/no-PostgreSQL management API tests, concurrent refill creation/claim/delivery, actual lease expiry/reclaim, failure, and restart recovery. Real NVIDIA buffered and streaming calls completed with 175 total tokens; audit IDs, price edits, costs, delivered credentials and task state survived restart. The embedded browser console showed two requests and 100% success without either original warning. Test processes were drained and closed. The live routes hash was unchanged (31 providers / 190 explicit credentials); the existing user desktop was not switched.

The accepted slice covers refill and credential-pool telemetry. Other PostgreSQL-owned server catalog/project/advanced-analysis APIs retain their server owners. Native binaries were built; the shared React UI was verified through the packaged backend in a browser.

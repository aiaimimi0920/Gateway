# Anomaly export metadata owner

Date: 2026-09-24 UTC. Status: structurally verified.

The coordinator moved the two pure analysis-export metadata/view builders out
of `src/db/anomaly_incidents/export_persistence.rs` into
`src/db/anomaly_incidents/export_metadata.rs`. The persistence owner keeps all
SQL, history writes, escalation ordering, and async lifecycle behavior. The new
owner has no `PgPool`, SQL, async function, await, request, or resource
lifecycle code.

The saved persistence owner was 456 effective / 473 physical lines. The current
owners are:

- `export_persistence.rs`: 396 effective / 411 physical lines;
- `export_metadata.rs`: 69 effective / 71 physical lines.

`target/effective-line-evidence/20260924-anomaly-export-metadata/verify-structure.mjs`
compares both moved helper bodies against the saved source, allowing only the
new `pub(super)` visibility. It also proves that the persistence owner retains
the imports and call sites and that no async SQL lifecycle crossed the boundary.

Verification for this continuation:

- focused anomaly-incident tests: 6/6 passed;
- `cargo check --locked --all-targets`: passed;
- scoped `rustfmt --edition 2021 --check`: passed;
- effective-line checker tests: 19/19 passed;
- effective-line ratchet: passed;
- strict audit remains intentionally red for the existing 12 browser-profile
  payload violations (2,481 files scanned; 0 first-party violations).

No release, Docker, provider, live runtime, or persistent deployment gate was
run for this pure ownership extraction.

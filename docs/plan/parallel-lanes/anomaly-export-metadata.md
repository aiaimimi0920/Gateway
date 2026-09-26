# Anomaly export metadata ownership

Date: 2026-09-24. State: structural_green.

Scope: the pure metadata/view builders at the end of
`src/db/anomaly_incidents/export_persistence.rs`. Keep SQL, history, escalation,
transaction ordering, and async persistence in the existing owner. Move only
`build_analysis_export_incident_snapshot_metadata` and
`build_row_analysis_export_anomaly_view` into a small private owner.

Result: `export_persistence.rs` falls from 456 to 396 effective lines and the
new `export_metadata.rs` owner is 69 effective lines. Structural proof confirms
the helper bodies and call wiring are exact. The focused anomaly-incident tests
pass 6/6; all-target compilation, scoped rustfmt, checker 19/19 and ratchet
pass. Strict remains red only for the existing browser-profile payload debt.
Evidence:
`target/effective-line-evidence/20260924-anomaly-export-metadata/` and
`docs/status/2026-09-24-anomaly-export-metadata.md`.

This slice changes ownership only. It does not change database queries, request
behavior, secrets, transactions, runtime, Docker, release, or live-provider
ownership.

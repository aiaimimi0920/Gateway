# Provider line matrix closure

Status: the corrected offline matrix passed. Its source-preservation window is
closed. This receipt supersedes the running-matrix cursors in the September 24
checkpoints; it does not claim native capture, S06/S18 or release acceptance.

Continuation: `01a0d5e2-bf06-7842-afb0-5da67e1bcf76`.

## Reused evidence

Evidence root:
`target/effective-line-evidence/20260924-integration-closure/line-matrix-2026-09-24T16-20-36-595Z-3ed33061/`.

- `summary.json`: `tools/verify-gateway-line.ps1 -All -LibOnly` with shared
  `target`, exit 0, no failure, duration 11,508,289 ms.
- `line-matrix.log`: 44 provider lines, 116 Cargo filters, 1,013 passed test
  executions, zero failed/ignored tests, empty filters or missing harness
  summaries. Terminal status is `status=pass lines=44`.
- `source-before.json` and `source-after.json` match. The continuation compared
  all 2,488 protected inputs with the current bytes before further source edits:
  zero changed or missing files.
- `remaining-owned-processes.json` is empty. The owned temporary directory has
  been removed; before/after container identities, states and ports match.

The later run under `target/provider-evidence/20260925T010347341Z-bc8de2c5/`
duplicated this gate using `target/provider-line-matrix`. It left partial logs
and no successful final receipt. PID 17184 is gone; a fresh process query found
no running `run-gateway-line-evidence.ps1` or `verify-gateway-line.ps1` process.
Keep its incomplete logs, but do not use them as the acceptance result.

## Continuation and test scope

Reuse passing evidence when its relevant inputs still match. Do not start a new
full matrix merely to refresh a cursor or change the Cargo target directory.
New Node capture changes require focused capture/execution tests, affected
packaging/static checks and bounded loopback browser proof. They do not by
themselves invalidate the provider-feature Rust matrix.

The next source scope is the program-handle native response capture's page and
popup ownership before traffic starts. S20 provenance/policy activation still
requires the explicit approval in main-plan section 7.1. Formal release and
packaged runtime/UI/Docker acceptance remain open.

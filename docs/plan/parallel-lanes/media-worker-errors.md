# Media browser worker diagnostic hardening

Owner: parallel coordinator. State: `hardened_verified`.
Started and verified: 2026-09-12.

Scope is the LumaLabs, Suno and Udio `worker_contracts.rs` owners, their three
existing worker-contract test owners, and a new public diagnostic contract target.
The accepted media extraction snapshots remain immutable. Production owners
measured 84/75/84 effective lines before hardening; every resulting owner must remain
below 500. No dependency, baseline or exception changes are authorized here.

The real callers forward raw stdout/stderr and failure strings to these protocol
constructors. `GatewayError::server_error` stores them unchanged; `IntoResponse`
and pipeline failure archives publish the message. Spawn errors also insert the
local script path. Reuse the existing provider sanitizer after admitting at most
16 KiB per detail, and sanitize the final composed message to 512 characters.
Omit oversized details before regex processing. Hide the explicit script-path
argument without changing public signatures, error kinds, codes or providers.

Capture failing public regressions for auth material, controls, multibyte bounds,
the exact detail budget, composed parse errors, local paths and HTTP publication.
Then rerun the same source plus the 75 original tests, changing only the three
intentional spawn-message expectations. Verify all-targets compilation, scoped
formatting, checker/ratchet, immutable neighbors, encoding and independent Git
states. Evidence: `target/effective-line-evidence/20260912-media-worker-errors/`.

The seven public regressions all failed against unchanged production source.
Baseline compilation completed, and failures demonstrate controls, unbounded
messages and explicit path publication. The same regression source is frozen
before the production fix; the three legacy spawn expectations are the only
intentional original-test changes.

The unchanged public regressions now pass 7/7. Original protocol tests pass 75/75,
and unchanged upstream response callers pass 122/122. Scoped formatting, checker
19/19, ratchet and both Git checks pass. The three production owners now measure
102/93/102 effective lines, and the public regression owner measures 225. Exact
comparison preserves 33 public signatures, 13 unchanged production functions,
29 neighboring files and all legacy test bodies except the three documented spawn
expectations. Fresh all-targets compilation passes. Global formatting reports only
the two unchanged S06 runtime-mirror files. Strict scans 1,455 files: 31 hard,
47 mandatory and 40 soft; 78 remain above 700. All scoped native gates are terminal.
Immutable evidence is `scope.json` in the directory above.
[Acceptance report](../../status/2026-09-12-media-worker-errors.md).

This boundary does not bound original child-process output allocation. Worker
failure payload message overrides in upstream response helpers remain a distinct
boundary. S06 retains its implementation/cursor; GWP-20260908-06 source/docs freeze
and shared release-build transfer remain unreceipted. No release is scheduled by
this scoped gate.

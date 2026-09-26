# Release-candidate gate owner

This checkpoint splits the existing release-candidate verifier by lifecycle
responsibility. The script was an unreserved Gateway soft-limit candidate; the
S06 Rust/Gemini lane and shared release/build operations remain unchanged and
coordinator-owned.

## Scope and ownership

`tools/verify-gateway-release-candidate.ps1` remains the public entry point and
keeps argument parsing, common command/step recording, canary preflight, gate
ordering, JSON serialization, and exit behavior. The new
`tools/verify-gateway-release-candidate.runtime.ps1` owner contains the runtime
smoke HTTP transport and assertions, Gateway process startup, endpoint checks,
and the `finally` cleanup that stops the child process and restores every
environment variable it changes.

The entry point dot-sources the runtime owner relative to `$PSScriptRoot` and
fails closed when that owner is missing. The call order and public parameters
are unchanged. The packager copies the complete `tools` directory, so the
runtime owner is available in staged packages without a new release artifact.

## Size result

The authoritative effective-line scan changed:

- entry point: 554 -> 267 effective lines (622 -> 304 physical lines);
- runtime owner: 292 effective lines (323 physical lines);
- soft-limit inventory: 13 -> 12 files;
- strict inventory: 2,332 scanned files, 6 above 1,500, 8 in 701-1,500,
  14 above 700 including the two reserved S06 Rust hubs and twelve browser
  payload/provenance entries.

Both resulting files are below the 500-line acceptable threshold and have one
cohesive responsibility. No browser payload, baseline, policy exclusion, S06
source file, release artifact, or deployment state was changed.

## Focused verification

- Release-candidate and PowerShell portability contracts: 8 passed.
- Safe `-AsJson` release-candidate invocation with Python, line matrix, Rust,
  release build, and browser workers skipped: pass; manifest and canary
  preflight passed and all requested heavy steps were recorded as skipped.
- The runtime owner is loaded through the unchanged public entry point; no
  runtime smoke or live provider call was enabled by this checkpoint.

## Current inventory reconciliation

The size figures above are the split-time snapshot. A later 2026-09-22
governance scan supersedes its repository total with 2,335 files, 6 above
1,500, 8 in 701-1,500, and 11 in 501-700. The remaining 14 files above 700
are still exactly the two reserved S06 Rust hubs and twelve immutable
browser-profile/runtime payloads; this reconciliation does not change the
owner split or transfer S06/shared release ownership.

# Packaged runtime smoke owners

This checkpoint completes one unreserved non-Rust large-file split from the
Gateway effective-line backlog. `tools/smoke-gateway-packaged-runtime.ps1`
remains the public entry point and keeps its parameter, package-integrity-first,
runtime-smoke, evidence, process-cleanup, and disposable-Redis behavior.

## Ownership split

The original script was 680 effective lines and 743 physical lines. It now
loads two cohesive owners from the same `tools` directory after enabling strict
mode:

- `smoke-gateway-packaged-runtime.integrity.ps1`: manifest, checksum, safe-path,
  and artifact-record validation (143 effective lines).
- `smoke-gateway-packaged-runtime.runtime.ps1`: native command capture,
  disposable Redis lifecycle, evidence-path/port utilities, HTTP request/retry
  handling, and smoke response assertions (295 effective lines).
- `smoke-gateway-packaged-runtime.ps1`: parameter parsing, owner composition,
  package preflight, runtime orchestration, evidence serialization, and final
  process/container/environment cleanup (245 effective lines).

The owner files are dot-sourced by the entry point using a path relative to the
entry script. The release packager already copies the complete `tools` tree, so
the two owners are present beside the entry point in a staged package. No
external invocation path changed.

## Contract preservation

- Manifest and exact checksum validation still runs before a packaged binary is
  started, including `-IntegrityOnly` mode.
- The runtime path still uses a disposable Redis container when `-RedisUrl` is
  omitted, records evidence paths relative to the evidence directory, restores
  process environment variables, and cleans the Gateway process and container
  in `finally`.
- The Python contract now reads the composed entry point plus both owners, so
  the source contract continues to cover all moved assertions.

## Verification

- Packaged-runtime Python contract: 9 passed.
- Package-layout Python contract: 1 passed.
- `smoke-gateway-packaged-runtime.ps1 -IntegrityOnly` passed against the two
  existing immutable Gateway releases:
  `20260908-producer-mailbox-s06-123700` and
  `20260922-gemini-live-s06-closure`.
- Effective-line checker tests: 19 passed.
- Adoption-baseline ratchet: passed; soft debt decreased from 14 to 13.
- The fresh strict audit still reports the two reserved S06 Rust hubs and the
  twelve browser-profile/runtime provenance entries; this split introduces no
  new strict violation or exception.
- No release artifact was rebuilt or replaced. The existing release directories
  were read-only integrity inputs.

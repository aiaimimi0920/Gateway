# Udio and S06 integrated interim release

Version: 20260908-udio-s06-034608.
Published directory:
C:\Users\Public\nas_home\AI\GameEditor\Neuro\release\Gateway\20260908-udio-s06-034608.

## Accepted scope

- Lane E Udio manual-browser extraction: entry 797 -> 373 effective lines,
  four modules 27/161/116/139, all 26 function bodies preserved.
- S06 shared-Bytes upload ownership and bounded upload-response reads are in
  this frozen integrated source. S06 reported body12/12, upload12/12, Gemini
  583 passed / 0 failed / 3 ignored, all-target check exit0 before transfer.
  Those focused Rust results are executor evidence, not coordinator reruns.
- Coordinator personally ran the official locked release build, full Node
  matrix, focused Python package contracts and packaged runtime gates below.

## Fresh verification

| Gate | Result |
| --- | --- |
| Receipt source hashes | 4/4 match S06 terminal handoff |
| Official release builder | exit0; both headless and Tauri executables built |
| Production npm audits | both report zero vulnerabilities |
| Desktop typecheck / web build | passed |
| Full Node test matrix | 134 passed, 0 failed |
| Focused Python package contracts | 14 passed |
| Checker tests / adoption ratchet | passed / exit0 |
| Strict debt audit | exit1; 123 remaining files above700, not full closure |
| Official packager | exit0, new immutable version |
| UI artifact integrity | passed without LaunchUi |
| Runtime smoke 1 | 10 checks passed; Gateway port62747, temporary Redis port62749 |
| Runtime smoke 2 | 10 checks passed; Gateway port63014, temporary Redis port63015 |
| Final package integrity | passed; 797 actual files, exact checksum/file-set validation |
| Udio nested module inclusion | all four packaged modules equal source bytes |
| Cleanup | both Gateway runs exit0, both temporary Redis containers removed |
| Existing services | all47 prior Docker container identities/names/states preserved |
| Final process census | zero processes using the exact published gateway.exe path |

Runtime gates clear GATEWAY_DATABASE_URL and DATABASE_URL only in the isolated
invoking process. No inherited PostgreSQL, existing Redis, live4200 instance or
production deployment was used or replaced. All smoke logs and state remain
outside the immutable package.

## Build identity and package hashes

Source algorithm: sha256-git-source-list-v2; 1642 files; dirty=true.
Full source fingerprint was equal before build, after build, and in provenance
and package manifest:

    73358c51c53996e62a1bae2f4f40884b765dd2b777c02671a7a9f8035dd4010e

| Artifact | Bytes | SHA256 |
| --- | ---: | --- |
| gateway.exe | 56979968 | 195206bfca65402bc7fa05deb2cc83d9035b430d99ab1597c655fa34cc414070 |
| gateway-ui.exe | 11935744 | 324021ab7dcf55da2640da64d73599c97e878d1c042a50ea1604e5fdac814bb7 |
| manifest.json | - | 6465889973b84861050676edfdc1e232639a3292f7415bf2561d57f31b424e2e |
| checksums.sha256 | - | c15b4cc48630e6e5ff41c507916c864dcb5b0ff3229c7bdbbf843b61c7b68f99 |

Evidence: target/release-evidence/20260908-udio-s06-034608/.
Includes build.log, source-before-build.json, gateway-build-provenance.json,
browser-workers.log, python-tests.log, package.log, ui-integrity.log,
runtime-smoke-1.json, runtime-smoke-2.json, final-integrity.log and container
censuses. Packager owns the immutable evidence provenance path.

The first packaging attempt safely refused the coordinator's pre-copied
provenance at that path. Its bytes matched the official build output; moving
only that coordinator-created copy to build-output-provenance.json allowed the
packager to create its own immutable record. No existing release was overwritten
and no partial version was published by the refused attempt.

## Remaining work and window return

This is an interim integration release, not full S06/S21 acceptance. Full
Rust/Python candidate matrices, live providers, visual UI/sidecar E2E and
remaining diagnostic/ingress/concurrency work are not claimed complete.
Existing Udio lock races and credential-bearing status behavior are unchanged.

Acceptance documentation changes after publication are a new source snapshot,
not retroactive changes to build provenance. The shared build window is returned
to S06 through the handoff; all coordinator build/test/runtime handles completed.
Original debt progress remains22/145 (15.2%), with123 >700 files remaining.

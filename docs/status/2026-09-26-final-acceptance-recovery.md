# Final acceptance recovery

Status: in progress. Structural size checks pass; S06/S18 hardening and integrated
release acceptance remain open. No S06 Rust/Gemini source was changed here.

## Product checkpoint at 02:20 UTC

The official Windows build completed with exit 0. It includes both production
dependency audits, desktop typecheck and Web build, optimized Gateway and Tauri
builds, and source-bound provenance. The immutable first candidate is
`../release/Gateway/gateway-product-20260926-acceptance`. Its frozen source hash
`52906dc7461e9fa55750350fe84ddb2970e84d7d2fb7efa3b44f0b9c3e67e793` matched all
3,601 source-list entries after candidate verification. Packaged runtime and UI
launch smoke passed; all 1,439 checksum entries still matched afterward, with no
extra package files. UI evidence proves startup/liveness, not a new visual matrix.

The serialized candidate verifier returned pass. Python ran 304 tests: 300 passed
and four opt-in skips. Earlier runs remain recorded: five missing-jsonschema
failures before propagating the venv PATH, then one 6.016-second wall-clock timeout
assertion during a concurrent build. The isolated timeout case and subsequent
complete serialized run both passed without changing its 5-second assertion.
Manifest validation and live-provider canary preflight passed. The candidate's
matrix, Rust, release-build and browser-worker steps were explicitly skipped and
reuse their separate evidence; Docker and runtime smoke are also separate gates.

The resource audit after these smokes found one anonymous Redis data volume even
though the container was removed. Stop-TemporaryRedis used Docker rm --force
without --volumes. The narrow fix adds --volumes; the helper stays at 295 effective
lines. Nine packaged-runtime contracts, 31 checker tests, ratchet and strict pass.
The orphan was the only new volume, was created within the isolated smoke window,
had Docker's anonymous-volume label, had no container users, and was absent from
the baseline. It was removed by exact ID after these checks. Historical mount
events were unavailable; the attribution and cleanup receipts state that limit.

The first candidate remains immutable. The refreshed candidate name is
`gateway-product-20260926-acceptance-r2`. Its build, package, bundled runtime/UI
smoke and post-smoke resource receipts use the `r2-` prefix in
`target/release-evidence/acceptance-20260926/`; consult their actual exit/status
fields after completion. No global WSL setting has changed. Docker release-image
acceptance and the final S06/S18 integrated closure are still open.

## Earlier recovery checkpoint

The two build clients for `gateway-docker-20260926T000033Z-d870d0` and
`gateway-docker-20260926T003325Z` were cancelled after backend/health timeouts.
Both wrappers finished with exit 1 and preserved deploy/.env. Their post-exit
inventory probes overlapped Docker shutdown and failed, so separate post-recovery
inventory evidence was collected rather than treating their incomplete receipts
as successful cleanup verification. Both official Desktop restarts returned 0.

The original gatewaydev0727 gateway container used container-loopback Redis and
a Windows routes path. A Compose gateway-only recreate corrected the Redis URL
to `redis://redis:6379/0` and routes to `/data/routes.yaml`. The existing state
directory value was explicitly retained. No env file or volume contents were
edited. These are persisted container settings; a future Compose recreate must
retain these overrides because the protected .env still has its original values.

At 00:54 UTC healthz and readyz both returned 200, including Redis, database and
routing readiness. Only 127.0.0.1:4200 listened; 4226/4252 did not. The recovered
inventory has 56 containers, 168 volumes and 10 networks. Exact deltas are the
deliberately recreated gateway container and Docker's default bridge ID following
restart; all other container rows, volumes and networks match the before snapshot.
See `target/release-evidence/acceptance-20260926/docker-recovery-inventory.json`,
`persistent-health.json`, and the two retry directories' restart receipts.

No Docker build remains running. The unchanged release compilation reproduced
health timeouts after restart. WSL is capped at 8 GB memory and 2 GB swap; OOM
remains unproven. Approval was requested for a backed-up 12 GB / 8 GB configuration
and a WSL restart before another trial. No global WSL configuration was changed.

The latest complete Python run executed 304 tests with five failures and four
skips. All five failures reached the inventory generator, which directly reported
that jsonschema was missing: the outer venv executable did not propagate its
Scripts directory to PowerShell's Python subprocess. A complete corrected-PATH
rerun is underway; its `python-full-venv.json` receipt is authoritative on exit.

The latest release-candidate Node log reports 2,058 passed, zero failed and one
platform skip in `target/release-candidate-20260925T153326Z-final/`. The prior Node
snapshot differs in only the two test fixtures fixed before that successful run;
no production Node input changed. Desktop reuse audit found all 352 recorded
inputs unchanged and no uncovered current input. One retired tracked test path
was initially misclassified as uncovered; an attempted targeted run found no test
files. The corrected audit excludes nonexistent tracked files and generated
reports, and is saved in `acceptance-20260926/gate-source-reuse.json`.

## Verifier correction and focused evidence

Windows PowerShell 5.1 can promote successful native stderr output into
`NativeCommandError` under `Stop` and outer redirection. The standalone Docker
verifier now uses one native invocation boundary for build, tag and Compose,
captures stderr as text, restores the error preference, and checks the exit code.
The existing unique-project preparation guard still controls Compose cleanup.

Four isolated regressions fail before the correction and pass after it: successful
build/Compose stderr, successful tag stderr, nonzero build exit before Compose,
and nonzero Compose exit with cleanup attempts and byte-for-byte env restoration.
They resolve a temporary fake docker.cmd explicitly and never contact real Docker.
The standalone CI contracts pass 13/13. An earlier broad run was 62/64 because two
new fixture cases had a scope error; their corrected focused rerun is 4/4, not a
new claim that the broad suite was rerun successfully.

The verifier is 166 effective / 188 physical lines; the new suite is 111 effective /
119 physical lines under the repository lexer. Both are UTF-8 without BOM or
trailing whitespace. Checker tests pass 31/31; ratchet and strict pass across 2,505
files, including 14 separately classified immutable runtime assets. No governed
source exceeds 700 effective lines. Fresh Neuro development-standard contract and
Gateway/Neuro worktree and index diff checks pass. Git emitted line-ending warnings.

## Current compilation and earlier release evidence

Evidence root: `target/release-evidence/acceptance-reuse-20260925T222021Z/`.

- `cargo-check-confirm.json`: exit 0 for offline locked all-target compilation,
  with one Cargo job, incremental disabled and prebuilt web UI enabled. The earlier
  `cargo-check-exit.json` has a null exit code and must not be used as success proof.
- `source-reuse.json`: 2,427 matching and 61 changed recorded inputs against the
  historical matrix snapshot. No recorded Rust/Cargo/toolchain/build input changed.
  Four build_support files and two examples are uncovered by that snapshot; this
  does not establish when they were added. Fresh all-target compilation covers the
  current tree, but does not replace runtime/protocol evidence.
- `package-evidence-reuse.json`: the earlier immutable package's manifest,
  checksums file and two executable hashes match the original receipts. This is
  key-artifact reuse evidence, not a fresh full package or runtime verification.
  Whole-source provenance differs, so that package cannot certify current inputs.
- Focused test logs, checker logs and Docker restart receipts are in this root.

The earlier package at `../release/Gateway/gateway-product-20260925-1618` remains
untouched. A new current-source package requires the documented rebuild workflow;
copying old binaries with SkipBuild would not resolve provenance mismatch.

## Earlier Docker recovery and supervised retry

The official Docker Desktop restart returned exit 0 after about 97 seconds.
The server then reported 29.5.3; inventory was 56 containers, 168 volumes and 10
networks. The three existing gatewaydev0727 services were healthy on port 4200.
The pre-retry deploy/.env SHA-256 was
`30763B888148AFA24961E69E552520C134B1E752BD06919FEA9DAA863FB1E87E`.

The real BuildImage retry uses port 4252 and unique image tag
`local-verify-gateway-docker-20260926T000033Z-d870d0`. Its evidence root is
`target/release-evidence/gateway-docker-20260926T000033Z-d870d0/`.
The wrapper records full before/after object inventories and env hashes on exit.
At this checkpoint the wrapper is still running, the receipt says running, and
stdout stops at Rust compilation warnings after build step 33 at 81.88 seconds.
No completed image or Compose acceptance is claimed.

Subsequent bounded Docker version and WSL process probes timed out; port 4200
health also timed out. Host free physical memory was approximately 1.4 GiB of
32 GiB, with vmmemWSL around 8 GiB. This suggests pressure but does not establish
OOM. Docker host logs show repeated IPC ping deadlines and UI networks HTTP 500s.
Older shutdown/reprovision entries precede the successful restart and must not be
misrepresented as proof of the new stall's cause. No matching Windows resource
exhaustion or disk event was found by the bounded diagnostic query.

Cancellation/restart approval was requested because another restart interrupts
other containers. Do not launch another build while this wrapper is active. After
completion or cancellation, record the real exit/receipt, per-object inventory
delta, env hash, absence of 4226/4252 listeners, and original 4200 health/readiness.
The new Docker gate remains unverified. Do not turn the running receipt into pass.

## Remaining acceptance

Resolve the Docker retry and restore/verify the persistent 4200 stack. Retain the
S06 source reservation and close remaining S06/S18 owner-level hardening evidence.
Reconcile current-input Python, Node, Rust, desktop/Tauri and provider matrix
evidence before the final release-candidate gate. Build a new immutable candidate
with matching provenance and complete its integrity, packaged runtime, UI and
Docker gates. Structural size closure alone does not close these acceptance items.

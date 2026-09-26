# Media browser worker diagnostic hardening

Accepted on 2026-09-12. The LumaLabs, Suno and Udio worker error constructors now
bound and sanitize untrusted diagnostics before API and durable message boundaries.
This is one hardening checkpoint in the continuing Gateway plan.

## Failure and correction

The original constructors formatted raw stderr, stdout and error strings into
`GatewayError::server_error`; spawn failures also formatted the explicit local
script path. The constructor stores messages unchanged. `IntoResponse` publishes
them, and pipeline failure archives clone them into stored error payloads.
The seven new public regressions all failed against unchanged production code.

Each provider-local owner now admits at most 16 KiB per detail before copying or
regex processing. Larger details become a fixed omission marker. Admitted details
use the existing `sanitize_provider_error_message`; the final composed message is
sanitized again and bounded to 512 characters. This covers both parse-error
arguments independently, multibyte text, controls and sensitive auth material.
Spawn errors omit the explicit script-path argument while retaining the safe cause.

The 20 changed constructors retain their public signatures, provider identities,
error kinds, HTTP status, codes, retryability and fallback policy. Ordinary safe
messages retain their existing contracts, except for the intentional removal of
the spawn path. The three production owners are 102/93/102 effective lines. The
new public contract target is 225; all seven scoped source/test files are below 500.

Exact comparison preserves 33 public function signatures, 13 unchanged production
functions and 29 neighboring files. The 75 original tests remain present; only
the three spawn-test names and expected messages change. The paired regression
source is byte-identical before and after the fix, with SHA-256:
`c4c7911eef0abd7c0f4f8a38c81d8ee281b018aa8b220bc9cc1d9510734fd6a4`.

## Verification

- Public diagnostics, detail admission and HTTP publication: 0/7 before, 7/7 after.
- Original LumaLabs/Suno/Udio protocol tests: 29/29, 22/22 and 24/24.
- Unchanged upstream response caller tests: 30/30, 52/52 and 40/40.
- Fresh offline, locked all-targets compilation: passed.
- Scoped official formatting: passed; checker tests: 19/19; ratchet: passed.
- Source preservation, UTF-8 without BOM, whitespace and both Git diff checks:
  passed.

Global formatting still reports only the two S06 runtime-mirror files; their
source hashes are unchanged from the preceding media extraction inventory.
Strict scans 1,455 files: 31 hard, 47 mandatory and 40 soft, with 78 above 700.
Structural clearance stays at 67/145 (46.2%). No baseline or exception changes.

Immutable evidence:
`target/effective-line-evidence/20260912-media-worker-errors/scope.json`.
The directory also retains original source/test copies, the unchanged regression
snapshot, paired logs, caller logs, all-targets output, formatter results and the
checker/ratchet/strict inventories. Capture time: 2026-09-11 21:04:10 UTC.

The pre-documentation capture records Gateway HEAD
`4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d`, with 1,569 status entries:
159 modified, one unstaged deletion, two staged deletions and 1,407 untracked.
Neuro HEAD remains `bf818f0324024634bc890585efb78cc8e603d11a`, with 192 entries:
10 modified and 182 untracked. Inherited changes and staged deletions are retained.

## Limits and continuation

This bounds diagnostic processing after worker output reaches these constructors.
It does not bound the original child-process output allocation or parsing of
successful worker output. Explicit `worker_error.message` overrides in upstream
response helpers and Suno's HTTP JSON preview remain separate diagnostic
boundaries. They must not be described as covered by these seven tests.

The preceding structural snapshots remain immutable; their original worker-body
comparison is expected to differ at the 20 deliberately hardened constructors.
No real provider, browser, packaged runtime or release was exercised. All scoped
native gates are terminal. S06 retains its implementation and cursor; the
GWP-20260908-06 source/docs freeze and shared release-build transfer still need an
explicit receipt. No new release was built.

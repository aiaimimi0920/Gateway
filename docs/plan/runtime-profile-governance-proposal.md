# Runtime-profile source ownership proposal

Status: applied and verified, 2026-09-25. The user replied `继续推进` after the
precise five-point S20 migration proposal and approval question. That continuation
authorizes the migration described below. Active checker version 2 passes strict
while retaining all fourteen authenticated runtime assets in the measured report.
See the [active migration receipt](../status/2026-09-25-runtime-profile-governance.md).

The controlling requirement is section 7.1 of
[the main plan](2026-09-03-gateway-effective-line-refactor.md): source evidence,
no hand editing, checker regressions and explicit policy/baseline migration
approval. The license observations below are preserved. This proposal concerns
local measurement; it grants no redistribution rights for any complete bundle.

## Historical evidence, 2026-09-12

Read-only capture:
`target/effective-line-evidence/20260912-runtime-provenance/audit-v2.json`.
The capture confirms all twelve paths are Git-ignored runtime files and still
match the strict inventory's six paired source hashes and effective-line counts.
No browser profile was edited, deleted, merged or excluded. Evidence contains only
static asset identity, hashes, license observations and selected manifest fields;
it does not contain browser credentials or session state.

The observed packages occur separately under both
`deploy/gateway_data/browser-profiles/suno` and `.../udio`:

- `WasmTtsEngine/20260806.1/bindings_main.js`: 4,986 effective lines per copy. The
  local manifest names WASM TTS Engine, version 20260806.1. A local LICENSE was not
  found. Content-verification metadata is present; authenticity was not checked.
- `Default/Extensions/gcalenpjmijncebpfijmoaglllgpjagf/5.6.6239_0`: the local
  manifest and English message key identify Tampermonkey BETA, version 5.6.6239.
  The four oversized members are `vendor/eslint/eslint.js` (2,663), `background.js`
  (1,121), `extension.js` (968) and `editor.js` (821). Root LICENSE says
  `Copyright 2018 Jan Biniok. All rights reserved.` The separate ESLint LICENSE
  contains the MIT permission notice and JS Foundation copyright. Preserve both
  scopes; the vendor notice does not establish a license for the outer extension.
- `Default/Extensions/nmmhkkegccagdldgiimedpiccmgmieda/1.0.0.6_0/craw_background.js`:
  1,190 effective lines per copy. Case-insensitive resolution of `APP_NAME` to the
  local English `app_name` key yields Chrome Web Store Payments, version 1.0.0.6.
  Earlier case-sensitive lookups returned null. Its header attributes Closure
  Library code and declares `SPDX-License-Identifier: Apache-2.0`; that header
  alone does not establish the license for the complete bundle. No root LICENSE
  was found. Both content-verification and computed-hash metadata are present.

Metadata presence and matching local copies establish local consistency only.
Download provenance, signature authenticity and complete applicable licensing
remain unresolved. The v2 audit corrects the initial phrase-only license-marker
detector to recognize the actual SPDX header; the initial audit is retained and
its hash is recorded in v2.

## Initial transition sketch, 2026-09-12

Keep these payloads immutable. Upgrades should replace the owning browser package
through its installer/update mechanism and record a new provenance review. Never
hand-edit a signed extension or generated engine binding to reduce source debt.

A future checker change should admit only individually named, versioned files
with reviewed source hashes and provenance/license references. A matching entry
may be classified as an immutable third-party runtime artifact. Hash drift, a new
version, an unlisted sibling or an unresolved entry must remain visible to strict
measurement. Preserve first-party Gateway source and every unrelated runtime path
in their existing measurement classes. Do not add a blanket browser-profile or
extension-directory exclusion.

Required checker regressions before migration cover exact path/hash acceptance,
byte drift, sibling files, version changes, first-party paths, traversal/absolute
paths, missing evidence and malformed records. These are proposed checks; no new
checker test or production checker change has been implemented in this checkpoint.

After those facts and tests are ready, request explicit approval for the precise
policy/schema change and any baseline provenance update. Preserve the current
baseline and pre-migration inventory as immutable evidence. Count an approved
classification separately from code extraction; no files or profile state need
to be removed to change a measurement classification.

Historical disposition: retain all twelve entries. The three existing policy
`excludedPaths`, dependency files, baseline and exceptions remain unchanged.

## Verified source identity, 2026-09-24

The [provenance receipt](runtime-profile-provenance-2026-09-24.json) records a
successful RSA/SHA-256 Webstore signature audit and exact 4,096-byte SHA-256 tree
hashes for all fourteen measured assets across six installed package copies.
The trust anchor and verification algorithm were checked against Chromium commit
`edaa5af80a511ba8bcb75c923c998910e7e839e7`. Modified signed payloads and file bytes
were rejected. Tampermonkey's root and ESLint license notices also matched their
signed records. Component manifests without a `key` are recorded as such; the
signed item ID/version remains authenticated.

The audit authenticates listed bytes and signed package identity. Human publisher
identity, download history and rights to redistribute the complete packages are
not established. The historical license facts remain applicable, including the
restricted Tampermonkey root notice and missing complete-package notices.

The [exact artifact registry](runtime-profile-artifacts-2026-09-24.json) lists every
path, version, item ID, byte count and raw SHA-256 individually. Its SHA-256 is
`9d898f36126d72289b729752b404787202f93f8f77cf6af6790da4dda43d68b0`.
It covers the six file families listed above plus both copies of Payments
`craw_window.js` at 551 effective lines. No profile directory is excluded.

## Approved migration

1. Add `immutableRuntimeArtifacts: { path, sha256 }` to the policy, referring to
   the exact registry above. The registry binds the provenance receipt by hash.
   Existing exclusion lists and the 150/500/700/1500 thresholds stay unchanged.
2. Add the 127-effective-line `effective-code-lines-runtime-artifacts.mjs` owner.
   It checks canonical paths, exact installed package/version ownership, bounded
   receipt loading, receipt/source hashes, required audit evidence and duplicate
   records. It permits only individually listed JavaScript paths beneath the
   explicit Suno/Udio installed-package layout. Evidence symlinks/junctions,
   traversal, first-party paths, malformed records and missing evidence fail closed.
3. Update the checker to version 2 (423 effective lines). It continues measuring
   and reporting all files, including the fourteen classified payloads; it reports
   their classification separately from governed source. A registered byte drift
   emits a failing diagnostic and keeps the source row. New versions and unlisted
   siblings retain the normal strict/ratchet rules, even for identical bytes.
   Optional runtime assets may be absent from a clean CI checkout.
4. Preserve all 182 adoption-baseline records, their ordered file-list digest,
   source commit/tree and source kind. Update only checker version and tool/policy
   integrity metadata, including a hash of the new classifier. Update the empty
   exception document's tool/policy metadata without creating approvals or waivers.
   Do not invoke `--write-baseline` or create a new adoption snapshot.
5. Keep the original 19 checker scenarios, isolate their temporary scan policies
   from local runtime metadata, add 12 admission-boundary regressions and include
   both suites in `test:effective-lines`. No dependency or lockfile change is needed.

The reviewed candidate implementation/configuration files are under
`target/effective-line-evidence/20260924-integration-closure/governance-candidate/`.
All candidate source and test files are at most 423 effective lines. The nine
governance files were applied exactly; the lexer and fourteen payload hashes are
unchanged. The active policy now binds the two JSON documents linked above.

## Candidate proof and acceptance boundary

Evidence:
`target/effective-line-evidence/20260924-integration-closure/governance-proof-2026-09-24T12-40-15-872Z/`.

The candidate passed all 31 checker tests with zero skips, syntax and UTF-8/no-BOM
checks, and a strict preview over the current checkout. The complete measured
inventory is identical: 2,482 files, four above 1,500, eight at 701-1,500 and two
at 501-700. All fourteen remain in the report. Only their approved classification
changes the evaluation; the 2,468 remaining source files have no entry above 500.
The candidate's source files are measured separately because its directory is
ignored evidence. All 182 baseline records and nineteen protected input hashes
are unchanged.

At this preview checkpoint the candidate exited 0 and the active checker exited 1.
Approval and active migration were still pending. The 2026-09-25 receipt below
records the subsequent official checker suites, ratchet/strict, conservation of
inventory and baseline records, authorization and final hashes.
This classification is not a code extraction, runtime repair or release acceptance.

Current-tree refresh at 2026-09-24 17:48 UTC:
`target/effective-line-evidence/20260924-integration-closure/governance-refresh-2026-09-24T17-48-57-157Z/`.
The official candidate test entrypoint passes 31/31. Active strict exits 1 and
candidate strict exits 0 over 2,485 files (four hard, eight mandatory, two soft).
The refresh compares every measured row, including all files below the report's
500-line display threshold; paths, languages, effective/physical counts and
normalized hashes are identical after removing only the classification field.
The full-row digest is
`f2a02294f67ba19609c640f0c53b9790fdb4da4461604f5acaa50b30496a1cae`.
All fourteen registered assets remain visible, and the other 2,471 source files
are at most 500 effective lines. All 182 adoption records and their original
provenance fields are conserved; 2,502 source/runtime inputs remain exact.
This was refreshed preview evidence before approval and active migration.

## Active migration, 2026-09-25

Evidence is under
`target/effective-line-evidence/20260924-integration-closure/s20-migration-20260925/`.
The official entrypoint passes 31/31 checker tests, with zero skipped tests.
Ratchet and strict both exit 0 with no diagnostics, violations or warnings.
The scan includes 2,497 files: fourteen classified assets and 2,483 governed source
files, all at most 500 effective lines. Three new governance source/test files
account for the inventory growth. Every non-governance measurement and all 2,494
protected input hashes are conserved. All 182 ordered adoption records, their
original digest and source commit/tree/kind remain exact; the exception list is
still empty. The original governance files and inventory are retained for review
and rollback. No adoption snapshot was regenerated.

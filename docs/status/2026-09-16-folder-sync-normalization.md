# Folder synchronization normalization ownership, 2026-09-16

S09 now has a complete import-normalization ownership tree. The legacy folder-sync
root decreases from 5366 to 3066 effective lines, a reduction of 2300. Its 27 original
classification, metadata, dispatch, provider and reader functions move into 13
owners, each below 500 effective lines. Fresh library tests pass 72/72, retaining
all 78 original identities/results, including six ignored tests. The full root
migration and optimization/release plan remain open.

## Ownership and preserved contracts

The files under src/provider_credential_folder_sync have these measured sizes:

| Owner | Effective lines |
| --- | ---: |
| classification.rs | 341 |
| payload_metadata.rs | 109 |
| normalization.rs | 94 |
| normalization/codex.rs | 102 |
| normalization/chatgpt_web.rs | 368 |
| normalization/qwen_web.rs | 225 |
| normalization/chataibot.rs | 147 |
| normalization/gemini_web.rs | 143 |
| normalization/accio.rs | 289 |
| normalization/gemini_business.rs | 81 |
| normalization/gemini_canvas.rs | 403 |
| normalization/chatgpt_auth.rs | 35 |
| normalization/source.rs | 72 |

Classification owns provider-family/service/surface inference. payload_metadata
serves both import and export, preserving material-kind inference and existing-key
insertion semantics. The dispatcher retains all branches, aliases and hint ordering.
Each provider owns its complete existing normalizer; Accio also owns its account
cookie parser. ChatGPT/Codex login readers and JWT metadata decoding have a shared
owner. Source readers own cookie/field access and bounded rawSource traversal.
Dependencies point to those owners without calling back into root orchestration.

Exact field/alias precedence, Chinese error text/kinds, passthrough returns, Accio's
leading compile guard, UUID creation sites, authSeed, OAuth metadata, cookies and
borrowed/owned reader lifetimes remain unchanged. JWT decoding still only reads
metadata and does not authenticate claims. Canvas share/provenance fallbacks and
program-owned API-key filtering remain exact. rawSource traversal stays bounded
to 32 steps. No public API, payload schema, validation, authentication, filesystem,
database, HTTP/Redis protocol or runtime behavior change is introduced.

The root retains its complete original test module byte-for-byte. Whole-source
projection checks the exact parent after extraction and minimal wiring, and all
13 owners after narrow visibility changes and official rustfmt. Non-moved function
bodies are separately checked. Test-only Gemini constants remain available in the
root; obsolete implementation imports move to the owners that consume them.

The 13 owners total 2409 effective lines. Combined root/owner size is 5475, up 109
from the original 5366 due to module/import wiring and formatter reflow. Every new
owner is below 500 and needs no exception. The root remains legacy debt; this
checkpoint does not claim it is fully migrated. No latency/allocation improvement
is inferred from this structural change.

## Verification and evidence

Before editing, the accepted canonicalization scope/publication, captured sources,
tests, assets, Git HEAD/status and library receipt/log/snapshot were hash-revalidated.
That successful unchanged 72-pass/six-ignored result serves as the before gate.
The candidate runs the same command freshly:

    cargo test --offline --locked --lib folder_sync -- --test-threads=1

Result: 72 passed, zero failed, six ignored, 3038 filtered, finished in 0.16 seconds.
The verifier compares all 78 exact emitted identities/results to the predecessor.
No test was added, removed, relocated or weakened for this extraction.

Fresh serialized gates also pass:

    cargo check --offline --locked --all-targets
    rustfmt --edition 2021 --config skip_children=true --check <all 14 scoped files>
    node --test scripts/tests/effective-code-lines.test.mjs
    node scripts/effective-code-lines.mjs --mode ratchet

The exact rustfmt argument list is retained in scoped-fmt.json. Checker tests pass
19/19. Whole-source projection, UTF-8/no-BOM checks, and Gateway/Neuro staged and
unstaged git diff --check pass. Source/test union 1843 includes 1829 unchanged
neighbors; all 22 captured web/Tauri assets remain unchanged. Existing all-targets
warnings remain: three library warnings and one lib-test warning.

Strict exits 1 for remaining debt: 2138 scanned / 12 hard / 20 mandatory / 39 soft.
There are still 32 files above 700; the other 31 retain exact hashes and counts.
Clearance remains 113/145 (77.9%). The strict report lists only files above 500;
compliant owners are measured with its authoritative lexer. Dependencies, checker,
policy, baseline and exceptions are unchanged.

Four independent read-only reviews found no source extraction defect. The evidence
review's preparation-order concern was checked against the actual two-phase flow:
snapshot.mjs before creates before.json, then prepare.mjs requires that baseline
and the absence of the distinct projection.json. This ordering is consistent and
was followed successfully. reviews.md records the commands and all dispositions.

After all-targets passed, the native admission guard observed transient Cargo/Rust
processes and stopped before checker-tests. A fresh observation was idle; the runner
resumed at checker-tests after validating prior receipts. No process was killed,
and no successful native gate was replayed. The transient processes exited before
workspace attribution, so no project ownership is claimed. recovery.md preserves
this boundary and an unrelated optional PowerShell observation quoting error.
The final native guard is empty. Source/tests/assets stayed frozen throughout.

Evidence: target/effective-line-evidence/20260916-folder-sync-normalization/.
scope.json observedAt: 2026-09-15T22:31:57.086Z. Scope SHA-256:
0a576282d0d7ef8a476224630f3671ef751abfe608229ba3f3e9c66adfe80257.
Captured inputs, scripts, reviews, recovery observations and gate receipts are
hash-bound. publication.json verifies these five final documents and exact Git deltas.

Gateway retains 194 modified, one unstaged deletion, two staged deletions and 2370
untracked entries, including this lane's 13 source files and two documents. Neuro
retains 10 modified and 182 untracked. HEADs remain
4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d and
bf818f0324024634bc890585efb78cc8e603d11a respectively. No staging, commit, dependency,
profile, persistent-service or release change occurred.

## Remaining work

Business and generic fallback still lack dedicated successful-input tests; Codex
normalization is covered through chatgpt-codex-backend rather than a dedicated
canonical codex-surface case. Exact function preservation and library success do
not establish every provider input combination or successful database/runtime use.
Existing input-size/allocation behavior and credential handling remain unchanged.

The [canonicalization checkpoint](2026-09-16-folder-sync-canonicalization.md) was
accepted before this lane began. The earlier disable-epoch Redis runtime 10/10
remains historical evidence; Redis/Docker/PostgreSQL/provider runtime tests were
not rerun for this structural move. No new provider or release success is claimed.

Endpoint Git audits prove the captured final status sets and before/after input
hashes, without claiming to observe hypothetical transient external file changes.
I/O deadlines, shared status priority, backend shutdown, successful database sync,
remaining production/test ownership in the root and full provider/runtime/UI/
Docker/release acceptance remain open. S06 source/cursor/final build stays reserved
under GWP-20260912-01. Persistent target stays 4200; release root stays
Neuro/release/Gateway. The overall optimization goal remains active.

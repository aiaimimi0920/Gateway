# Routing configuration ownership acceptance

Accepted at 2026-09-13 19:13:39.649 UTC (2026-09-14 local time). The overall
Gateway optimization and release plan remains open.

The routing configuration entry decreases from 3,984 to 166 effective lines.
All 24 source/test owners are at most 323. Private modules separate schema,
substitution, payload/provider/document compilation, aliases, candidates, account
inventory/constraints, snapshot publication/querying, model discovery, refresh
lifecycle and store operations. Eight test modules retain the original domains.

Exact source proof preserves 75 production items, 68 tests and three fixtures,
21 public root paths, eleven crate root paths, all 50 raw YAML literals and the
original private state fields. Eighteen private free helpers and two associated
constructors gain only pub(super). Shared state remains private at the parent;
all function, impl and test bodies remain exact after official formatting.
The 599 neighboring inputs and published web assets are unchanged.

Review covered credential boundaries, environment substitution, disabled account
admission, constrained round-robin, alias precedence, atomic snapshot publication,
refresh retirement/generation checks, cache ownership and raw YAML preservation.
An observation about initial versus refresh-time OAuth deadline handling did not
demonstrate a new bug; the distinct existing behaviors are preserved.

Paired route-config, credential-admission and hot-reload tests pass 68/68, 6/6
and 13/13. Exact test identities are preserved with a recorded nested-module
mapping. Warning sets match. The exploratory token_refresh:: baseline selected
zero tests and is explicitly excluded from coverage; four config lifecycle tests
and hot-reload contracts cover the relevant refresh behavior.

The first final gates passed, but acceptance rejected two new unused reexports.
The unchanged normalize_model_name and safe_refresh_deadline functions now stay
at their original parent owner. Only the parent and those two former owners
changed; no suppression or forwarding wrapper was added. The original candidate,
projection, successful gates and rejection receipt remain immutable. Acceptance
uses candidate2/projection2 and the fresh, separately named attempt2 gate set.

Locked offline all-targets, scoped rustfmt, source/encoding proof, checker 19/19,
ratchet and separate Gateway/Neuro diff checks pass. Compiler/formatter gates
are serialized, per-process GATEWAY_PREBUILT_WEB_UI=1 is recorded, and owned
native handles are terminal. No external process was terminated.

Strict scans 1,807 files: 18 hard, 25 mandatory, 40 soft; 43 remain above 700.
Clearance is 102/145 (70.3%). Strict exits 1. Global formatting exits 1 only for
src/upstream/gemini_canvas_runtime_mirror.rs and its unchanged test sibling;
both remain reserved to S06.

Immutable evidence: target/effective-line-evidence/20260914-routing-config-owners/scope.json.
SHA-256: 741762daf7875bf036d48aea40481a7ecfc0ac638bd21227959d8f4ab6453665.
Acceptance verifies the 623-input union, saved/current hashes, predecessor and
amended-projection chain, tests/warnings, serialized timing, assets and Git state.

At acceptance Gateway HEAD is 4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d:
2,017 status entries (182 modified, one unstaged deletion, two staged deletions,
1,832 untracked). Neuro HEAD is bf818f0324024634bc890585efb78cc8e603d11a:
192 entries (ten modified, 182 untracked). Documentation publication follows
this census; inherited edits remain preserved.

Pipeline route-stage ownership is the next independent S15 boundary. S06
ownership/cursor and final shared-build transfer still require an actual receipt
under GWP-20260912-01. Runtime-profile governance, remaining strict debt, full
language/provider/release gates, immutable packaging and packaged runtime/UI/Docker
acceptance remain open. No release or persistent service changed; the final
runtime target remains persistent 4200 and no persistent 4226.

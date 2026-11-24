---
name: gateway-productization
description: Productize the Neuro Gateway across portable delivery, provider evidence, and enterprise runtime operations while preserving the canonical pipeline and development credentials.
---

# Gateway Productization

## Required Boundaries

- Work only under `C:\Users\Public\nas_home\AI\GameEditor\Neuro\Gateway`.
- Write releases only under `C:\Users\Public\nas_home\AI\GameEditor\Neuro\release\Gateway`.
- Never alter the existing uncommitted media route edits or unrelated subprojects.
- Do not remove, rotate, redact, variableize, or migrate development secrets requested by the owner.

## Delivery Order

1. Package and path contract.
2. Desktop profile, dependency preflight, and lifecycle.
3. Isolated packaged runtime E2E.
4. Provider inventory, evidence, and canaries.
5. Observability, reliability, access isolation, recovery, and final release.

## Test Discipline

Daily functional work and provider integrations follow the devtest and speed rules
in repository `AGENTS.md` and `docs/devtest-build.md`: focused checks, batched
implementation, and single-link test execution — never a full release gate or
three-pass Rust re-link on every small edit. The full validation steps apply only
to explicitly requested formal releases. Never relabel devtest artifacts as
release artifacts.

For routine feature or provider implementation:

1. Read sibling reference modules in a single batched `inspect_local_file(files=[...])`
   call, and **explicitly fan out 3-4 parallel read-only `default` subagents**
   (`spawn_agent` with `fork_turns = "none"`) whenever a provider card spans 4+ modules,
   wire protocols, DB/pipeline layers, or external SDK specs. Reuse `crates/gateway-provider-kit`
   and frontend `declarativeProviderSpec.ts`, and plan compliant file splits (150-500 effective lines)
   upfront before writing code.
2. Implement the **entire provider card's remaining non-account slices** (auth, protocol
   request/stream, multimodal/reasoning, lifecycle/DB ownership, console fields, and unit tests)
   in coordinated multi-file `apply_patch` batches — **never** end a `/goal` turn (`task_complete`),
   rewrite closure ledgers, or run milestone gates after each 100-line sub-bullet, and avoid
   slow single-edit FastCtx `replace` or PowerShell file-splicing scripts.
3. Validate fast inner-loop gates first (`validate-gateway-line-manifests.py`,
   `npm run check:rust:fast --prefix scripts` for isolated `+nightly -Z threads=6` typechecking
   when checking intermediate edits without running tests yet, `npm --prefix apps/desktop run typecheck`,
   and `npm run check:effective-lines:fast --prefix scripts`). Skip `cargo check` when the very next
   step already runs `cargo nextest`.
4. Run focused tests once the card batch is written via `npm run test:rust:fast --prefix scripts -- "<expr>"`
   or process-isolated `cargo nextest`:
   `cargo nextest run --locked --offline --lib --no-default-features --features runtime-defaults -E "test(<filter>)"`
   (use `npm run test:rust:retest --prefix scripts -- "<substring>"` to re-run the already-built test binary
   in ~0.1s when no Rust source changed, or `-p gateway-core` / `-p gateway-provider-kit` inside extracted crates).
   Never pass inconsistent `--config ...debug=...` flags or flip-flop between `--features line-<provider>` and `--features runtime-defaults` in the shared `target/` (run any isolated single-line `--features line-<provider>` check once at card milestone completion).
5. Run packaging (`tools/package-gateway-devtest.ps1`, using `-Edition api` for backend-only
   validation and `-Edition desktop` only when UI changed) and final authoritative
   `npm run check:effective-lines --prefix scripts` **strictly once after 100% of the provider card's
   code slices are complete** — never mid-card before remaining protocol/DB slices are implemented,
   and run `npm run test:effective-lines --prefix scripts` only when `scripts/effective-code-lines*` changed.
   Never create disposable cold `CARGO_TARGET_DIR` directories under `linshi/`.

Do not make live upstream calls part of default CI. Use explicit opt-in flags and persist classified evidence.

## Preferred Commands

```powershell
python tools/validate-gateway-line-manifests.py
npm run check:rust:fast --prefix scripts
npm run test:rust:fast --prefix scripts -- "test(<filter>)"
npm run test:rust:retest --prefix scripts -- "<test_name_substring>"
npm --prefix apps/desktop run typecheck
npm run check:effective-lines:fast --prefix scripts
# Milestone / provider-card closure only:
npm run check:effective-lines --prefix scripts
```

## Completion Standard

Do not report a task complete based on source inspection alone. Require working
code, passing focused tests, directly affected static/type/effective-line gates,
and honest reporting of any unexecuted package or live-upstream checks. Record
long-form `docs/progress/` receipts and packaged runtime evidence only when the
active plan or user request explicitly asks for milestone/release handoff artifacts.

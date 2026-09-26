# Gateway Repository Instructions

Scope: this independent Gateway repository and all of its subdirectories.

## Active parallel refactor coordination

For the user-authorized 2026-09-08 parallel refactor, read
`docs/plan/parallel-refactor-board.md` before selecting a write scope. The existing
S06 executor retains its Rust/Gemini lane; other agents must not take over that
scope. The coordinator may delegate code changes to default-role agents within
the board's exclusive lanes. This is a task-scoped exception to the earlier
read-only-subagent default, not permission to bypass any validation or safety gate.
Shared build/release operations remain coordinator-owned and serialized.

Gateway owns provider routing, protocols, credentials, management APIs, runtime
workers, desktop console, and Gateway packaging. Do not move Platform, Loom,
Hook, Talk, or Tea implementation into this repository merely to avoid a proper
cross-project contract.

Before development, read the repository README and the owning architecture,
protocol, security, test, and release documents. When this checkout is mounted
inside Neuro, `../docs/DEVELOPMENT_STANDARD.md` is the canonical expanded common
standard. This file remains self-contained for standalone Gateway checkouts.

## Mandatory incremental code-size standard

Effective code lines exclude blank lines, comment-only lines, and comment-only
multiline regions. A line containing code plus an inline comment still counts.
A repository language-aware checker, when present, is authoritative.

- Target about 150 effective lines; 100-250 is the preferred design range.
- 251-500 is acceptable only for one clear responsibility.
- 501-700 is a soft-limit exception. A new or changed file in this range needs
  an exact, unexpired record with independent approval and protecting tests in
  `scripts/effective-code-lines-exceptions.json`.
- A new file or new extraction result at 701-1500 is not complete; split it.
- More than 1500 is a hard violation with no waiver; split again.

The rule covers handwritten production code, tests, scripts, and styles.
Generated or immutable third-party files are excluded only by explicit policy.
Do not game the metric with comments, strings, minification, generated code,
renames, extensions, or dumping grounds named `common`, `utils`, or `helpers`.

The machine-authoritative policy is:

- `scripts/effective-code-lines-policy.json`
- `scripts/effective-code-lines.mjs`
- `scripts/effective-code-lines-lexer.mjs`
- `scripts/effective-code-lines-runtime-artifacts.mjs`
- `scripts/effective-code-lines-baseline.json`
- `scripts/effective-code-lines-exceptions.json`

The normal repository and CI gate is the adoption-baseline `ratchet`. It rejects
new oversized files and any effective-line growth in a baseline file above 700.
`strict` is the migration-closure audit and remains red while governed source debt
is above 700. Exact, hash-bound immutable runtime assets remain measured and are
reported separately under the reviewed policy. Baseline regeneration is a
governance migration, never a way to make an ordinary task green. See
`docs/effective-code-lines.md` for the exact schema,
commands, and exception process.

## Legacy-debt ratchet

- Existing oversized files recorded by the adoption baseline are grandfathered
  debt. Unrelated work does not have to split every historical file before it
  can finish.
- The grandfathering protects existing content, not future growth. New files,
  modules, responsibilities, types/functions, and material extensions are new
  code and must follow the limits.
- Put a new responsibility in a cohesive compliant module and leave only
  minimal wiring in an existing large file.
- A narrow correctness, security, compatibility, or wiring fix may be made in
  place without an unrelated rewrite, but a file above 700 must not increase its
  effective-line count. Report the before/after count and why the fix stayed in
  place.
- A file that was at most 700 effective lines before the task must not cross
  700 because of the task.
- If the task explicitly refactors a large file, every new or fully migrated
  result must be at most 700 lines. Report unrelated debt honestly.

## Required development workflow

1. Measure affected files before design and check for stricter local policy.
2. Preserve behavior with focused tests; split by responsibility, dependency
   direction, state ownership, and resource lifetime rather than arbitrary size.
3. Add concise comments for module purpose and non-obvious invariants, trust
   boundaries, concurrency, cancellation, cleanup, and performance tradeoffs.
4. Review each new or materially changed file for input/auth/secret safety,
   injection and traversal, bounded memory/queues, task/handle/process cleanup,
   blocking work, allocation/copying, concurrency, and algorithmic complexity.
5. Run focused tests, directly dependent compile/typecheck/static checks, the
   official formatter, the checker tests, and the effective-line ratchet:
   `npm run test:effective-lines --prefix scripts` followed by
   `npm run check:effective-lines --prefix scripts`.
6. Run `git diff --check` and inspect this independent repository's scoped diff.
7. For runtime or release work, run Gateway's documented package/runtime gates;
   documentation-only governance changes do not require a fake release build.

Any stricter repository-local checker or CI gate overrides this common floor.

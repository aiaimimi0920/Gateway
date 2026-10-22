# Gateway Repository Instructions

Scope: this independent Gateway repository and all of its subdirectories.

## 日常手测与正式发布规则（2026-10-23）

- 日常修复后的“编译、生成 EXE、出包、release 包”等普通请求，默认使用快速手测包。
  只有用户明确要求“正式发布包、最终交付、正式 Release 流程”时，才执行正式发布流程。
- 快速手测入口：`tools/package-gateway-devtest.ps1`；仅构建使用
  `tools/build-gateway-devtest.ps1`。默认桌面版，`-Edition api|web|desktop` 可选。
- 快速构建使用独立 `devtest` profile：优化等级 1、无 LTO、开启增量；默认最多 4 个
  Cargo 并发任务，可通过 `-Jobs` 调整。不得无理由固定单任务，也不得未经资源评估占满核心。
- 日常仅运行改动相关的定向检查和包完整性检查，不重复全量回归、付费上游测试或正式发布安全审计。
  首次/改动启动流程时补隔离启动验证。编译失败、来源不匹配、哈希错误仍必须阻止交付。
- 依赖保持锁定；不自动升级或删除重装。复用由 Cargo 指纹缓存判断，不凭时间戳拷贝旧 EXE。
  不修改运行实例、凭证或数据库；不自动创建便携 `.ng`。保留旧包，禁止覆盖同版本目录。
- 手测包目录必须以 `gateway-devtest-` 开头；manifest 和构建凭据必须标注 `buildKind=devtest`。
  正式与手测产物目录独立，禁止把手测二进制改名或伪造凭据后当正式 Release 发布。
- 正式发布仍使用 `build-gateway-release.ps1` 和 `package-gateway-release.ps1`，保留其严格检查。
  本节是日常手测的专用流程规则，优先于旧技能中“每次变更都做完整发布验证”的笼统要求。
- 交付必须明确：包类型、路径、实际通过的检查、未执行的验证。首次冷构建和热增量构建
  分开统计，没有实测不得承诺提速倍数。具体命令见 `docs/devtest-build.md`。

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
Otter, Talk, or Tea implementation into this repository merely to avoid a proper
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

## Persistent local data and credential compatibility

- Both EXEs use the shared gateway-local-data owner: an existing executable-adjacent
  .ng directory takes precedence over the current user's home/.ng. Never create a
  portable .ng directory implicitly or derive the choice from the working directory.
- Keep local credentials, state, runtime objects and desktop settings outside versioned
  release payloads. Preserve explicit server/Docker storage settings.
- Prefer backward-compatible readers, optional fields and defaults. Do not bump the
  storage version or rewrite an existing database for an additive application change.
- If a breaking credential/database change is unavoidable, ship its migration code
  inside the same binaries and register it with the startup migration owner. It must
  run before readers/writers start, preserve a recovery copy, be retryable after a
  crash, and reject incompatible concurrent instances or newer storage versions.
- Test old-format reads, secret/ID preservation, repeat startup and interrupted
  migration recovery. Never make end users edit database contents or run manual SQL
  for a normal upgrade. Server database migrations require their own reviewed design.
- See docs/local-data-storage.md. Never publish live .ng contents or credentials.

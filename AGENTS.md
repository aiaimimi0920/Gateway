# Gateway Repository Instructions

Scope: this independent Gateway repository and all of its subdirectories.

## 日常手测与正式发布规则（2026-10-23）

- 日常修复后的“编译、生成 EXE、出包、release 包”等普通请求，默认使用快速手测包。
  只有用户明确要求“正式发布包、最终交付、正式 Release 流程”时，才执行正式发布流程。
- 快速手测入口：`tools/package-gateway-devtest.ps1`；仅构建使用
  `tools/build-gateway-devtest.ps1`。默认桌面版，`-Edition api|web|desktop` 可选。
- 快速构建使用独立 `devtest` profile：优化等级 1、无 LTO、开启增量；默认最多 6 个
  Cargo 并发任务（与全局 `CARGO_BUILD_JOBS=6` 对齐），可通过 `-Jobs` 调整。不得无理由固定单任务，也不得未经资源评估占满核心。
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

## 编译、测试与服务商接入提速纪律

- **子库分层与防巨型单体膨胀**：基础错误、限流、重试、凭据原语已抽离至 `crates/gateway-core`，管理前端嵌入资源已抽离至 `crates/gateway-embedded-ui`（改前端不再触发 `neuro_gateway` 重编），新服务商声明式描述符与有界 SSE 解码器位于 `crates/gateway-provider-kit`（前端配套 `apps/desktop/src/features/console/declarativeProviderSpec.ts`）。接入新服务商优先复用上述子库与声明式配置，避免跨 10+ 目录散落重复样板代码；独立 `tests/*.rs` 契约测试统一归入 `tests/suite_*.rs` 聚合入口，禁止新增零散顶层集成测试二进制。
- **先批量读样板、显式派只读子代理并行预研，再一次性成批 `apply_patch` 落盘**：同一轮并发发出全部独立读文件（`inspect_local_file(files=[...])`）与检索调用（禁止单工具串行往返）；**只要单个复杂服务商涉及 4 个以上模块/协议目录/外部 SDK 文档核对，或跨多个服务商盘点，明确要求同轮并发派发 3–4 个只读 `default` 子代理（`fork_turns = "none"`）分片梳理合同与调用点**；常规单文件/多文件增改一律合并为原生 `apply_patch`（严禁在大脏工作树上对常规局部编辑调用每次耗时 8–17s 的 FastCtx `replace` 或手写 PowerShell `[IO.File]::ReadAllText` 切片脚本），动手前按 150–500 有效行拆好子文件，一次性把整张服务商卡剩余后端、数据库、前端与测试整批写齐。
- **增量编译、快速并行 `check:rust:fast`、`cargo nextest` 与快速行数门禁**：本机已全局启用 `CARGO_INCREMENTAL=1`、`CARGO_BUILD_JOBS=6`、`lld-link`、`debug=0` 及 `[profile.*.build-override] opt-level=3`。多文件分步写码期间仅需静态语法/类型快检时，优先跑 `cargo check -p gateway-core` / `cargo check -p gateway-provider-kit` 或独立目录 6 线程并行前端检查 `npm run check:rust:fast --prefix scripts`（使用 `target/nightly-check` 隔离缓存，绝不污染或击穿主 `target/` 指纹）及缓存门禁 `npm run check:effective-lines:fast --prefix scripts`（约 0.2s）；测试期统一使用 `npm run test:rust:fast --prefix scripts -- "<expr>"` 或 `cargo nextest run --locked --offline --lib --no-default-features --features runtime-defaults -E "test(<filter>)"`（未改 Rust 源码仅重跑过滤词时直接调 `npm run test:rust:retest --prefix scripts -- "<substring>"` 秒级复验；严禁零改动间隔下先跑 `cargo check --tests` 又紧接着跑测试，严禁混用临时 `--config debug=...` 或在主 `target/` 来回翻转 `--features line-<provider>` 与 `runtime-defaults`；单线独立编译隔离验证仅在里程碑收口做 1 次 `cargo check` 探针）。
- **`/goal` 持续模式与整卡单次出包门禁，严禁微切片提前收口/反复打包**：在普通对话及 `/goal` 自动续跑模式下，开发单个服务商的剩余能力（或批量推进 3–4 个新服务商）时，**禁止每完成一个 100–200 行子功能就结束回合（`task_complete`）、重写台账或触发收口门禁**；必须在同一推进周期内将该服务商获准范围内的全部非账号代码切片（如认证、协议流、媒体/推理、持久化生命周期、计费归属与单测）一次性补齐并通过源码级 `nextest` 后，**仅在整张服务商卡片彻底写完的收口里程碑执行 1 次** `tools/package-gateway-devtest.ps1`（纯后端改动用 `-Edition api`，仅前端 UI 变动才用 `-Edition desktop`）、1 次权威 `npm run check:effective-lines --prefix scripts` 及 1 次汇总回执更新；严禁在服务商协议切片未全部写完前中途提前出包或反复点击 WebView2。
- **单写者 `target/` 锁、双开礼让与禁建一次性冷编译目录**：同一检出目录同一时刻只允许一个会话执行主 `target/` Cargo 构建；当本机同时运行其他仓库 `rustc` 或可用内存紧张时自动降并发（如 `--jobs 3`）；严禁在 `linshi/` 下新建一次性空 `CARGO_TARGET_DIR`。后台构建单次设足等待窗口（`exec_command` 传 `yield_time_ms = 30000, login = false`，后续单次 `write_stdin` 传 `60000–120000`）并利用等待窗口并发预读下一批代码，严禁 `1000ms` 短轮询空等。

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
   official formatter (`rustfmt --edition 2021 --config skip_children=true <changed-files>`
   during iteration; `cargo fmt --all --check` at milestone closure), and the
   effective-line gate: use `npm run check:effective-lines:fast --prefix scripts`
   during inner-loop iterations, run the authoritative ratchet
   `npm run check:effective-lines --prefix scripts` once at provider-card or
   milestone closure, and run `npm run test:effective-lines --prefix scripts`
   only when `scripts/effective-code-lines*` (checker, lexer, runtime-artifact
   matcher, policy, baseline, or exceptions) was modified.
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

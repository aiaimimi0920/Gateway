# Gateway 项目现状盘点

日期：2026-09-08，Asia/Shanghai。范围：Gateway 独立仓库，以及用户指定的外部发布根。

## 1. 结论

**Gateway 已经是具备较完整功能面的、能够运行并产出便携包的开发版本；不是只有框架的项目，但也不能宣布工程质量、全部 provider 实机能力和最终发布验收已经完成。**

当前主要剩余工作不是从零搭建网关，而是：

1. 收口正在进行的 Gemini/媒体资源生命周期加固。
2. 修复或更新控制台交互回归测试，并用浏览器验证真实行为。
3. 按现有计划推进剩余大文件拆分，而非再制定一份重复的大重构计划。
4. 更新 provider 分能力验证证据，完成当前源码的全量门禁与产品级验收。
5. 将已验证的开发成果同步到独立 GitHub 仓库，并形成可追溯的新版本。

不提供单一“完成百分比”：功能实现、结构治理、离线测试、实机能力与发布验收是不同维度，不能相互替代。

## 2. 本对话的工作边界

- 后续开发代码集中在 `C:\Users\Public\nas_home\AI\GameEditor\Neuro\Gateway`。
- 每个大任务完成后，构建并验证到 `C:\Users\Public\nas_home\AI\GameEditor\Neuro\release\Gateway\<新版本标识>`。
- 使用新建的不可变版本目录，不覆盖或删除已有版本。
- 独立 Git 仓库分别检查状态；不修改 Platform、Loom、Hook、Talk、Tea 的实现。
- 保留继承的未提交改动、凭据、路由和正在运行的容器。
- 本轮是盘点，只新增本文和运行检查产生的忽略目录报告；没有修改生产源码、运行配置或主计划，没有构建新二进制、部署或调用真实上游。

这些边界与 [Gateway 集成契约](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/INTEGRATION_CONTRACT.md#L24) 一致。独立运行时由 Gateway 管理 provider 和转发；网站用户、公开访问策略及 Platform 计费边界不应搬进 Gateway。

## 3. 已实现的主要内容

下表的“已实现”指当前代码和接口存在，不意味着本轮已对每一条路径完成实机验收。

| 模块 | 已有内容 | 证据与限制 |
| --- | --- | --- |
| 独立 Rust 网关 | Axum/Tokio 服务，Redis、可选 PostgreSQL，standalone/splitter/worker 运行形态；HTTP/WebSocket 服务与管理边界 | [README](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/README.md#L10)、[配置契约](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/INTEGRATION_CONTRACT.md#L119) |
| 对外协议 | Chat Completions、Completions、Responses、Messages、Gemini model actions、Bedrock Converse、Cohere Chat，以及 new-api 兼容路由 | [router.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/http/router.rs#L43)；路由注册不能证明每个 provider 都支持全部协议 |
| 多模态及检索 | embeddings、语音转写/合成、图片生成/编辑、搜索/fetch/research、音乐、视频、Realtime/Gemini Live | [router.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/http/router.rs#L112)、[公开 API 契约](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/INTEGRATION_CONTRACT.md#L53) |
| Provider 接入体系 | provider line manifests、Cargo feature 选择、协议映射、路由配置、凭据与浏览器执行路径；目前有 44 份 manifest | [implementation_lines.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/implementation_lines.rs#L86)、[manifests](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/manifests)；本轮 validator 确认 44 份均合法 |
| 账户和路由管理 | 账户/凭据池、分组、倍率元数据、模型映射、access keys、路由修订、凭据 probe 与 Secret Grant | [README 账户池说明](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/README.md#L308)、[管理路由](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/http/router.rs#L319) |
| 桌面及 Web 控制台 | React/TypeScript 控制台、Tauri 启动器；账户、分组、模型池、访问密钥、运维、状态、日志及配置工作区 | [desktop 源码](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/apps/desktop/src)、[BrowserConsoleApp](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/apps/desktop/src/features/console/BrowserConsoleApp.tsx)；类型检查通过，交互测试仍红 |
| 运维与可靠性接口 | health/readiness/metrics、drain、browser executor 管理、request audits、usage aggregates、异常与 remediation 等管理接口 | [router.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/http/router.rs#L319)、[运维契约](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/INTEGRATION_CONTRACT.md#L81)；不将接口存在当作故障注入通过 |
| 发布与自动化 | 双 Windows 可执行文件、source-tree provenance、manifest/checksums、不可变包、Docker Compose、完整性及 runtime/UI smoke、GitHub CI/Windows/Docker workflows | [发布文档](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/README.md#L145)；本轮实际验证了 3 个历史包的完整性 |

Provider 家族覆盖官方 HTTP/OpenAI-compatible、ChatGPT/Codex、Gemini/AI Studio、搜索服务，以及 Suno/Udio/Luma/Producer 等浏览器或媒体路径。**44 是接入清单数量，不是 44 个当前实机全部可用的平台。**

某些 `unsupported` 是正确的能力边界。例如 fixed-model、browser-backed 或 stateful 凭据无法执行通用无副作用 probe 时，后端显式返回 `unsupported`，不能把这些分支统称为未实现。见 [provider_runtime.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/provider_runtime.rs#L381)。

## 4. 最近已经完成的拆分工作

[现有主计划](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/plan/2026-09-03-gateway-effective-line-refactor.md#L307) 将以下批次标记为 complete：

| 批次 | 已记录完成的范围 |
| --- | --- |
| S00 | 测量、政策、旧债与行为基线 |
| S01 | 桌面 API schema、contract、console 类型叶子 |
| S02 | 账户目录转换、账户卡片及分组工作区边界 |
| S03 | Responses normalize/pack/stream/translation 等协议边界 |
| S04 | Producer、媒体协议、tool injection，以及有界流和 worker 生命周期加固 |
| S05 | Accio、Anthropic、OpenAI、Kiro、FreeBuff 等其他协议族 |

这些是主计划中的已完成批次及其历史验证记录，不是本轮重新运行其全部 Rust、UI 和运行时测试后作出的全绿结论。

## 5. 当前还没有完成的工作

### 5.1 正在进行：S06 Gemini/媒体上游叶子

主计划恢复游标仍是 `S06 / S06-i-g`。最近记录涉及生产图片编码器的 process/workspace 所有权、准入容量、取消、off-thread cleanup，明确没有宣布整个 S06 完成。见 [恢复游标](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/plan/2026-09-03-gateway-effective-line-refactor.md#L12) 和 [最近的局部收口记录](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/plan/2026-09-03-gateway-effective-line-refactor.md#L5462)。

当前源码还已出现独立 reaper 和 timeout/wait-error 故障测试，因此不能简单说“延迟 reap 完全没有实现”。准确状态是：**主计划要求的延迟 reap 故障分支与实际浏览器验收仍缺少本轮确认的收口证据**。见 [encoder_reaper.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/gemini_canvas_encoder_reaper.rs#L43)、[reaper 测试](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/gemini_canvas_encoder_reaper_tests.rs#L119)。

资源清理的明确限制也需要保留：filesystem cleanup 为 best effort；runtime shutdown 可能丢弃排队的 cleanup closure，不能据此声称崩溃清理、secure deletion 或系统调用延迟已有完整保证。

### 5.2 控制台回归测试仍然失败

本轮重新执行 `BrowserConsoleApp.test.tsx`：**49 项，25 通过、24 失败，退出码 1**。

抽样失败包括寻找 `7/30`、`保存路由配置`、`校验草稿` 和账户统计文字失败。它们说明测试契约和当前界面不同步，但仅凭这些报错还不能判定全部都是旧断言，也不能判定所有生产交互坏了。需要逐项确认真实交互，再决定修正实现还是测试，不能直接删掉失败断言。

证据入口：[BrowserConsoleApp.test.tsx](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/apps/desktop/src/features/console/BrowserConsoleApp.test.tsx)。本轮没有重新运行浏览器 E2E 或全量 Vitest；历史 E2E 拦截和 telemetry 404 只能作为排查线索，不能当作今天的新复现。

### 5.3 大文件治理仍有明确旧债

本轮官方 strict 报告：**993 个扫描文件；46 个超过 1500 行，81 个为 701-1500 行，合计 127 个超过 700 有效行；另有 38 个 soft 文件。** strict 退出码 1。

同轮 ratchet 退出码 0：没有触发本次扫描的新增违规/旧债增长门禁，**不等于旧债清零**。soft 文件数量也不等于存在 38 个无效例外；例外合法性由 checker 判断。

主要热点：

| 文件 | 当前有效行 |
| --- | ---: |
| `src/upstream/client.rs` | 15,638 |
| `scripts/gemini-canvas-browser-pool.mjs` | 10,462 |
| `src/protocol/gemini_canvas.rs` | 7,944 |
| `src/db/remediation.rs` | 6,932 |
| `src/provider_credential_folder_sync.rs` | 6,655 |
| `apps/desktop/src/features/console/BrowserConsoleApp.tsx` | 6,139 |
| `apps/desktop/src/styles.css` | 5,545 |

报告：[effective-code-lines-strict.json](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/artifacts/effective-code-lines-strict.json)。该路径是检查工具生成的本地报告，会被未来检查更新。

主计划 S07-S21 仍为 pending，分组如下：

- S07-S10：数据库、访问/remediation、凭据生命周期、Console/HTTP 后端。
- S11-S14：browser worker、桌面工作区、控制台壳/样式、工具和大型测试。
- S15-S19：pipeline、routing/config、Gemini Canvas 协议、browser pool、UpstreamClient。
- S20：残余 strict 清零。
- S21：全量门禁、新 release、打包运行时/UI/Docker 和 4200 栈总体验收。

这里的 pending 主要指**已有实现的结构治理和加固**，不是这些功能尚未开发。见 [批次表](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/plan/2026-09-03-gateway-effective-line-refactor.md#L315)。

### 5.4 Provider 证据需要更新，不能把旧成功当作今天全部可用

- 当前 [provider-inventory.json](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/provider-inventory.json) 生成于 2026-07-29，sourceRevision 为 `657956d`，44 条均为 `metadata_only`。
- 历史 evidence 文件中有 41 条 `fixture_passed` 的离线记录，也有 2026-07-20 的 5 条 `live_passed` 搜索服务记录：Exa、Jina、Linkup、Tavily、You。
- 这说明不能说项目“从未实机验证”，但这些记录不能证明当前工作树、当前凭据或今天的上游协议状态；本轮没有重新核验旧 response artifacts 或发起 live canary。
- Gemini 文本、图片、图片编辑、音乐、视频等必须按能力分开验收。媒体 HTTP 200 但语义仍 pending 不算完成。
- 当前 inventory 与历史 evidence 的汇总没有形成当前源码的验收快照，需要重新生成和验证，而不是把 `metadata_only` 直接改为成功。

状态定义及 live 开关要求见 [provider-evidence.md](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/provider-evidence.md#L183)。

### 5.5 当前源码的全量验证与远端同步尚未收口

- 本地 HEAD：`4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d`，分支 `main`。
- 本轮实时查询 GitHub：远端 main 仍为 `75f306b962e53b548bca5df457c723fb6d7421b6`；本地领先 2 个提交，还有大量未提交/未跟踪内容。
- 普通 `git status --short` 的目录折叠口径约 278 项；本轮一次 `--porcelain=v1 -uall` 快照为 125 个 tracked 改动项、879 个 untracked 文件。两者口径不同，不能直接相减解释为新增改动。
- 最新可见远端 CI 在 `75f306b` 上失败，失败 job 为 Windows product validation 的 `Run python-tests`；同一提交的 Docker 和 Build Windows 成功。
- 这是旧远端提交的结果，不等于当前本地 Python 测试一定仍然失败；也不能用旧 Windows/Docker 成功替代当前 HEAD 的 CI。

在线证据：[CI 失败 job](https://github.com/aiaimimi0920/Gateway/actions/runs/30372884152/job/90321032068)、[Docker 成功](https://github.com/aiaimimi0920/Gateway/actions/runs/30372884008)、[Build Windows 成功](https://github.com/aiaimimi0920/Gateway/actions/runs/30372883877)。本轮未 fetch/pull/push、暂存或提交任何内容。

## 6. 现有发布与运行状态

用户指定的 [Neuro/release/Gateway](C:/Users/Public/nas_home/AI/GameEditor/Neuro/release/Gateway) 当前有以下 3 个版本。不要与 Gateway 仓库自身的 `release/Gateway` 历史目录混淆。

| 版本目录 | provenance 实际构建时间 UTC | 本轮完整性校验 |
| --- | --- | --- |
| `20260903-bounded-object-storage` | 2026-09-03 02:33:40 | 通过 |
| `20260903-bounded-browser-executor-response` | 2026-09-03 07:16:32 | 通过 |
| `20260903-shared-retry-inputs` | 2026-09-03 12:27:51 | 通过 |

三者都有 `gateway.exe`、`gateway-ui.exe`、manifest、checksums 和 build provenance。本轮对每个目录实际执行官方 `smoke-gateway-packaged-runtime.ps1 -IntegrityOnly`，全部退出码 0。

三者 sourceRevision 同为 `4a7aece`，均声明 `sourceTreeDirty: true`，且 source-tree fingerprint 不同，代表同一 HEAD 下的不同开发快照。**完整性通过证明包内文件与其校验合同一致，不证明它们对应今天的 dirty 工作树或已完成全部运行时验收。**

注意：manifest 中的 `buildTimestamp` 为 2026-08-15，而 provenance 的 `builtAt` 为 2026-09-03。这不是本轮确认的缺陷：打包器有意采用 `SOURCE_DATE_EPOCH` 或 Git commit timestamp 生成确定性时间，实际构建时间另由 provenance 记录。见 [Get-DeterministicBuildTimestamp](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/tools/package-gateway-release.ps1#L333)。

本轮只读运行检查：

- `http://127.0.0.1:4200/healthz`：HTTP 200，`status=ok`。
- `http://127.0.0.1:4200/readyz`：HTTP 200，`status=ready`。
- Docker 中 `gatewaydev0727-gateway-1` 使用 `neuro-gateway-dev:local`，显示 healthy，绑定 `127.0.0.1:4200`；其 PostgreSQL/Redis 也显示 healthy。
- 没有证明这个运行实例的二进制对应当前工作树，也没有测试鉴权、模型清单、真实 provider 调用、浏览器交互、drain 或故障恢复。

## 7. 本轮实际运行的检查

命令均从 Gateway 根目录调用；外部可执行文件通过 RTK 转发，以下列出实际底层命令。

| 检查 | 命令 | 本轮结果 |
| --- | --- | --- |
| 行数 checker 测试 | `npm run test:effective-lines --prefix scripts` | exit 0 |
| 增量行数门禁 | `npm run check:effective-lines --prefix scripts` | exit 0 |
| strict 旧债审计 | `npm run strict:effective-lines --prefix scripts` | exit 1；127 个文件超过 700 |
| Rust 格式 | `cargo fmt --all -- --check` | exit 0 |
| 桌面类型检查 | `npm --prefix apps/desktop run typecheck` | exit 0 |
| Manifest 校验 | `python tools/validate-gateway-line-manifests.py` | exit 0；44 checked，0 errors |
| 已知控制台回归 | `npm --prefix apps/desktop run test -- --run src/features/console/BrowserConsoleApp.test.tsx --reporter=json` | exit 1；25 pass，24 fail |
| 工作树空白检查 | `git diff --check` | exit 0；有既存 LF/CRLF 提示，不等于发现空白错误 |
| 3 个包的完整性 | `powershell -NoProfile -ExecutionPolicy Bypass -File tools/smoke-gateway-packaged-runtime.ps1 -ReleaseDir <表中各版本绝对路径> -IntegrityOnly` | 3 次均 exit 0 |

没有重跑：Cargo 全量编译/测试、完整 Python/Node/Vitest、Tauri 编译、provider line matrix、完整 release-candidate、打包 runtime/UI smoke、Docker 重建或 live canary。此次盘点不宣称这些门禁当前通过，也不自动启动耗时的大任务或与其他开发争用共享构建资源。

## 8. 后续推荐顺序

1. **先确认当前 S06 的写入所有权，再收口其资源生命周期边界。** 工作树已有新 reaper 文件，不从旧交接文本重复实现；补齐当前源码的故障注入、聚焦编译/测试及浏览器证据。
2. **单独处理控制台 24 项失败。** 查明产品行为与旧断言的差异，保留交互覆盖，并补真实浏览器验证；不要将 UI 修改混入媒体生命周期补丁。
3. **继续沿现有 S07-S20 计划逐批拆分与加固。** 每批运行同一组行为证明、formatter、ratchet 和 scoped Git 检查，不要求一个普通局部修复先清偿所有旧债。
4. **每个大任务形成新候选包。** 执行当前任务适用的完整验证，再用 `tools/build-gateway-release.ps1` 和 `tools/package-gateway-release.ps1` 发布到用户指定根；外部根需显式传入 `-ReleaseRoot` 和 `-AllowCustomReleaseRoot`。`-SkipBuild` 只有 provenance 与当前源码及双二进制匹配时才合法。
5. **最终总计划收口执行 S21。** 全量门禁、strict 清零、更新 provider 证据、package/runtime/UI/Docker 验收完成后，才能宣布完整重构及产品验收完成；随后按用户授权同步独立 GitHub 仓库。

本报告是带时间边界的盘点，不是新的实施计划，也不修改现有 S06/S07 状态。源码和忽略目录报告可能在其他开发过程中继续变化，下次动手前应重新核对。

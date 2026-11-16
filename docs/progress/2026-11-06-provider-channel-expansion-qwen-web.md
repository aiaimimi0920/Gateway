# R07：Qwen Web 当前消费端单轮文本子链

日期：2026-11-06。工作区 `C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`，分支 `main`，HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`。大量继承 dirty edits 保留，HEAD 不是本批实现或二进制交付凭据。总目标仍 active；不是 R07 整卡、P50 或最终交付。

## 本批改了什么

- 联网核实官方 Chat 首页及 0.3.36 SPA，固定 18 个来源和 2 个衍生摘录的 SHA256；完整字段、许可使用及限制见 [Qwen Web 合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/provider-qwen-web.md)。精确下载 UTC 未记录，不用 manifest 生成时间冒充下载时间；错误命名的旧下载副本排除。
- typed Qwen Web 使用独立 `web_session` owner，保留旧非 typed 兼容。模板原 `/api/v1` 配置不迁移，实际 wire 固定 origin 的 `/api/v2/chats/new` 与 `/api/v2/chat/completions`。校验发生在 create 前，精确账号 Bearer/Cookie/可选 UA 不被 caller 材料覆盖；无重放、redirect、换号或跨 API/OAuth 池 fallback。
- 沿原三适配器、路由、公共 Chat/Responses、NativeEvents、TrackedStream/audit/取消链实现单 user 纯文本 buffered 和真正增量 SSE。严格 UTF-8/JSON、会话/response identity、明确终态、usage 和有界资源；phase finished、partial EOF、坏尾或晚错误不能算成功，unknown usage 不补零。
- 正常显式 model probe 接精确账号、配置模型/别名、Grant 和原 recording seam；默认、禁用、无 Grant、未知模型不生成。当前 owner 不进入旧 signin seed/profile/browser 恢复，过期明确失败。
- 前端暂存/提交绑定 Qwen 可选原始 UA；后端 Keep 将 Qwen Web 纳入原受保护产品集合，比较目的地/认证/flow/有效 UA/绝对 expiry。保留 Perplexity 原“UA 必需”的语义，未因 Qwen 可选而放宽其他产品。
- 旧 buffered accumulator 最小补终态和晚错误检查，兼容原 JSON、无空行 data 和明确 stop/length。未借机改造旧 history flatten、旧流式 translator 或 S06/Gemini lane。

## 已执行的最终检查

证据根为 `C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-qwen-web-20261106`，每阶段 `.json` 记录命令、实际退出码、选定输入快照与漂移；完整来源最终以本目录的 `audit.json` / `closeout.json` 为准。

| 检查 | 结果与范围 |
| --- | --- |
| 正常 Qwen Web 定向 Rust | `current-final-v2` 47/47；包含当前 sender、解码/准入、公共 pipeline、显式 probe 及适用既有测试 |
| legacy Qwen 相邻协议 | `legacy-final` 16/16；与上一行有重合，不直接相加 |
| 正常永久 integration | `integration-final` 6/6：SecretPatch 2、原三项响应红测 3、typed preparation 1 |
| 正常 Rust 按完整名去重 | 57 项通过，不包含历史/中间重复阶段 |
| 仅关闭 Qwen Web | `line-off-final` 4/4，typed buffered/stream 网络前拒绝及 SecretPatch 仍受保护；不是完整 feature 矩阵 |
| 生产库 / 官方格式 | `check-final` / `fmt-final` 通过；六个最终 Rust 阶段各 3291 项选定输入无漂移，最终审计再比当前文件 |
| 前端 | `ui-adjacent-v2` 实际 5 文件 102/102，`ui-typecheck-v2` 通过；各 959 项选定输入无漂移 |
| 行数与 diff | checker 31/31、ratchet 扫描 4667、governed source >700=0；source/diff/链接结果见最终 audit |

正常与关线合并按完整测试名去重为 58 项通过，其中 25 项新增 Rust 测试；前端新增 8 个用例计入上述 102 项。重复阶段与正常/关线同名用例不重复计数，不以测试数量宣称完整渠道已完成。

公共测试使用生产 `build_router.oneshot`、真实 LocalRuntime SQLite 和随机回环上游 HTTP：Chat/Responses × buffered/SSE × 已报告/未知 usage、模型别名与精确 credential、caller 材料隔离、部分失败与本地 Drop。每次断言仅一条审计，失败/取消不 completed，只有两次上游调用，不走 backup。它不是入站独立 TCP、浏览器到真实管理 TCP 联合流程、独立进程或原生 EXE 验收；取消只证明本地退出，不承诺远端停费。

## 红测与纠正记录

所有失败保留原日志，不改测试掩盖实现问题，不把中间阶段计入最终通过数：

1. `terminal-red`：0 passed / 3 failed，真实回环复现 partial EOF、晚错误、malformed tail 被旧 accumulator 当成功。
2. `current-check-v1`：新代码误用未直接依赖的 `chrono`，编译失败；改为复用现有 `time`，未增加或升级依赖。
3. `probe-red`：显式 Qwen 模型测试真实返回 unsupported；随后接当前 owner。
4. `current-v1`：5 passed / 1 failed，代理记录绝对 URI 而夹具只比路径；修正记录为 path/query，未改变生产路径。`public-v1` 的 3 项已通过，但不充作最终来源。
5. `secret-red-terminal-green`：实际只运行 SecretPatch binary，1 passed / 1 failed；Keep 可改变目标。Cargo 随后停止，阶段名里的 terminal-green **没有执行，不能据名称报通过**。三项 terminal 绿测由最终 integration 补齐。
6. `current-final-v1`：46 passed / 1 failed，未知模型 probe 确实发送并通过；修 Qwen 专属配置模型准入后 47/47，不放宽测试，也不改变其他产品规则。
7. `ui-binding-red`：2 passed / 5 failed，五类 Qwen UA rebind 可提交；修复后通过。相邻复核又用独立红测发现原改法可在绕过 parser 的手工 binding 中放松 Perplexity 必需 UA；已恢复原约束，最终新增反例通过。首次 green 过滤器含不存在的第三文件，最终按实际 `testResults` 的五个文件报告，不冒称该路径已测。
8. 临时行数 helper 首次使用 Windows `C:` 作为 ESM import 而失败，改成 `file:///`。`git diff --no-index --check` 在有差异时退出 1，本批以 0/1 且无 whitespace 输出判定，不将“存在 diff”当成检查失败。

## 源码与产物保护

目前本批 30 项源码/测试文件，19 项新增；最大 497 有效行，新增最大 298，无 baseline/exception 变更。新增生产职责按材料、请求、SSE、transport、关闭实现与管理绑定拆分；测试单独归属。UTF-8 无 BOM、最终精确范围和本地链接的结果由最终 audit 固定。

根 `Gateway/artifacts/effective-code-lines.json` 在 npm gate 前直接备份到本批证据根，实际输出另存，再按字节恢复；不是 `Gateway/scripts/artifacts`。前后 SHA256 为 `dc7e58afefbfaa02f4d257ef099af01e91b8fb61a444b517863b3f816bdb3550`。TS 最后修复后以同一 checker 的无报告模式重验通过，没有覆盖现存报告。

`src/protocol/qwen/web_reverse.rs` 的本批 direct backup 取得偏晚。已移除唯一新增 registration 行重建 original，并与真实 `terminal-red-inputs.json` 中原文件 SHA256 精确匹配；单独保存在 `derived-before`，不伪称原先有直接快照。其他修改源码的直接备份与红测快照逐一比对。详见 `scope.json` 的 provenance note。

未读取真实 `.ng`、seal 或历史凭据，未发实际平台认证/生成/付费请求；未操作 Redis、Docker、在线实例，未提交、推送、部署或交付 devtest / 正式包。Cargo integration 的测试编译不等于新应用整包交付。P34 授权此前已消费并清理，本批没有复用。

## 下一步与停止边界

本批限定子链检查后停止扩展，继续原计划其他仍主要模板级渠道；不不断扩大已验单轮子链替代原始范围。R07 仍缺：当前 signin/MFA/challenge/Cookie/refresh 与受控 seed owner；远端多轮/parent/response ownership、system、工具/local MCP、搜索引用、附件/其他模态；账号模型目录、权益和独立额度；Set-Cookie 持久 owner、平台 TLS/WAF 接受性；完整 stop body、删除/停费/退款语义。不会猜 stop 请求，也绝不用删全部聊天的接口做清理。

FreeBuff 正常逐次消费/unknown claim/完整公共链、Kiro OAuth 新模板/兼容、Qwen OAuth 受限决策、其他 API/OAuth 已声明能力及 ChatAIBot/Luma Web/Suno/Udio 完整接线继续。累计新增模板仍 16，无新增整卡 `offline_ready`；P50 全范围集成/devtest、P60 集中录制、P70 真实逐渠道验收均未完成，不在这批提前索要逐项凭据。

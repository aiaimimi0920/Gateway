# Cursor：远端接入实验与历史执行器边界（2026-11-25）

关联 Magpie #15。本文保留 CLI／ACP 和非官方远端适配器的历史实验。官方接口、4200 账号管理、本地 mock 与真实推理的当前结论统一见[能力矩阵](cursor-responses-tools.md)。

历史 CLI／ACP／SDK 隔离实验脚本与测试不随本次直连实现交付；下文对应路径和命令仅记录
当时本机实验，不能作为干净检出的现行复跑入口。现行工具验收使用能力矩阵所列
`tools/verify-cursor-*.mjs`；运行 Gateway 不依赖这些历史执行器。

当前凭据导入流程单独见 [Cursor 账号凭据导入](cursor-account-import.md)。它将授权结果核验后保存到
现有配置，不再使用退出即丢失材料的短期探针充当导入；不把账号已保存标成远端推理已接通。
下文历次实验的“未导入／未出包”只描述各自当时的状态。

**2026-11-27 纯 Rust 收口更新**：`src/protocol/cursor_remote/` 已由早期内嵌 Node 子进程过渡实现迁移为 **100% 纯 Rust 进程内实现**（`src/protocol/cursor_remote/upstream/`：回环 HTTP `CONNECT` 代理隧道 + `tokio-rustls` 证书校验 ALPN `h2` + `http2` 双向流 + 手写有界 Connect/Protobuf/严格唯一键 JSON 编解码），运行时不再查找或拉起 `node.exe`、不再内嵌 `.mjs` 脚本、不再读取 `GATEWAY_CURSOR_REMOTE_NODE`；同时已移除废弃的 `cursor_acp` 运行时死代码与配置接受分支，完整保留 `cursor-personal` 与 `cursor-local/*` 保留模型的零回落拦截门禁。下文带日期的历史章节保留各自当时的实验事实，不篡改历史记录。

**2026-11-26 11:06（北京时间）历史文本回执：在隔离实例上，经非官方实验适配器的
`codex-cli 0.155.1 → Gateway :4315 /v1/responses → Cursor 远端` 文本链路当次通过。
CLI 实际回复 `CURSOR GATEWAY OK - 2 + 3 = 5`，exact 账号审计为 completed／HTTP 200／1 次尝试。
无重连、无本机 Cursor、无付费回落；没有重新登录。只启用隔离实例的指定账号，两个 4200 实例未动。
具体执行模型及请求级 Free 扣额仍未证明；这不是 Cursor 厂商官方 API 授权或完整编程代理能力。**

## Gateway 远端 Responses 实现与真实验收（2026-11-26）

### 文本阶段实现与启用契约

- `src/protocol/cursor_remote/` 管理请求校验、嵌入协议模块和自有 Node 子进程；专用
  `cursor_remote_identity`／`cursor_remote_dispatch` 对接 Local Key、冻结配置、请求预算及审计。
  运行时不调用独立探针 `run_saved_once.mjs`，也不运行 Cursor desktop／CLI／SDK／Docker。
- 正常启动从 `PATH` 解析本机 Node 的绝对路径（Windows 仅允许本机 `C:`，不搜索当前目录），
  并沿用 Gateway 启动时的出站代理策略：自定义代理，或自动模式的环境／Windows 用户系统代理。
  需要三个固定 Cursor 目标全部匹配同一无认证的 `http://127.0.0.1:<端口>` CONNECT 代理；
  关闭代理、任一目标命中 bypass、其他主机／协议／代理认证均明确拒绝，不静默直连。
  `GATEWAY_CURSOR_REMOTE_NODE` 和 `GATEWAY_CURSOR_REMOTE_PROXY_PORT` 保留为各自独立的显式覆盖，
  无效覆盖不回退；端口没有硬编码默认值。代理设置变更仍按既有规则重启生效。
  本机实测 Node `v22.22.2`，Node 仍是宿主依赖，不随 Windows 包分发。
- 必须通过既有管理配置将一个已核验的 `cursor_remote` 凭据设为 `enabled:true`。
  依赖自动发现不自动启用账号；导入仍创建禁用账号。通用模型目录、账号池、定时探测及普通模型路由仍不能使用该凭据。
  已配置远端服务商缺少依赖时返回 `cursor_remote_node_not_found`／`cursor_remote_proxy_required`
  等明确错误，不误入历史 ACP 路径；未配置远端服务商的保留名拒发契约不变。
- 仅本地存储模式、普通 Local Key；Key 必须明确授权 `responses`（或 `relay`）、
  `cursor-personal` 和 exact provider，并满足账号组限制。请求冻结凭据与修订；
  远端预检及 TLS/H2 准备后、Run 请求头发出前，再核当前 Key／修订／取消／预算。
- 每次执行保留本人身份、个人 Free、按量关闭、无其他账源、服务路由和目录选择检查。
  一次请求最多一个 Run，不自动刷新、重发、换账号／模型或付费回落。
  已确认发送后才记实际账号与 attempt；授权后缺失发送回执标记交付未知，不能伪装零执行。
- 源码与单次材料走私有 stdin，不进入 argv／环境／临时凭据文件；固定嵌入模块不可由请求替换。
  单实例一个 worker 槽；取消、超时和关停回收自有进程树，审计等待回收及发送回执结算。
- 该文本阶段仅 **无工具、纯文本、stateless Responses**。instructions 和角色历史完整序列化为带角色标签
  的 JSON transcript，放进 ASK 文本字段，**不是原生角色优先级或原生多轮状态协议**。
  拒绝工具、媒体、实际 reasoning、`store:true`、`previous_response_id` 及未映射选项。
  SSE 缓冲到合法 TurnEnded 和正常 EndStream 后输出，不是实时 token 流；不伪造 usage 或实际模型。

### 本地与真实回执

- Node 定向 **155/155**；Rust 定向 Nextest **50/50**
  （`1082b1a4-04ea-411a-80f8-17858929b056`）；复用同一测试二进制补相邻回归
  **171 passed／0 failed／8 ignored**，跳过的是旧 Docker opt-in 试验。
  新 HTTP 夹具的 non-Send SQLite future 已按既有 ACP 夹具在线程内构造，不修改生产 SQL。
- `cargo fmt --all --check`、权威有效行数 ratchet、scoped diffcheck 和 58 个相关文件
  UTF-8 无 BOM 检查通过。最后仅调整 `protocol/mod.rs` 模块排序；不是额外语义补丁。
- API **devtest** 包：
  `C:/Users/Public/nas_home/AI/GameEditor/Neuro/release/Gateway/gateway-devtest-cursor-remote-20261126-104351`。
  构建／来源／6255 文件打包及 EXE 哈希检查通过，Jobs=3；不是正式 Release。
  构建指纹 `c9e29805446905e6b31773e0f3ed2a4c7c79c03ec1ff5d11cdfea205472278ee`；
  本节是构建后补写的验收记录，不将文档更新后的工作树冒称为原构建快照。
- 11:06:28 将原 4315 验收实例切换为新 API 后端 PID **53328**，保留原数据根和凭据指纹。
  唯一账号 `cursor-1d963071-76a0-4d4a-a109-9068948b26b4` 显式启用，
  修订 `r2-1664e9a2f8ee → r3-be579f578017`，后台探测仍禁用。旧包保留。
  这是无管理网页的 API 包；原桌面启动器不用于启动新执行器。重启时须显式带上上述运行时环境。
- 长期验收入口 `tools/verify-cursor-codex.mjs` 从进程环境接收管理授权，创建五分钟最小权限
  Local Key，启动真实固定版本 Codex，关闭工具、reasoning 和重试；从 `agent_message` 正文匹配答案，
  再核 exact audit／修订，最后撤销 Key 并独立读回，不读取 Cursor 令牌。
  该入口允许共享实例保留其他服务商／普通模型路由，但目标服务商仍须只有一个启用账号，
  Key 明确限于该服务商及 `cursor-personal`；不为验收清空路由或扩大授权。
  `--http-one-shot` 改为只发一次直接 HTTP 请求（无重试／重定向），此模式不证明 Codex CLI。
  共享实例测试须处于无人并行编辑路由的窗口，发前再次核修订，事后核 exact audit：
  provider-scoped Key 本身不是 credential pin，读回与请求取快照间的竞态并未消除。
- 真实请求 **11:06:43–11:06:56**：
  `gw-audit-255bbf68-c11f-4bca-ab59-4dcdf22ab005`，`endpointKind=responses`，
  `status=completed`，`providerAccountId=cursor-remote`，`upstreamStatus=200`，
  `routeAttemptCount=1`，`realCredentialRef` 为上述账号，`selectedPipelineMode=cursor_remote`。
  CLI exit 0、正文精确匹配、无重连；前后修订相同，临时 Key 已撤销。
- 脱敏证据 `target/provider-evidence/cursor-gateway-codex-49d0d145-c706-42e1-a85e-a7a3fed6a4d0.json`。
  切换及收尾读回位于本机 `linshi/cursor-remote-gateway-20261126/`；
  汇总为 `target/provider-evidence/cursor-gateway-20261126-runtime-validation.json`。
  收尾 Cursor／cursor-agent 进程 0、Gateway 协议 worker 0；正常宿主 `conhost.exe` 不算协议 worker。
  loopback 4200 PID **40484**、LAN 4200 PID **11216** 均未变化。
- 验收只证明上述文本链路；`actualModel=null`、`freeAttribution=not_proven_by_protocol` 保留。
  不把账户级 Free 检查当请求级扣额证明，不声称工具、媒体、存储续轮、生产 4200 或官方授权已验收。

## 当前验收范围：运行时不依赖本地 Cursor

- 目标是客户端（包括 `codex-cli`）只访问 Gateway，由 Gateway 使用导入账号获准的凭据直接访问 Cursor 远端。生产路径不能启动或依赖 Cursor 桌面、`cursor-agent`、SDK Bridge 或 Cursor 容器，包括按请求临时启动。
- 下文的 ACP／Docker 内容为历史实验，不再是生产实施顺序。容器是在验证真实 CLI 作为执行器的隔离和回收，而不只是核对字段；这条路线不满足当前约束。保持既有 ACP 槽未启用，不继续接入登录、容器或中继。
- 当前缺口是同时满足 **Gateway 直连、本人 Free 额度、模型供应商语义** 的上游合同。公开 Cloud Agents API 可远程调用，但 [官方套餐表](https://cursor.com/pricing) 不给 Free 提供 Cloud Agents，且 [官方 API 总览](https://cursor.com/docs/api) 明确该 API 运行 Agent 工作流，不是独立模型推理接口。Free 额度的现有实测仅对应 CLI／ACP，不可移作远端 API 的计费证据。
- 社区有 Oh My Pi 这类直连候选，但 [Cursor 支持已明确指出](https://forum.cursor.com/t/does-using-oh-my-pi-s-cursor-provider-or-an-openai-compatible-proxy-to-the-same-endpoints-violate-cursor-s-tos/167778/5)该接法访问非公开客户端端点、不是获准客户端，个人自用也存在账号封禁风险。用户知悉后明确回复“同意并继续”，已允许本人账号的非官方实验；这不是厂商授权，也不允许付费回落或默认开放生产路由。
- 满足条件的上游可验证后，再按既有账号导入、路由和审计体系接入。`codex-cli` 验收使用 `/v1/responses`；历史 `/v1/chat/completions` 单轮文本合成测试不算通过。真实验收同时核对调用账号、真实回复、免费归属和零本地 Cursor 进程，并禁止付费回落。

### 独立的个人本机 Auto 候选（2026-11-27）

`cursor-local/auto` 是保留名，不是可用模型；`cursor-local/` 也不接受其他模型。它不读取导入账号，不走 `cursor-personal`、ACP、`cursor_remote` 或普通供应商池。没有启动进程的代码、HTTP 启用参数或控制台开关。不要把这个保留名添加为模型、别名或付费备用。

- **之前的真实结果**：操作者在台式机用已登录 Free 的官方 `cursor-agent 2026.08.11`，在空目录以 Ask、Auto、headless 请求固定算术题 `17 × 19`，得到 `323`、success、退出码 0。此为 CLI 单次结果，不是 Gateway 成功，不证明指定下游模型、真实输出增量或逐请求免费。
- **当前入口契约**：已认证的普通文本在 Chat / Responses 返回 `503 cursor_local_auto_disabled`；工具、工具结果、Messages、存储/续轮及其他本机模型不支持。没有 session 创建或恢复，也不能拿旧响应 ID 在其他账号重放。以上拒绝不会进入普通模型路由；无权访问的调用可以先被既有鉴权拒绝。
- **隔离验收**：`cargo nextest run --locked --offline --no-default-features --features runtime-defaults --test suite_routing_and_console -E 'test(cursor_local_auto)'`。两端只监听动态环回端口，使用临时 SQLite 与合成 Local Key。正对照故意使用另一个模型 `offline-smoke`：只回放固定 NDJSON 的末尾 `result`，不拼接可能重复的 `assistant` 事件。Chat 与 Responses 均只在验证完整回执后交付整段文本和结束事件，不宣称 Cursor 实时流式。将 `GATEWAY_TEST_CODEX_EXE` 指向受信的 `codex-cli 0.155.1`，加 `--run-ignored all` 可同时验真实 Codex **客户端**的失败和成功对照，不启动 Cursor。
- **真实首轮为何被拒**：只在测试入口按计数和布尔值观察 `/v1/responses`。本次首轮有顶层 `instructions`，3 个输入项（2 个 `user`，1 个 `developer`），5 个工具定义，无前序 ID、显式用户或会话字段，没有要求存储。旧判断把规范化后有多条消息当作“续轮”；现在明确返回 `400 cursor_local_unsupported`，原因是工具不受支持，没有丢弃客户端指令或工具。无工具的多消息输入单独解释，空 `tools` 加 `tool_choice:"none"` 的单条用户文本仍返回路由关闭的 `503`。
- **回执**：Nextest `724ce834-5ed8-4ae5-83a4-d0400681c625`，5/5 通过。两份临时 SQLite 分组读回：真实 Codex 请求保留模型 1 次，失败、零尝试、无供应商；同一客户端另起一次首轮，`offline-smoke` 仅命中本机 `canary` 1 次，收到 `item.completed` 的 `agent_message` 文本 `323` 和 `turn.completed`，退出 0。该本机测试模型的 Codex 目录缺项提示不是推理错误，也没有换模型。另一份审计为 20 次候选拒绝、均零尝试，4 次普通对照（Chat / Responses × JSON / SSE）均仅命中本机 1 次。无密钥请求另返 401。本轮没有向 Cursor 发请求、读取登录态或验证真实账单。
- **启用前的两个门槛**：[CLI 使用](https://cursor.com/docs/cli/using)说明 Ask 仍会搜索代码，[MCP 文档](https://cursor.com/docs/cli/mcp)说明还会发现父目录及全局配置。[工具权限](https://cursor.com/docs/cli/reference/permissions)不等于已隔离整个进程树；收到工具事件再拒绝也不等于阻止了执行。[套餐与用量](https://cursor.com/docs/models-and-pricing)区分 Free、付费及旧套餐；没有核验此执行身份固定在个人免费工作区、额度耗尽必停且不能扣付费余额。须在**同一隔离执行环境**证明两项，才能考虑派发；宿主上的一次 Free 成功和另一个容器的禁网测试不能拼成这个证明。无需重复网页登录或向 Gateway 粘贴令牌。

**到官方 CLI 的固定题验收（不开放上述保留名）**。Windows 集成测试 `cursor_official_fixed` 在独立环回端口启动 Gateway HTTP 入口和单次测试适配器，使用临时 SQLite 和随机测试密钥。`POST /v1/responses` 仅接受预定义算术题的完整请求；适配器核对 Gateway 转换后的 Chat 数据，向官方 CLI 传硬编码题目，不传入 HTTP 文本、工具或命令。CLI 自行使用已有登录；工作目录为空、配置单独创建，无自动批准。此测试不安装生产适配器，不证明任意提示词的宿主隔离。

- **预检实测**：同一执行环境的官方 `status` 和 `about` 报告同一身份、`authenticated`、`subscriptionTier=Free`。测试适配器随后故意返回 `412`；公共 Responses 同样返回 `412`，SQLite 为 `failed`、1 次到本机适配器的尝试、0 次模型调用。联合回归 `c72809b0-a70d-48e1-b82f-8c1a387f4cbe` **7/7**；此前 `offline-smoke` 的 `323` 仍是模拟，不作为这条链路的真实答案。
- **单次真实请求的门槛**：`Free` 不能单独区分工作区和计费方式。[旧版个人 Hobby](https://cursor.com/docs/account/pricing/legacy-cursor-plans) 不支持超额按需计费；[新版额外用量](https://cursor.com/help/account-and-billing/overages)另有预付余额和自动充值。操作者须核实当前 CLI 账号使用个人旧版 Hobby，或确认使用新版个人 Free、没有可扣预付余额、自动充值关闭且不使用付费团队。未核实不派发，不需要再次登录。
- **复验**：`operator_preflight_without_inference` 只查账号元数据。真实用例 `operator_one_live_fixed_question` 还要求宿主通过离线沙箱检查、人工信任过的 `GATEWAY_CURSOR_FIXED_TRUST_ID`（12 位小写十六进制编号，不接受路径），以及本次单独核实的 `GATEWAY_CURSOR_FIXED_PROBE_BILLING`（`verified-personal-legacy-hobby` 或 `verified-personal-free-no-credit-no-topup`）。只向单次测试子进程提供，不设置持久环境变量。工作区编号在模型启动前原子占用；即使超时或失败也保留，不删除标记重放。成功标准为官方 CLI 在同一 Auto 会话正常退出并给出 `323`、公共 Responses `200/completed` 返回该文本、审计仅 1 次派发。当前 Windows 宿主不能通过沙箱门槛，见下文；不要重跑真实用例。
- **真实尝试（2026-11-27）**：本人确认新版个人 Free、预付为 0、自动充值关闭。同一环境的身份与套餐预检通过，官方 CLI 的固定题命令启动一次，在未交付已验证文本时以 `exit=1` 结束。此次 `stderr_marker=unclassified`，没有可用于确定 CLI 退出原因的错误码；不能据此归咎于登录、额度或沙箱，也不能断言远端收到请求或账单有无变化。测试全程约 11.6 秒，未换模型。隔离 SQLite 为 `responses/failed`，`canary`、最终 `409`、`routeAttemptCount=2`：首次本机适配器返回 `502`，Gateway 重试的是本机适配器，第二次由单次保护拦下，未再运行 CLI。
- **修复后的本机回归**：仅测试适配器将“CLI 未交付可验证结果”作为终态 `424 Failed Dependency`；保留防重放，不改生产重试策略。现在仅输出 `stdout`/`stderr` 的预定义错误类别，不输出原文。本机模拟验证公共入口 `424`、SQLite `failed`、到适配器恰好一次、CLI 零次。Nextest `70003c98-a368-40cb-8747-e60f8ae29fbb` **7/7**，排除两个 `operator_` 用例；没有再次请求 Cursor。仅固定题模型验收失败，通用路由保持关闭。
- **Windows 启动诊断（不使用账号、不发推理）**：同版本 CLI 的 `sandbox run` 使用空工作目录、空身份目录和固定无副作用的 `cmd.exe /d /c exit 0`。去掉 `PATH` 时复现 `Cannot read properties of undefined (reading 'split')`；仅补 `ComSpec` 仍失败，仅把 `PATH` 指向新建空目录就越过这一步。测试启动器据此修正，不继承用户 `PATH`。随后官方 helper 明确返回 `windows_sandbox_no_filesystem`：该 Windows helper 只提供网络代理，不提供 `workspace_readwrite` 文件系统隔离。此诊断不等同于 Ask 文本请求，上次未保留其原始错误，不能倒推两者必然同因。
- **验证范围**：Bundled Node 对 CLI 默认 API 主机的 TLS 验证及 `h2` 协商成功，未发送应用层请求；模拟不可达入口可识别 `api_unreachable`，不据此更改代理、工作区信任或 `--sandbox enabled`。单次固定题入口在账号预检前先运行无身份的本机诊断，已知宿主限制不作为通用能力；其他未知诊断失败则不派发。Nextest `a6ef0ed2-53ed-490c-9795-5c033a89b6be` **9/9**（含真实 Windows helper 的负例、模拟 `424` 和真实 Codex 客户端的本机对照，排除两个 `operator_` 用例）。此轮真实模型请求 **0 次**；真实端到端仍未通过。

- **修复后的单次真实复验（2026-11-27）**：`account_preflight=true`。无身份沙箱自检查出既知的 `windows_sandbox_no_filesystem`；随后针对固定题的官方 `Auto` CLI 只启动一次，`exit=1`、`stderr_marker=workspace_trust`、`stdout_marker=empty`，没有返回 `323`。这是新临时工作目录的信任检查，不是登录失败；没有设置自动信任，也没有换模型或重试。公共 `/v1/responses` 返回 `424`；只读回查本次独立 SQLite 仅一条 `responses/failed`，本机 `canary`、`upstreamStatus=424`、`routeAttemptCount=1`、`durationMs=13289`。`424` 来自本机适配器，不是 Cursor 服务端状态；远端是否收到请求及实际账单均未验证。回执：`linshi/cursor-official-live-af48cfdf-abfa-495b-9b37-1b878bfc2395.log`；隔离状态目录：`linshi/cursor-fixed-proof-e652381a-72e0-4ef6-a5f4-854b98c7ca55`。单次失败不再重放已实测，真实答案验收仍未通过。

**工作区信任的离线对照（2026-11-27，Windows）**。使用上述哈希固定的官方 CLI、两个并列空目录 `candidate` / `sibling`、同一临时配置和空身份目录。保留 `--mode ask --model auto --sandbox enabled`，不传 `--trust`、`--force` 或 `--approve-mcps`。CLI 的 API 目标和代理仅配为环回地址；代理负对照只拒绝连接或固定回复 `503`，不转发、不模拟 Cursor 身份或推理。

| 对照 | 实际观察 |
| --- | --- |
| 无身份，两目录分别使用 `--print` | 两者均在 `Authentication required` 退出；这是空身份目录的预期行为，不是本人账号掉线。 |
| 无身份，`candidate` 打开不带题目的交互终端 | 只看到 `Log in`，未输入按键，终止该测试进程。 |
| 仅给本机负对照一个无效的合成 key | 两目录的 `--print` 均报 `Failed to reach the Cursor API`；交互入口也未出现信任选项。本机拒绝服务收到连接，不代表 Cursor 收到请求。 |

**联网启动、无题目的人工信任已通过（2026-11-27 11:07，上海时间）**。操作者只在 `cursor-trust-5222d8b2421c\candidate` 确认了官方 CLI 的信任提示。使用同一临时配置、既有登录和相同启动参数，分别在两个目录新开进程；两个窗口均读回了各自的完整路径，没有传入题目、发送按键或设置 `--trust`：

- `candidate` 直接进入待输入界面（`→`、`shift+tab` 快捷键），没有 `[a] Trust this workspace`。
- `sibling` 仍显示 `Do you trust` 和 `[a] Trust this workspace`，未确认。复查后两个目录仍为空；两个对照进程和最初的确认窗口均已关闭。脱敏回执：`linshi/cursor-trust-5222d8b2421c/trust-post-confirmation.json`。

结论仅限这两个目录的信任状态。Windows CLI 明确提示 `Falling back to allowlist mode`，不是文件系统沙箱。目录对照本身没有发起推理、查询账单或修改 `4200`。

**复用已确认目录后的单次真实验收（2026-11-27）：未通过。** 验收用例复用了 `5222d8b2421c` 的空 `candidate` 和配置，仍为本次创建独立的 SQLite、环回端口、空 `PATH` 目录与临时目录。实际 CLI 在启动前复核目录、拒绝权限和 Free 身份；固定题只启动一次。`exit=1`、`stderr_marker=sandbox`、`stdout_marker=empty`，没有可验证的 `323`。本机 `/v1/responses` 返回 `424`，SQLite 仅一条 `responses/failed`，测试适配器 `canary`、`routeAttemptCount=1`、`durationMs=8587`。未重试、未换模型，`candidate` 仍为空。此 `424` 不是 Cursor 服务端状态；未核实远端收到请求或账单变动。脱敏回执在 `linshi/cursor-trust-5222d8b2421c/gateway-fixed-once.log` 和 `gateway-fixed-once.audit.json`。

**为何不再用同一条件发题**：在另一个空身份环境，无推理、API 和代理仅指向不可达环回端口，固定版本官方 CLI 的 `sandbox enable` 明确报告 `Sandbox requires macOS or Linux`（退出 1）。这与交互模式的 allowlist 回退、独立 `sandbox run` 不具备文件系统隔离一致；真实 Ask 的原始错误只保留了 `sandbox` 分类，不据此推断更细的服务端原因。验收用例现先在可丢弃配置下检查 `sandbox enable` 和 `sandbox run`，任一失败即在账号访问和模型启动前停止。保护性回归 `bb1f6c11-1361-4faa-b360-f23d6e74b09e` **6/6**，包含本机官方 CLI 的离线拒绝、工作区约束及单次占用，不含再次真实发题。没有关闭沙箱、启用生产路由或修改 `4200`。

**Linux 离线沙箱已实测（2026-11-27，不是 Gateway 推理回执）。** 同版本官方 Linux x64 CLI `2026.08.11-e8db854` 在本机 Ubuntu WSL2、一次性 tmpfs 根和空 HOME 中执行。Linux 启动器以普通用户运行，不使用 Docker 或 sudo；CLI 只见选定的系统文件和经 SHA-256 钉住的公开安装包，不挂宿主项目、账号目录或 WSL 互操作端点。测试前核实 UID/GID `65532`、零 capability、`NoNewPrivs`、只读根、仅 `lo`、无 IPv4 路由，创建 `AF_VSOCK` socket 被拒。探针只运行固定命令，不接受提示词，也不提供联网开关。

| 同一环境的对照 | 实际结果 |
| --- | --- |
| 普通命令写工作区外的合成目录 `/control` | 成功；该目录本身不是只读挂载。 |
| 官方 `sandbox enable`，随后 `sandbox run` 执行固定 `echo` | 两者退出 0；读到独立输出行 `CURSOR_OFFLINE_OK`，不靠命令回显判成功。 |
| 同一 `workspace_readwrite` 策略在工作区内、外各运行一次 `touch` | `/work/allowed` 实际存在；`/control/denied` 返回 `Permission denied`、退出 1，文件不存在。 |

初始 `sandbox enable` 的 AppArmor 提示不是根因：独立 `sandbox run` 指出测试夹具把私有 `/proc/self/uid_map` 挂成了只读。仅调整探针新 PID namespace 的 procfs，非进程控制项仍只读，未更改宿主 sysctl、AppArmor 或 Docker；补入缺少的 `libz.so.1` 和 `cat` 后对照通过。CLI 另有 `dump_bash_state: command not found` 提示，不影响上述文件与输出对照，shell 状态保存未验。用普通 Linux 用户在清空环境下执行 `python3 -IB tools/probe-cursor-linux-sandbox.py <固定官方归档>` 可复核；错误归档实测在启动 CLI 前即拒绝。脱敏回执 `linshi/cursor-linux-sandbox-507f3680b1aa.log`。这不是读取隔离、多轮或模型输出证明；本轮真实推理 0 次。下一门槛是为**同一隔离环境**验证仅到官方服务的出站以及独立官方登录，不能搬用 Windows 登录材料。`4200` 和通用路由未变。

**独立 Linux VM 的受管出站复核（2026-11-27，未登录）。** 普通 Ubuntu WSL 经已知 Windows 代理访问 API 首页可得到 `200`，但那不是上面禁网 namespace 的流量。另行验证了本机 Docker Sandboxes v0.12.0（独立于超时的 Desktop engine）：以 `--pull-template never` 和新的空工作区建立个人测试 VM。在该 VM 内另起无宿主卷、无 Docker socket、只读根、默认 seccomp、零 capability 的非 root worker，仅把同一份公开 CLI 归档和验收脚本送进 tmpfs。没有把 shell 管理容器或其 Docker socket 交给 CLI。

| 同一 worker 的实际观测 | 可得结论 |
| --- | --- |
| VM 默认拒绝时，直接访问两个官方域名均为 `ConnectionRefusedError`；代理先返 CONNECT `200`，两个 TLS 握手均未通过 CLI 自带公共 CA 验证 | CONNECT 状态不是端到端 HTTPS 成功；没有导入代理 CA。 |
| 给该 VM 两个精确的 `:443` 域名配置 TLS 直通后，匿名 `HEAD /`：`api2.cursor.sh` 为 `200`，`cursor.com` 超时 | 已验证一个官方目的地的公共证书 HTTPS，不证明登录或 Free 推理。 |
| 对保留域名的普通 HTTP 返回 `403`；其他 HTTPS authority 和非 443 端口的 CONNECT 首段均为 `200` | HTTP 的拒绝不能证明这些 TLS 连接已被放行或拦截；负对照未全部通过。 |
| 同一 worker 的官方 CLI `sandbox enable`、固定 `sandbox run` 均退出 `1`；诊断含 `user namespace` 和 `Operation not permitted`，无独立命令输出 | 该执行环境不能与 WSL 的沙箱正例合并验收；没有加 capability、更改宿主内核策略或关闭 seccomp。 |

复核脚本 `tools/probe-cursor-managed-egress.py` 只有 `prepare`、`deny`、`allow-official`、`sandbox-only` 四个固定检查，不启动登录或模型。回执 `linshi/cursor-probe-af79129e6feb/{managed-worker.log,other-policy.log,cleanup.json}`。已停止本轮 worker 并发出 VM 停止命令；控制面随后不再公布 VM 的 engine socket，但列表状态仍显示 `running`，不据此声称 VM 已卸载或删除。两组对照未在同一环境通过，结论仅限这条 CLI 实验。独立 Linux 环境不是导入或直连远端的前置条件；不要求用户另备机器或重复登录。未变更 `4200` 或任何账号。

**同机纯文本的官方候选（文档核对，未启用）。** [TypeScript SDK 的工具限制](https://cursor.com/docs/sdk/typescript#restricting-the-toolset)明确区分 `tools: []`（不提供内置工具）和省略 `tools`（默认提供）；恢复会话必须重新传入限制。[Cursor 对 CLI 与 SDK 的说明](https://forum.cursor.com/t/cursor-cli-a-locked-down-headless-mode-file-tools-only-no-hooks-mcp-plugins-enforced-tool-allowlist/173496/14)还区分了 SDK 不加载配置层与 CLI 的默认发现行为。若评估个人本机纯文本 Agent，应优先核实此路径，不引入自定义工具、MCP 或项目配置。不向模型提供工具不等于证明 SDK 本身不读文件，也不是独立模型 API。[Cursor 支持在 2026-08-22 的答复](https://forum.cursor.com/t/agent-create-local-apikey-model-fails-for-free-users-because-local-model-validation-calls-cloud-v1-models/160839/12)称当时 Free 不能通过本地 SDK 的模型验证；尚未证实新版个人 Free 已解除该限制。CLI 成功不能移作 SDK 免费资格；未向 SDK 提供已导入的登录材料，未发模型请求。

**同机 SDK 实测（2026-11-27；无账号、无外网）**。本机用官方 `@cursor/sdk@1.4.2` 的 Node ESM 入口，Node `24.5.0`，在临时空工作区执行。未安装到 Gateway、未连接 `4200`。测试仅加载哈希固定的公开包，复用现有离线夹具，不要求操作者另配环境。

- **7 项通过**：空凭据存储报告未登录；内存文件可读；虚拟文件树读不到进程能看到的对照文件；可写树正对照成功而只读树拒写；为虚拟树显式申请 `shell` 时抛出 `ConfigurationError`；没有 SDK 凭据仍能建立本地纯文本 Agent 句柄；向该句柄提交固定题后返回错误而非答案。没有调用登录接口或读取任何既有身份。
- 本地发送使用 `tools: []`、`settingSources: []` 和只读空文件树；`run.stream()` 共两条消息，未见工具活动，未报告工具列表，`run.wait()` 返回 `status=error`，无结果、无结构化错误码。**能创建 Agent 不代表能推理；离线错误不能判为本人账号未登录、Free 被拒或额度不足。** 此次远端模型请求为 0；文件工具是回合外直接测试，不是模型工具回传。
- [SDK 认证与计费说明](https://cursor.com/docs/sdk/typescript)：SDK 使用用户 API Key，浏览器 SDK 登录会新签发 Key，不是读取已导入 Gateway 的 CLI 登录令牌。`getUsage()` 是事后查询，不是只扣 Free 的发送前保证。[当前套餐说明](https://cursor.com/docs/models-and-pricing)未明确授予新版个人 Free 的本机 SDK Auto 权益。未尝试登录、申请新 Key、付费回退或启用通用路由。
- 离线复核：`python3 -IB tools/probe-cursor-sdk-offline.py <sdk-1.4.2.tgz> <固定 CLI 归档，仅取 Node> <依赖归档目录>`；归档名称、版本与摘要由脚本固定，脚本不下载依赖。脱敏回执：`linshi/cursor-sdk-local-contract-525fbc6e4dd14087ae34e18ac0e02d76/sdk-contract-final.jsonl`。

**Windows 官方 SDK 账号预检（独立于 CLI、4200）**。隔离测试的固定 `/v1/responses` 请求通过真实 Gateway 路由到单一环回适配器，拒绝工具和其他输入。适配器只做 SDK 身份与模型目录检查，**没有模型发送代码**；即使 `auto` 可见也只返回本地 `424`，不得记为 Cursor 推理成功、远端 `424` 或 Free 计费证明。固定公开 SDK 与六个公开依赖在 `tools/cursor_sdk_public_dist.py` 校验；不从项目或用户目录读取账号文件。

- 不访问账号的入口：`operator_sdk_local_check_without_account`。同机 Windows Node 实际加载 `@cursor/sdk@1.4.2`，SDK 使用空内存凭据存储；Node 无文件写入、宿主目录读取或子进程权限。独立 HTTP／SQLite 测试通过，断言审计只去本机适配器一次。另有 9 项模拟 metadata 契约、7 项 Linux 无网对照通过。
- 人工入口：`operator_sdk_account_metadata_without_inference`（忽略测试，显式单独运行）。应在本机浏览器用已确认的**本人个人工作区**授权。SDK 的 `Cursor.auth.login` 请求签发 **10 分钟**的用户 Key（不是 CLI 令牌），`store:null`；一次性网址只送系统浏览器，Key 不交给 Gateway、不写盘／日志。取消请勿批准；已批准后退出本机进程不等于撤销已签发 Key。`Cursor.me` 与登录身份不相符、返回期限过长、目录拒绝或不含精确 `auto` 均停止；没有换模型或付费回退。
- 本地回执只含阶段标记；`official_sdk_browser_dispatched` 仅表示已提交浏览器打开请求，不表示操作者已批准。`sdk_user_verified` 不是套餐检查；`auto_listed_billing_unverified` 不是免费调用资格。官方 `SDKUser` 与模型目录均未声明套餐或个人账源字段；省略 `teamId` 也不能单独证明没有团队账源。此实验不启用通用 Chat、Responses 或 Messages。

## 重做前的开源对照

| 实现 | 对 Gateway 有参考价值 | 不作为本次上游 |
| --- | --- | --- |
| [cursor-sdk2api](https://github.com/Sunnyender-org/cursor-sdk2api) | Responses 事件生命周期、按 `call_id` 关联工具结果、续轮与确切凭据绑定 | 项目的工具适配走 SDK Local；它能接 Codex 不等于能在零本地 Cursor runtime 的条件下使用 Free |
| [cursor-api-proxy](https://github.com/anyrobert/cursor-api-proxy) | HTTP 兼容层与错误映射 | 执行每轮请求需要安装 Cursor CLI，与旧 Gateway 路线同类 |
| [D3Dream/cursor2api](https://github.com/D3Dream/cursor2api)、[AstroQore/cursor2api](https://github.com/AstroQore/cursor2api) | 证明有不启动本地 CLI 的协议桥设计 | 两者使用非公开客户端接口；后者 README 将自己的用例限定为付费订阅。仅研究固定协议；不运行第三方实现，不把其付费账号回执当作本人 Free 证据 |

重做时替换的是上游执行方式：走 Gateway 的远端 HTTP、凭据存储和审计边界，不把 `cursor_acp` 换名后当成新适配器。实验先独立认证并核验免费条件，再推进真实调用与生产协议映射；凭据导入可以单独完成，但入口必须明确标注推理未验证、调用禁用，不能伪装成已可调用的服务商。

## 直连目标复核与防误路由（2026-11-25）

- 该阶段重新读取官方 API 总览、SDK 文档、套餐表及支持答复：Cloud Agents 仍不在 Free 套餐内；官方 API／SDK 是 Agent 工作流接口，不是独立模型推理接口。SDK 标注 `All users` 不代表 Free 具备 Cloud Agents 资格，Local runtime 也不符合零本地 Cursor 约束。没有找到同时满足当前三个条件的获准上游；当时没有使用账号凭据试探私有接口或付费路线。后续经用户明确允许的独立认证实验见下。
- 保留名拒发不再依赖 `auth_mode: cursor_acp` 声明或发送阶段的路由快照；启动未导入、热移除账号和无快照直调发送阶段都不能进入普通 HTTP 重试／回落。私有执行的授权仍要求冻结快照，不用当前快照补位。
- 配置边界覆盖别名、精确路由、`supported_models`、`default_model`、公开映射键，以及账号的 `discovery.models`、`model_catalog.models`、`model_selection.manual/fetched`。后四字段可进入编译后的账号模型列表，影响公共目录或显式开启的定时探测，不能只拦普通选路。四类元数据各有 `cursor-other` 合法正对照及保留名正／大写拒绝，启动和热加载均校验，失败保留活动修订；不禁止上游映射目标中的同名字串。
- 持久化 Responses 的 `resp_gw_*` 前序 ID 会绕过普通选路，因此在既有审计开始后、前序分流之前补保留名拒绝，不改变历史记录的读取、取消和删除。未盘点真实历史 pin；回归使用合成前序 ID 证明请求在 pin 查询之前拒绝，不宣称已验证真实历史数据迁移。
- 真实 Gateway HTTP router＋隔离 SQLite＋回环合成上游回归覆盖未导入／热移除两场景，每场景 12 次 Responses（大小写、`stream`、`store:false`／`store:true`／带合成前序 ID）和 1 次 Chat。**26 次**拒绝均为 `503 cursor_acp_execution_unavailable`，终态审计 attempt 为 0、无账号归属、上游状态或 token 用量。2 次普通模型正对照均成功，并断言实际出站为 `POST /v1/chat/completions`；这是 Gateway Responses 到 Chat 的桥接正对照，不是原生 Responses 上游符合性或 Cursor 真实验收。
- 最终联合 Nextest **316/316**（`5f428789-09aa-426e-90e1-b21dd42d8f19`，6299 skipped），覆盖 Cursor、路由配置、发送阶段及相邻 SAP 持久化 HTTP／前序重写；未启用 Docker opt-in。首轮 `534a139a-1f79-4656-957e-7eb273effeba` 为 315/316：新夹具重复提交同一配置触发 `RevisionUnchanged`，改为实际变更 label 的热加载正对照后复验通过，未放宽业务断言。
- `cargo fmt --all --check`、权威行数 ratchet、`git diff --check` 与修改文件 UTF-8 无 BOM 检查通过。将发送选择归到既有 `cursor_dispatch`，端点流式限制原样归到既有 `endpoint_policy`，`stage_send.rs` **508 → 482**；另有 14 个受限端点和 4 个会话端点的流式／非流式回归。续接前后 `cursor.rs` **137 → 147**、配置测试 **169 → 231**、HTTP 测试 **292 → 333**；未改基线或申请行数例外。最终静态门禁日志位于本机 `linshi/cursor-resume-20261125-final/`，旧失败日志保留。
- 当前仍**未完成供应商接入**。已收到用户继续验证本人账号的操作授权；账号网页前置核验见下，但尚无 Gateway 远端请求的 Free 归属证据，零本地 Cursor／禁止付费回落约束不变。生产 ACP 槽保持未安装；未新增可用账号入口或凭据导入，未启动 Cursor／Docker、发真实推理、生成手测包或切换 4200。

## 远端候选与本人账号网页核验（2026-11-25）

- 北京时间 **14:58**，在用户已登录的官方 `cursor.com/dashboard` 直接读到套餐 **Free**、`Your included usage` 为 **1% used**、重置日 **2026-11-27**。在同一会话的 `dashboard/spending` 读到 `On-Demand Usage` 及 `Requires a paid plan.`；页面提供升级按钮，没有可用的按量付费启用开关。未点击升级或修改计费设置。
- 这证明读取时本人网页会话展示的套餐与用量状态，不证明远端推理可用、某个模型免费，或 Gateway 将来的凭据与该会话是同一账号。该网页核验阶段未读取 Cookie、访问令牌、refresh token 或本地 Cursor 登录文件，也未发认证 API／推理请求。不能把网页显示的 1% 用量归因于本轮调用。
- 只读核对 [D3Dream/cursor2api 固定提交](https://github.com/D3Dream/cursor2api/tree/ea652189aca4d3e244a126dffedd35116695d9a9)：真实发送路径为远端 HTTP/2 Connect `AgentService/Run`，并非启动 CLI；Responses 与工具续轮有实际代码。其 schema 与请求构造没有可验证的请求级 Free-only 开关；默认模型放行、认证重发、冷重建及裸 EOF 处理均不能直接沿用为 Gateway 合同。
- 只读核对 [AstroQore/cursor2api 固定提交](https://github.com/AstroQore/cursor2api/tree/b5b994b98ae69211db3209dcda75319e6dc600b5)：浏览器挑战及官方域名轮询有实际实现，但所得凭据的用途仍属于非官方客户端接法。该项目没有本人 Free 资格或禁止超额付费的验证；没有运行其脚本、复制其实现或让其接触账号。
- 用户随后明确“同意并继续”，已越过非官方实验风险确认；当前推进独立认证及资格核验，不再重复索要风险确认。新凭据必须由本人在官方网页批准，并绑定同一账号；无法核对免费条件则不发推理。旧 ACP 成功仍不抵扣远端 `/v1/responses` 与真实 `codex-cli` 验收。

## 独立远端探针（2026-11-25）

- `tools/cursor_remote/probe.mjs` 支持 `network`、只读 `account`、显式单次 `once` 和有界内存 `session`。经指定 loopback CONNECT 代理建立验证证书的 TLS／HTTP/2，不运行 Cursor、CLI、Bridge 或 Docker，也不读取它们的登录文件。诚实声明实验客户端 User-Agent，不跟重定向、不自动刷新或重发。
- 独立 PKCE 由本人在官方网页批准；token 只存探针进程内。`GetMe` 必须把完整 token subject 与服务端身份、预期本人邮箱摘要同时匹配，再读取 `usage-summary`、赠金、Stripe 余额、团队及模型 ID；不输出或持久化 token、Cookie、原始账号响应或模型对象中的凭据字段。模型响应受 512 KiB 总上限保护，遍历并去重全部合法 ID，不截断为前 256 条。
- 首次授权已成功：服务端身份与本人匹配，`membership=free`、`limitType=user`、`onDemandEnabled=false`、`onDemandUsed=0`。随后模型目录触发旧的 `model_catalog_too_large` 本地检查，进程退出，**Run 为 0 次**，token 未保存。该次未读取新增的其他账源，不能引用为新准入门禁通过；重试只读实验也必须重新批准新挑战。`plan.used/remaining=0` 是金额字段，不能代替免费请求百分比。
- 初版 `once` 在选定目录内明确默认模型后，发送前再次核同一账号全部条件：个人 Free、按量明确关闭且用量为 0、使用百分比在 `[0,100)`、赠金标志明确为 false 且总额／已用均明确为 0、Stripe Free 且余额为 0、显式空团队列表、所有读取成功。该版拒绝 `auto/default`；下节第一方 schema 复核已纠正“未知具体模型即不可使用账号选择器”的过度限制，未放宽独立账源门禁。条件通过才发送一次固定 ASK，不执行工具、不刷新或重放；取消后不再做账号读取。
- 上述是账户级无备用账源检查，**不是请求级 Free-only 协议保证**。成功必须同时具备非空文本、合法 TurnEnded 和无 error 的 Connect EndStream；裸 EOF、工具活动或交付未知都失败。回执仅记录固定答案匹配、计数及终态，不打印任意生成文本；`requestedModel` 不冒充 `actualModel`，免费归属仍标为 `not_proven_by_protocol`，另读同账号调用后状态。
- 第二次授权也已成功且绑定本人：`membership=free`、`limitType=user`、按量明确关闭且已用为 0、Stripe Free、余额为 0。目录返回 **284 项／284 个唯一 ID**；账号默认模型为目录内的字面 `default`，不能据此声称具体执行模型或该模型的 Free 资格。使用百分比为 **0.5**；金额 used／remaining 为 0，不代替免费额度百分比。
- 该次赠金三个投影值均未知，未保存原响应，**不能追述为已观察到空对象**；团队请求返回 **403**，不能当作空团队列表。`once` 被 `account_default_model_not_verified` 拒发，**Run 为 0 次**，进程已退出且 token 未保存。浏览器仍显示 Free、1% used 和按量需要付费套餐；菜单无团队切换入口不是“无团队”的证明。
- 初版新增只读 `GetPlanInfo`／`GetHardLimit`／`GetCurrentPeriodUsage` 诊断，及赠金响应固定字段名的存在性／类型投影；只输出白名单内的字段与类型，不输出未知键、原始响应或凭据。`planOwner` 是订阅商店（Stripe／Apple），不是个人／团队归属。诊断不替代 `inferenceReasons`；当时缺少第一方 schema，未将省略字段补零。下节仅对已验证 schema 的成功响应纠正该解释。
- `session` 仅接受固定 `inspect`／`run`／`quit`，登录后最多保留 5 分钟且不超过 token 到期前 60 秒；最多 3 次检查（含自动首查）和 1 次 Run 请求名额。预检拒绝也消耗该名额，忙时不排队；登录阶段输入不留作后续命令。EOF、`close`、错误、信号与期限取消在途；退出等待所有已发起的并行请求执行清理，但不把这一点等同于逐个确认所有 socket `close` 事件。无监听端口、命令文件、热加载或 token 导出。
- 当前 exec 工具的直接 stdin 已实测立即 EOF，不能依赖 `write_stdin` 保持认证会话。改用持久 Node 工具宿主的 `child_process.spawn` 真正管道，在 **无凭据假回调** 下已跨两轮工具调用完成 `inspect`／`quit` 并观察子进程 `close`、exit 0。父宿主只持有句柄和脱敏回执；宿主真正退出会关闭管道，工具连接断开但宿主未退出则仍由会话 TTL 兜底。该宿主验证不是新的账号授权或真实推理。
- 第三次短期授权已完成并绑定同一本人账号，新增三个诊断均成功：plan 名称为 `free`，`GetHardLimit` 唯一字段为 **`noUsageBasedAllowed=true`**；当前周期百分比为 **0.5**、`limitType=user`。赠金响应这次明确为字段数 **0** 的对象，当时仍保留未知；团队网页接口仍 **403**。目录仍为 **284** 个唯一 ID，默认项仍是 `default`。未发 `run` 命令，**Run 为 0 次**；发送 `quit` 后观察子进程 `close`、exit 0，凭据未保存。
- 该版脱敏回执及 14 个源码／测试输入的 SHA-256 位于 `target/provider-evidence/cursor-remote-20261125-diagnostics.json`；不含挑战 URL、本人邮箱／摘要、令牌、原始响应或生成文本。它证明远端认证与账号诊断有效，**不证明真实推理或免费归属**，也不替代后续修改的来源与验证。旧回执原样保留。
- 最新定向 Node **44/44**、快速及权威行数 ratchet、UTF-8 无 BOM／空白检查与 scoped `git diff --check` 通过，包含既有真实回环 TLS／HTTP/2 测试，以及会话中途关闭、登录输入不重放、身份校验先于目录、并行取消等待和目录失败不掩盖诊断的回归；两路独立只读复审未发现剩余取消／准入缺口。合成流不能冒充 Cursor 推理回执。
- 通过 `127.0.0.1:42344` 代理的无凭据 HEAD 已实测 **HTTP 200**、验证 TLS、协商 `h2`。截至第三次登录诊断结束仍未发送真实 Run。未改 live 4200、账号库或生产运行槽，未重复 Rust 构建、旧测试或出包；探针不算完整供应商或 `codex-cli` 接入。

## 第一方 schema 与单次拒付实验门禁（2026-11-25）

- 官方文档确认包名为 [`@cursor/sdk`](https://cursor.com/docs/sdk/typescript)。仅静态下载和读取 npm 发布归档 `@cursor/sdk@1.4.2`，未安装、导入或执行 SDK。归档 SHA-512（base64）为 `IMeIZtLvVX5w6rOS34bfBQt2b/WsMm+6lLU6vdlia0E6RuuXup+aVXyJrzvVAfnI7pK8bPPc7Gx1dvxy7ci+1Q==`；原始输入位于本机 `linshi/cursor-public-schema-20261125/`。这不是运行时依赖，也不把非公开端点声明为厂商获准 API。
- 包内 `aiserver.v1.DashboardService` 明确声明 `GetTeams`、`GetCreditGrantsBalance`、`GetHardLimit`。赠金响应四个字段 `hasCreditGrants`／`creditBalanceCents`／`totalCents`／`usedCents` 均为隐式标量，默认 false／0；`GetTeamsResponse.teams` 为 repeated，默认空列表。因此，正确 RPC 的成功 ProtoJSON `{}` 可以表达上述默认值，不再一概保留未知。这依据第一方描述符和 [ProtoJSON presence/default 语义](https://protobuf.dev/programming-guides/json/)，不是把 HTTP 错误、任意对象或第三方默认值当成功。
- `account_wire.mjs` 只解析已核实 schema，HTTP 200、JSON 媒体类型、UTF-8、有界唯一键解析先于默认值。拒绝包装对象、未知字段、错误类型、显式 null、双拼写别名和超界金额；null 和数值表示有意采用比通用 ProtoJSON 更严格的子集，不宣称标准禁止 null。int64 不经不安全 Number 舍入后放行。
- 团队读取改为固定 `api2.cursor.sh/aiserver.v1.DashboardService/GetTeams`、Bearer 和 `activeOnly:false`，不伪造 Origin／CLI 指纹，不把旧网页 403 当空列表。任何团队条目均拒发，`FREE_OWNER` 不解释为个人团队。赠金请求显式 `includeAllProducts:true,includePersonalTeam:true`；字段名表达查询意图，不声称已获得服务端对所有范围覆盖的保证。
- 发送前新增个人 `GetHardLimit`（`personalScope:true`，不传 teamId），必须 `noUsageBasedAllowed=true`；省略该字段按 schema 解为 false 并拒发。optional 附加预算字段省略仍是 null；显式促销、待领额度、非零或无限附加预算等拒发。原有 Free、本人身份、个人范围、按量 off／0、百分比、Stripe Free／0 检查全部保留；赠金新增检查 `creditBalanceCents=0`。
- `catalog_wire.mjs` 对默认对象及同 ID 的唯一目录条目同时校验；拒绝 BYOK oneof、重 ID 歧义、`maxMode:true`、任何 `thinkingDetails` presence（当前 RequestedModel 无无损映射）及未知结构。可使用同账号原样返回的 `default/auto` 不透明选择器，不改写、不选替代模型；它不是实际执行模型或 Free 资格。请求仍只有单次固定 ASK，无工具、凭据分支或重试／回落。
- 上述可用于用户已授权的单次、无工具实验，**不构成 RPC 级绝对不收费合同**。请求后仍需核账源；无可关联扣额证据则免费归属保持未证，不能用零金额变化代替 Free 扣额成功，更不能据此默认开放生产路由。
- 该门禁批源码定向 Node **53/53**，快速及权威行数 ratchet、UTF-8 无 BOM／空白检查通过；三个独立只读分片复审覆盖账源、模型和 JSON／传输边界。随后实际调用及其诊断补丁分项记录如下，不能用新诊断代码追述旧响应。

## 首次远端 Run 与错误诊断收口（2026-11-25）

- 用户完成第四次新挑战后，真实预检已通过：本人身份、`membership=free`、`limitType=user`、`onDemandEnabled=false`／`onDemandUsed=0`、百分比 **0.5**；赠金四项 false／0，Stripe Free／0，个人 HardLimit 明确禁止按量且无已确认替代预算。新的 Bearer `GetTeams` 成功返回字段数 0 的对象，按已核 schema 得空列表；不是忽略原网页接口 403。目录仍 **284** 项／唯一 ID，默认 `default` 同时通过来源、BYOK、变体和重 ID 检查，`selectorSafe=true`。
- 经发送前重新读取同一账号全部账源后，**只调用一次** `AgentService/Run`。收到 **2 个 Connect 帧**，事件只有 `server:1`／`update:13`，文本 **0 字节**，`turnEnded=false`，随后本地报 **`connect_remote_error`**。第一方描述符确认 `update:13` 是空消息 heartbeat；它不证明模型执行、回复或计费。未刷新、重试、切模型／账号或供应商回落。
- 旧解码器只检查终止对象中存在 `error` 键，未保存 `error.code` 或正文，所以不能回溯为额度、鉴权、模型或某个缺失请求字段的拒绝，也不能断言已验证合法远端错误终态。SDK 通用构造器存在 `modelDetails` 参数，但后续追到主调用点确认传入 `undefined`，不能把子代理分支的行为搬到主请求。空 `mcpTools`／`runId` 等字段差异也不足以单独证明本次拒绝原因。
- 请求后的同账号读取仍为个人 Free、按量 off／0、百分比 **0.5**，赠金／支付余额为 0、无团队、个人按量被禁止。这里只是读取时状态，没有可关联的 Free 扣额证据，不推断本次零计费。`quit` 后收到子进程 `close`、exit **0**，读回探针 PID 不存在、命名 `Cursor`／`cursor-agent` 进程数 **0**，token 未保存。
- 脱敏真实回执冻结于 `target/provider-evidence/cursor-remote-20261125-single-run.json`，含实际运行时 **18** 个源码／测试文件 SHA-256、53 测试验证状态与前后账源投影；无挑战 URL、邮箱／摘要、凭据、原始响应或生成正文。该回执保留旧输入，不覆盖为新诊断结果。
- 后续仅补错误可观测性：Connect 终止帧复用唯一键 JSON 解析，只保留 **16 个标准错误码白名单**；不输出 `message`／`details`／`metadata`。合法错误帧也必须等待正常传输收束，尾随数据／reset／截断仍为 `delivery_unknown`；完整错误终态记 `remote_terminal_error`，不是“执行前拒绝”，两者均禁止重放。对未知／缺失错误码、非法形状和敏感正文的合成回归已覆盖。
- 最终定向 Node **57/57**、权威行数 ratchet、UTF-8 无 BOM／空白检查与 scoped diffcheck 通过；末次独立只读复审通过。新增模块和测试均低于 **250** 有效行。新诊断只经合成验证，**没有发第二次真实 Run**；其独立输入回执为 `target/provider-evidence/cursor-remote-20261125-error-diagnostics.json`。
- 该阶段仍**未完成远端供应商接入**：首次真实 Run 无文本成功，具体上游错误码未取得，免费归属未证；没有进行 `codex-cli → Gateway /v1/responses` 验收。未改 live 4200、账号库或生产槽，未启动本地 Cursor runtime／CLI／Bridge／Docker，没有 Rust 变更、构建或出包。后续独立诊断结果见下，不回溯或自动重放首次调用。

## 请求 presence 对齐与可定位错误（2026-11-25）

- 官方固定 SDK 的主会话以空状态开始、`modelDetails=undefined`、模型放 `requestedModel`。仅校齐已确定的 presence 差异：#4 显式空 `McpTools`（`22 00`）、#25 新生成的 `runId`、#34 optional uint32 显式零（`90 02 00`）。保留 ASK、原样账号默认选择器、空历史；不宣告工具、不换 Agent 模式、不补子代理模型参数。新实验标记 `wireRevision=2`，不宣称省略字段已被证明是首次错误根因。
- `connect_error.mjs` 仅识别 `aiserver.v1.ErrorDetails` 中经固定描述符核验的 58 个非零枚举，保留标准 Connect code 及固定分类。依据 [Connect 规范](https://connectrpc.com/docs/protocol/#error-and-endstreamresponse) 校验无填充标准 Base64；同类型重复详情、重复枚举、未知值、过大或坏二进制不猜原因。详情信封本身不合规仍拒绝终态；结构合规但无法解释的可选载荷仅省略增强分类。正文、debug、metadata、原始字节均不进回执。
- 线编码原语原样归到 `protobuf_wire.mjs`，保留旧导出供现有测试使用。真实 loopback TLS／HTTP/2 对端以 ping 往返作为观测屏障，分别验证请求写侧保持开放及 half-close 正对照，随后发送 heartbeat 和 typed error；这不是 Cursor 实际推理，且不扩大已获准的远端尝试次数。
- 定向 Node **65/65**、快速行数门禁、UTF-8 无 BOM／空白、scoped diffcheck 和三路只读复审通过。21 个源码／测试指纹及逐文件行数保存在 `target/provider-evidence/cursor-remote-20261125-wire-v2-validation.json`，全部低于 250 有效行。该本地验证收口时补丁后真实 Run **0 次**，不代表下节后续自动单次实验也未发送；旧回执保持不变，不复用已经退出的会话或从浏览器提取 Cookie／token。

## 已授权事实、自动单次结果与流程修复（2026-11-25）

- **收到过认证材料，不是用户没有登录。** 一次 `session` 已完成 `login_completed`、`accountMatched=true` 和完整免费账源预检；但等待聊天衔接期间，探针的 **5 分钟会话期限**到期，`runRequested=false`，Run **0 次**。这是探针到期，不是已证实 token 自身失效，更不等于浏览器退出登录。回执为 `target/provider-evidence/cursor-remote-20261125-authorized-session-expired.json`。
- 随后的 `once` 已自动衔接：北京时间 **19:25:30** 收到令牌，**19:25:32** 本人账号匹配；读取 284 项目录并再次通过账源检查后，**19:25:37** 对原样 `default` 选择器发起 **1 次** Run。**19:25:38** 收到完整远端错误终态：`resource_exhausted`／`GPT_4_VISION_PREVIEW_RATE_LIMIT`，2 帧、文本 0 字节、`turnEnded=false`。**19:25:41** 完成后置账号读取后退出 1；没有重挑战、刷新、重试、换模型／账号或付费回落。
- 该标签是远端限流类分类，不是实际模型 ID，也不能证明本人 Free 总额度耗尽。前后仍为个人 Free、按量 off／0、百分比 0.5、赠金／支付余额为 0、团队为空且个人按量禁止；账户快照不证明这次零计费或 Free 扣额成功。冻结回执 `target/provider-evidence/cursor-remote-20261125-wire-v2-once.json` 保留当时 21 个源码／测试指纹。现有两笔真实 Run 都没有文本成功，首次具体错误仍不可追溯。
- **后续已批准的单次实验继续使用现成 `--mode once`，不再先开 `session` 等聊天确认。** `account` 仍只读，`session` 仍受原有 5 分钟／单次 Run 上限限制；不通过延长 TTL、落盘明文 token 或解除免费门禁来消除反复登录。当前没有启用长期凭据导入；浏览器登录、探针收到令牌、本人身份核验、远端推理可用是四个独立状态。
- 探针新增内建 `receipt.mjs`：登录前在固定仓库 `target/provider-evidence/cursor-remote-<attemptId>.jsonl` 独占创建回执，阶段行带同一 attempt ID、时间戳，启动行记录脚本源码 SHA-256。每行同步落盘，不再依赖持久 Node 宿主在退出时汇总；固定字段及文本枚举投影，不落盘授权 URL、邮箱／摘要、token、Cookie、模型字符串、原始响应或生成正文。交互输出仍可展示公开授权链接，但写回执失败后不再显示失去接收方的链接。
- 摘要分别保留 `authReceived`、`accountMatched`、`credentialRetention`、`inferenceStatus`、`failureStage`、`stopReason`。探针清除令牌引用不会抹去已授权事实；不宣称 JS 字符串已安全擦除。`inferenceAttempts` 是本地 transport 尝试数，不是远端接收／执行次数；准入行写失败会在调用前取消并记 0，不生成伪 Run 回执。
- 文件最多 256 行／1 MiB；写失败取消后续工作，保留已得到的上游错误，仍执行清理，不重放。磁盘末行 `receiptComplete=true` 表示摘要已写入；终端同名字段表示末行同步成功，另用 `receiptClosed` 表示文件关闭成功。无摘要或尾行截断必须视为回执未完成，不能据此推断零发送、正常退出或成功。
- 本轮流程补丁定向 Node **82/82**，包含关闭 stdin 后一次登录自动一发、限流仍保留认证事实、账号／身份／免费预检分阶段、写入和关闭故障、短写、日志上限及真实子进程退出后保留阶段行。权威行数 ratchet、UTF-8 无 BOM／空白、scoped diffcheck 与三路只读复审通过；24 个源码／测试均低于 250 有效行，输入绑定回执为 `target/provider-evidence/cursor-remote-20261125-auth-lifecycle-validation.json`。测试没有访问 Cursor；本轮新增真实 Run **0 次**。远端供应商、免费归属和 `codex-cli → Gateway /v1/responses` 仍未通过；未改 live 4200、账号库或生产槽，没有 Rust 构建或出包。

## 限流根因排查与诊断边界（2026-11-25）

- 继续静态核对官方 CLI `2026.08.11-e8db854` 已下载归档（84,532,310 字节，SHA-256 `bfff4bf6f4e9dd30c1d0ef0a70b6077b074015dd2948e4c50685d53afdcfce5a`），不安装、执行或读取其账号文件。ACP 的 `ask → search → ASK=2` 与探针一致；有 `RequestedModel` 时同样不填 `modelDetails`。模型构造不硬编码 `default → auto`，但配置及参数化目录中的 `legacySlugs` 等可影响最终选择；没有本人相应目录或运行记录，不能直接改写当前选择器。
- CLI 会先读取 `ServerConfigService/GetServerConfig`，结合 `http2Config`、`agentUrlConfig` 和账号隐私设置选择 Run 通道／主机；探针固定 HTTP/2 与 `agentn.global.api5.cursor.sh`，尚未实现该发现流程。这是已证的合同差异，**不是限流根因证据**。本轮一次无凭据配置查询返回 **401**；只证明该匿名查询被拒绝，不说明用户浏览器未登录或此前账号令牌无效。没有凭据可据此回填本人配置，也没有改主机或仿写 ACP／SDK 身份头。
- SDK 和 CLI 都不能仅凭枚举 28 判断具体模型、额度池、等待时长或必需动作。SDK 的 `retryAfterMs` 专用分支要求另一枚举 `RATE_LIMITED=50` 与特定 `additionalInfo.kind`，不能套到本次。最新 SDK 文档中 legacy `default` 的警告属于 **Cursor Router** 专节，不是 CLI 的通用选择器禁令；不据此换成 `auto-smart`。
- 补齐经固定 CLI 描述符核验的安全提示：`CustomErrorDetails.#4` optional bool 投影为 `remoteCursorRetryable` 三态；`#10` analytics 内 `#1` 只接受精确字面 `login/upgrade/payment/config`，未知动作不保存。单独的 `remoteCursorHintsStatus` 区分缺省、有效、不支持和无效；坏提示不覆盖合法错误枚举，正文、按钮、URL、其他详情均不进入回执。BOM 前缀、重复字段、坏 wire、无效 UTF-8 和超长动作均有回归。
- 提示只供诊断，**不会触发重新授权、升级、付费、配置修改或重放**，也不抹去 `authReceived/accountMatched`。完整 `once` 回归注入 `retryable=true` 和 `action=login`，仍只有一次登录轮询、一次 Run，保留原错误和 `replayAllowed=false`。当前两笔真实 Run 的旧回执没有这些字段，不能追述为“上游明确要求重新登录”。
- 该批 Node **89/89**、权威行数 ratchet、UTF-8 无 BOM／空白和 scoped diffcheck 通过，三路只读复审完成；输入及静态调查定位保存在 `target/provider-evidence/cursor-remote-20261125-error-hints-validation.json`。本轮没有新挑战或真实 Run，没有修改 live 4200、凭据库或生产槽；上游真实回复、免费归属和完整 Gateway 接入仍未通过。下一步先核对本人用量记录及带账号身份的服务端选路／模型选择证据，不因通用限流标签盲目重试。

## 保存凭据诊断与调用路径排查（2026-11-26）

- 新增 `inspect_saved.mjs`、`saved_credential.mjs`、`server_config.mjs` 和 `model_selection.mjs`，
  从本人已经导入的 exact Gateway 活动修订只读取得材料，先做远端 `GetMe` 绑定；不创建挑战，
  不读取本机 Cursor 登录文件，不执行 SDK／CLI／Docker，不新增凭据文件或修改账号路由。
  本机文件读有大小、摘要、链接、重复 JSON 键和活动修订防线，每次远端读取前重新检查修订。
  全流程最多 60 秒，各请求仍受原传输期限约束，等待已发起的并行读取结束后释放材料引用。
- 服务配置只投影固定字段。静态依据仍是已冻结的 `@cursor/sdk@1.4.2` 归档：
  `GetServerConfigResponse` 的 `http2_config`／`agent_url_config` 与 `AgentUrlConfig` 三个 URL 字段。
  不保存配置版本值、任意主机或响应里的其他配置／密钥；不把未知地址加入出站白名单。
  本轮在线官方文档工具未取得有效内容，不能将固定归档描述为“最新文档已复核”。
- 继续静态核对已冻结 CLI `2026.08.11-e8db854` 归档，重新核验 SHA-256 与此前相同。
  `dist-package/189.index.js` 的 ACP shared-services 使用 `aiserver.v1.AiService` 模型客户端；
  `7397.index.js` 的 model-service 并行读取 `GetUsableModels`、`GetDefaultModelForCli` 和
  `AvailableModels(useModelParameters=true,doNotUseMarkdown=true)`。原探针前两项走
  `agent.v1.AgentService`，诊断现在分别读取并比较，未直接替换推理选择器或放宽 Free 门禁。
- 三次本人只读诊断均成功载入保存材料并通过远端身份匹配。首次回执
  `target/provider-evidence/cursor-remote-a93c2a29-b6c8-41c0-89cb-d39e1cd49b42.jsonl`
  显示赠金及个人 HardLimit 两项读取失败，摘要为 `saved_inspection_incomplete`，**不是未登录**。
  当时投影只保留失败计数，无法追述具体错误；随后补 operation／安全 code／HTTP status 投影，
  不输出任意错误消息。第二次
  `cursor-remote-8e56fb50-9193-4480-914c-62be7d9b7af7.jsonl` 全部读取成功，未更改账源解析或准入条件，
  不能将前次失败冒称为已证实某种网络或 schema 根因。
- 第三次完整回执
  `target/provider-evidence/cursor-remote-2986af52-18ab-4cac-840a-805f72a74ef4.jsonl`
  记录：个人 Free、按量 off／0、包含用量百分比 **0.5**、赠金／支付余额 0、团队 0、
  `personalNoUsageBasedAllowed=true`、`alternateLimitSource=false`、读取错误 0，
  `billingBlockers=[]`。两类服务模型目录均 **284** 项，默认选择器相同且通过既有 selector 检查；
  参数化目录 **49** 项，默认项有 **1** 个同名匹配、**0** 个 legacy alias 匹配、
  **0** 个参数定义和 **1** 个 variant。本次没有需要按 legacy alias 改写默认模型的证据。
- 三次带身份的空请求服务配置均返回 `http2Config=UNSPECIFIED`，三个配置主机均与当前
  `agentn.global.api5.cursor.sh` 一致。它不证明所有客户端上下文、隐私设置或未来请求的路由相同，
  也不证明 HTTP/2 被显式授权；但不能再以先前匿名 401 或“固定主机必定错误”解释本次账号状态。
- 本轮新挑战 **0**、新 Run **0**，没有自动重放历史两次错误，没有启用账号或后台探测。
  历史 `resource_exhausted / GPT_4_VISION_PREVIEW_RATE_LIMIT` 根因仍未闭环；
  本次不宣称真实回复、请求级 Free 归属或 `codex-cli → Gateway /v1/responses` 已通过。
  后续应继续核对隐私／请求上下文及可关联的上游错误证据，不猜测模型、仿写官方客户端指纹或付费回落。
- 最终定向 Node **108/108**，新增用例覆盖 exact archive 读取、重定向／硬链接／摘要／修订冲突、
  脱敏、写回执失败零发送、身份错误先停、并行读取回收和服务／模型配置投影。
  权威行数 ratchet、UTF-8 无 BOM／空白和 scoped diffcheck 通过；本批源码／测试 **41–232**
  有效行。测试日志为本机 `linshi/cursor-saved-diagnostics-20261126/node-tests.tap`。
  三路只读子代理因其模型调用路径返回 403 未能完成复审，不记作独立复审通过，也不把它误认为
  Cursor 账号 403。本轮由主代理核对源码及测试；没有 Rust 或桌面变更，无须重复构建／出包。
- 验收后管理 API 再读仍为 `r2-1664e9a2f8ee`，同一账号调用及后台探测保持禁用，到期信息未变。
  4315 后端 PID 50260、loopback 4200 PID 40484、LAN 4200 PID 11216 保持不变；
  命名 `Cursor`／`cursor-agent` 进程数为 0。没有切换运行包或调整其他实例。

## 保存凭据单次实验与 payment 提示（2026-11-26）

- 静态核对固定 CLI 主路径：`UserMessageAction` 会填充 `requestContext`，每个请求设置
  `x-ghost-mode` 和新 `x-request-id`。本批 `wireRevision=3` 保留 SDK 已确认的 Run presence，
  增加明确的严格隐私请求和随机请求 ID；按第一方 RequestContext 描述符仅填
  `web_search_enabled=false`、`web_fetch_enabled=false`，没有环境、仓库、工具或文件内容。
  继续使用诚实的 Gateway User-Agent，不添加官方 CLI／ACP／SDK 身份头，不更改账号隐私设置。
  这些是已证实的协议差异，但没有证据把它们称作历史拒绝的根因。
- 新增独立 `run_saved_once.mjs`，复用 exact 已保存凭据、固定投影回执、身份、目录及完整账源门禁。
  只读 `inspect_saved.mjs` 不变。单次入口先核带隐私头的服务配置和两类模型目录，
  主机不一致、HTTP/2 被禁止或默认项需要映射时拒发；总期限 120 秒，Run 期限 60 秒。
  无登录、刷新、重试、替代模型或付费回落；并行请求回收后才释放材料引用。
- 错误正文仅在内存中作有界词汇提示投影：固定第一方 `CustomErrorDetails` 的 title／detail
  只输出预定义的“提及”标签，不能把否定句或关键词当作原因。未知文本、URL、命令、按钮、
  附加信息、原始字节及生成文本不输出、不持久化；提示不触发任何动作。
- 本人凭据载入及身份核验成功，服务配置三个主机仍相同，两类目录各 284 项、默认选择器相同，
  参数化默认项 1 个同名匹配、0 个参数定义、1 个变体。即时预检全部通过：
  个人 Free、按量 off／0、包含用量 0.5%、无赠金／支付余额／团队、个人按量禁止。
- 北京时间 **06:29:31.880** 发出本轮唯一 Run，**06:29:33.058** 收到完整错误终态：
  `resource_exhausted / GPT_4_VISION_PREVIEW_RATE_LIMIT`，**新增确切证据为
  `remoteCursorAction=payment`**。重试提示缺省，正文没有匹配已定义的词汇标签；
  2 帧、0 文本、无 TurnEnded。没有再次授权、刷新、重放、换模型／账号或支付。
  本地请求 ID：`e678296b-ab42-4cf9-9ff0-1b2c2ec527a6`。
- **06:29:35.165** 后置账源读取成功，仍为上述个人 Free 状态和 0.5%；账户快照不证明零计费，
  `payment` 提示也不能推出实际模型、Free 总额度耗尽、所有远端接口的付费要求或厂商授权。
  可确认的是：保存凭据可用，但本次修正仍不足以让该远端调用返回文本。
  后续须解决该方法对本人账号／客户端的准入问题，不能再用重复登录或放宽付费门禁替代诊断。
- 完整脱敏回执：
  `target/provider-evidence/cursor-remote-cf0f967f-0e30-4c1a-aa83-7ee5b3b75a2b.jsonl`，
  带实际脚本来源 SHA-256、阶段时间、完整摘要及正常关闭确认。三笔历史真实 Run 均未成功，
  当前依然没有 Gateway HTTP／`codex-cli` 成功证据；不启用生产账号或后台探测。
- 本批 Node **117/117**，无失败／跳过；覆盖单次保存凭据入口、逐项拒发、取消、回执写失败零发送、
  终态错误／未知交付不重放、并行回收、隐私头、无工具上下文及正文不泄漏。
  日志：`C:/Users/Public/nas_home/AI/GameEditor/linshi/cursor-saved-once-20261126/node-tests.tap`。
  新增／修改源码及测试为 **37–247** 有效行；主代理复审，未把此前失败的子代理当作独立复审。
  权威有效行数 ratchet、UTF-8 无 BOM／空白及 scoped diffcheck 通过。
  验收后管理 API 读回仍为 `r2-1664e9a2f8ee`，同一账号及后台探测禁用，到期信息未变。
  没有 Rust／桌面源码变化或重新出包，4315 和两个 4200 实例未被切换；
  读回命名 `Cursor`／`cursor-agent` 进程数为 0。

## 分项用量、默认变体和账号页面复核（2026-11-26）

- `inspect_saved.mjs` 新增固定只读 `GetPlanInfo`／`GetCurrentPeriodUsage(personalScope=true)`，
  按已核第一方描述符分别投影总用量与 Auto／API 分桶。optional 金额与重置时间缺省保留 null，
  零金额 limit 不解释成额度耗尽，`usageEnabled=false` 不解释成账号禁用；
  原始时间整数不擅自判定单位，`entitlementVerified=false` 保持不变。
- 默认变体诊断补充参数值数量、Max Mode、Agent 支持与数据保留要求。显式单次入口对非空参数、
  Max Mode、明确不支持 Agent 或要求数据保留的选择拒发；不改写模型、不放宽账源门禁。
  固定 CLI 的 `buildRequestedModel()` 主路径按所选 ID、maxMode 与 parameters 构造，
  尚无默认补 `builtInModel=true` 的依据，不为碰运气添加字段或官方客户端指纹。
- 首次只读回执 `target/provider-evidence/cursor-remote-e9be287f-f6c4-4589-b53b-65163e0182b3.jsonl`
  已取得分桶数据，但后续模型读取报 `invalid_json_response`，当时未保留具体 operation／解析原因。
  后续补固定安全标签和套餐名大小写归一化；不回溯解释旧错误，也不宣称后次成功证明根因已修复。
- 完整只读回执 `target/provider-evidence/cursor-remote-aa461142-7ceb-40fc-b090-4b78008c96a6.jsonl`
  于北京时间 **06:47:46** 完成：本人身份、个人 Free、全部无替代账源检查通过；
  总百分比 **0.5%**、Auto **1%**、API **0%**，默认选择器在 Auto 桶内。默认变体参数值 **0**、
  Max Mode **false**、支持 Agent **true**、数据保留要求 **null**。这些读取未显示分桶百分比耗尽
  或默认变体参数遗漏，但不证明本方法对该账号的推理准入或请求级 Free 归属。
- 用户随后提供官方 `Overview` 与 `Spending` 页面截图：当前显示 **Free**、包含用量 **1% used**、
  **2026 年 11 月 27 日**重置；Spending 可见内容仅说明按量使用需要付费套餐，没有请求明细。
  截图不包含可关联到 **06:29** 失败请求的记录，不能据此证明零计费、具体执行模型或
  `remoteCursorAction=payment` 的根因。无需继续要求用户登录、切换页面或升级。
- 该阶段两次只读检查新增 Run **0**，当时三笔历史 Run 均未返回文本。缺口是该远端 Run
  的拒绝原因及可用调用合同，不是凭据导入；当时未发第四笔，后续获准诊断结果见下节。
  Gateway HTTP、`codex-cli` 与 Free 扣额验收仍未通过。
- Node 回归 **123/123**、失败／跳过均为 **0**，复用
  `C:/Users/Public/nas_home/AI/GameEditor/linshi/cursor-allowance-20261126/node-tests-2.tap`；
  当前 **22** 个探针源码指纹与完整只读回执一致。权威有效行数 ratchet、scoped diffcheck
  和源码／测试／本文档 UTF-8 无 BOM、空白检查通过。没有新的 Rust／桌面改动或出包，
  不把旧子代理调用失败计作本批独立复审，也不把账号截图当作成功推理证据。

## 版本门禁根因与拒发修复（2026-11-26）

- 用户再次明确要求尝试并查明原因后，增强 Connect 顶层 `error.message` 与 typed title／detail 的
  有界诊断：先遮蔽邮箱、URL、JWT，再只投影固定标签和最多 96 个白名单词汇；其他词、数字和
  标识符变为 `_`，单段正文不超过 4096 字节。原始正文不输出、不落盘，任何提示均不触发动作。
- 第四笔真实 Run 于北京时间 **07:58:28.068** 发出，**07:58:29.603** 收到完整错误终态。
  外层仍是 `resource_exhausted / GPT_4_VISION_PREVIEW_RATE_LIMIT` 和 `payment`，但详情词序明确出现
  `update required`、版本不再受支持及更新后继续的要求；两帧、零文本、无 TurnEnded。
  这是版本兼容拒绝证据，不能再将通用枚举／动作解释为 Free 额度耗尽或必须付费。
  回执为 `target/provider-evidence/cursor-remote-02ca10a3-174a-4225-bd64-33131f3f0c94.jsonl`，
  请求 ID 为 `bc99232e-b0e9-4830-9c67-a1202c2a6156`；调用前后仍通过同账号全部无替代账源门禁。
  原回执的信号标签为空；识别 `Update Required / no longer supported` 的正则在取得该证据后补齐，
  不改写旧回执，也不以新解析器冒称当时已经产生新标签。
- 静态核对固定 CLI 描述符：`GetServerConfigRequest` 没有版本字段；
  `GetServerConfigResponse.#23 client_version_status` 包含更新等级、当前版本、两种最低版本及消息。
  新投影校验类型、枚举、别名冲突和长度；缺失／UNSPECIFIED 保持未知，消息只记存在性。
  版本只有短数字 `major.minor.patch` 可输出，其他值脱敏；不保存其余配置或密钥。
- 两次无 Run 的已保存账号诊断均返回 `REQUIRED`。完整数值回执
  `target/provider-evidence/cursor-remote-4ae46551-feb1-4719-ab58-f30829b4fcbe.jsonl` 显示
  `minAllowedClientVersion=3.12.0`、`minSupportedClientVersion=3.18.0`；当前版本为非数字版本投影，
  不是已确认的某个旧版本。本人身份、Free、总用量 0.5%、Auto 1%、API 0% 及账源门禁均通过。
- 随后一次临时只读对照如实声明 `x-cursor-client-type=neuro-gateway`、
  `x-cursor-client-version=neuro-gateway-0.1.0`，只发送 GetMe 和 GetServerConfig。
  服务端当前版本与声明精确匹配，但仍为 `REQUIRED`；**不是只要补任意版本头即可接通**。
  此实验没有 Run，也未将未获支持的头加入正式探针；记录在
  `C:/Users/Public/nas_home/AI/GameEditor/linshi/cursor-allowance-20261126/identity-readonly-check.jsonl`。
  上述最低版本是服务端当前客户端检查的回包，不代表 Gateway 实际软件版本或已获准协议版本。
- 第一方固定 SDK 的版本函数实际发出 `sdk-<包版本>`，CLI 发出 `cli-<构建版本>`，不是第三方通用
  协议版本。通过已有代理重新读取官方 API 总览、TypeScript SDK 页面均为 HTTP 200；所核页面没有
  给出第三方直连所需版本声明合同。文档工具的 403 属于工具通道，不是 Cursor 账号错误。
  因而没有把 SDK／CLI 标识照抄到 Gateway，也没有升级或运行本机 Cursor 软件。
- `run_saved_once.mjs` 现在先处理服务配置的显式更新拒绝，再读取参数化模型目录，错误为
  `saved_run_client_update_required`。首次门禁验收先遇到后续模型 `invalid_json_response`，
  说明原检查顺序会遮蔽已知阻塞；调整顺序后，真实回执
  `target/provider-evidence/cursor-remote-bf87564a-fe41-4922-9c29-ad4eb2fa1901.jsonl`
  确认在 preflight 以该明确错误退出、`inferenceAttempts=0`，摘要完整且关闭成功。
- 本批全套源码回归 **129/129**；最后调整检查顺序后，相关单次入口 **6/6** 再验通过，其余输入未变，
  没有重复空跑。覆盖版本消息分类、枚举／别名／缺省、版本值脱敏、已知配置拒绝先于后续模型读取及零 Run。
  权威行数 ratchet、39 个相关文件 UTF-8 无 BOM／空白和 scoped diffcheck 通过，
  22 个探针源码 hash 与最终真实拒发回执一致；源码及测试为 **50–247** 有效行。
  汇总为 `target/provider-evidence/cursor-remote-20261126-version-validation.json`。
  4315 与两个 4200 的既有进程均仍存在，命名 `Cursor`／`cursor-agent` 进程为 0；
  没有 Rust／桌面变更、构建、出包或运行实例切换。此前子代理通道 403 仍不计作独立复审。
  该阶段没有第五笔真实 Run；当时四笔历史 Run 均未返回文本。生产执行器、Gateway HTTP／`codex-cli`、
  实际模型及 Free 扣额验收仍未完成。后续用户指定的版本头实验如下，不需要再登录。

### 用户指定的版本头兼容实验

- 用户明确要求修改发送请求的客户端版本号尝试后，仅在临时实验包装层设置
  `x-cursor-client-version=3.18.0`，保留 `x-cursor-client-type=neuro-gateway` 和原 Gateway User-Agent。
  实际 Gateway 仍为 `0.1.0`，没有安装、升级或执行本机 Cursor，也没有把此头写入默认传输或生产配置。
  包装层必须观察到服务配置回显相同版本且 `clientUpdateLevel=NONE` 才允许继续；
  其余身份、exact 修订、路由、模型及 Free／无替代账源检查全部复用原入口，最多一笔 Run。
- 服务配置实际返回 `currentClientVersion=3.18.0`、`clientUpdateLevel=NONE`、
  `clientUpdateRequired=false`，说明这个数值通过了配置接口的版本检查。不能把配置检查通过解释为
  Run 兼容、完整协议实现或厂商认可第三方客户端。
- 第五笔真实 Run 返回完整错误终态 **`permission_denied / OUTDATED_CLIENT`**，
  `remoteCursorAction=config`，详情提及过期客户端及更新要求；两帧、零文本、无 TurnEnded。
  因此版本头变化确实改变了响应，但仅改为该数值仍未接通。尚不能据此断定具体缺少哪个字段、
  是否存在不同版本阈值或客户端类型检查，不通过重复换身份／版本来猜测。
- 真实回执：
  `target/provider-evidence/cursor-remote-8f0e7c80-0dbb-4220-8bee-09a9129cc0bd.jsonl`；
  请求 ID：`c71f7006-51c6-44e6-85e5-fce980a4cb44`。
  前后身份、个人 Free、按量 off／0、总用量 0.5%、赠金／余额 0、团队为空及个人按量禁止均通过；
  这些快照仍不能替代可关联的 Free 扣额证明。没有刷新、重放、替代模型／账号或付费回落。
- 临时实验脚本 SHA-256：
  `13708db9c7c628982a68c8cebda2ff068d6660850b324107c3e64e9a8099d29b`。
  头值与该脚本指纹在本机 `linshi/cursor-allowance-20261126/version-318-once.jsonl` 中记录，
  持久化实验关联摘要见
  `target/provider-evidence/cursor-remote-20261126-version-318-experiment.json`。
  包装脚本语法检查通过，正式探针源码未改，不重复旧单测、构建或出包。五笔真实 Run 仍无文本成功。

## 开源对照后的首次远端成功（2026-11-26）

### 固定来源与版本判断纠偏

- 本轮实际下载并静态阅读以下源码，没有安装、执行第三方实现或给它们提供凭据：
  - [D3Dream client.go](https://github.com/D3Dream/cursor2api/blob/ea652189aca4d3e244a126dffedd35116695d9a9/internal/cursor/client.go)：
    `x-cursor-client-type=cli`，默认版本 `cli-2026.07.23-e383d2b`，处理上下文与 KV 双向消息。
  - [AstroQore session.ts](https://github.com/AstroQore/cursor2api/blob/b5b994b98ae69211db3209dcda75319e6dc600b5/src/cursor/session.ts)：
    CLI 类型和 `cli-2026.08.11-e8db854`，有 `request_context`／KV 握手，不能把其付费账号用例当 Free 验收。
  - [Oh My Pi wire/cursor.ts](https://github.com/can1357/oh-my-pi/blob/0ba99d7b351c25478859ace113b24b1aefec07e3/packages/catalog/src/wire/cursor.ts)：
    CLI 类型、`cli-2026.09.02-c22c1a3`，实际 provider 直连 HTTP/2，使用 unary protobuf。
  两个仓库的 `OUTDATED_CLIENT` issue 查询页没有匹配项；这不证明该问题不存在或已有专门修复。
- 读取官方 `cursor.com/install` 当时指向 **`2026.11.22-e9042b2`**。只下载该版本 Linux x64
  发布归档并静态读取 JS，未运行安装脚本、CLI 或 SDK。
  归档 SHA-256：`7663b1c5d1b56e095ef8e23bef38d66df6cfd665c4873470214e0e411f0f97b3`。
  第一方代码明确发送 `cli-2026.11.22-e9042b2`，常规 surface 为 `cli`；本次是兼容头声明，
  不是把 Gateway 软件升级为官方 CLI。User-Agent 仍为 `Neuro-Gateway-Cursor-Experimental/0.1`。
- 同一账号的只读对照：最新 CLI 格式版本分别搭配 `neuro-gateway` 和 `cli` 类型，
  配置均回显匹配版本但返回 `REQUIRED`，最低数字仍为 `3.12.0`／`3.18.0`。
  第一方静态代码的 ServerConfig 使用路径没有以 `clientVersionStatus` 阻止 CLI Run。
  随后真实成功证明：**这个配置字段不能作为本次 CLI 兼容路线的通用推理拒发判据**。
  不据此推导“所有版本检查都可忽略”，也没有通过伪造配置 `NONE` 来放行。
- `runSavedOnce()` 仅接受固定显式 `compatibilityExperiment=cliCompatibilityExperiment`
  （值 `cli_20261122_e9042b2`）。该实验在回执保留实际 `REQUIRED`，记录 `diagnostic_only`、
  未执行官方 CLI、单次上限及无回落；默认入口仍拒绝更新要求，未知实验名在读取凭据前拒绝。
  身份、exact 修订、端点、HTTP/2、模型与全部 Free／账源检查未放宽。

### 最小协议与传输修复

- 原 `AvailableModels` JSON 路径实测收到 HTTP 200／JSON 类型但不完整的正文；原生 `JSON.parse`
  也失败，其中一笔为 `Unterminated string`，不是严格 JSON 检查误判或认证材料丢失。
  未保存正文，也不能凭这些证据确定截断发生在上游还是传输中间层。
  改为第一方描述符对应的 bare unary protobuf，请求 #5／#7 为 true；成功读到 **49** 个参数化模型。
  新模块只投影选择校验所需字段，保留变体、别名、Max Mode、Agent 与数据保留门禁；
  不修补坏 JSON、不跳过模型校验、不失败后自动换格式重发。
- unary 传输显式协商 gzip，压缩前后均限制 512 KiB，校验 gzip 完整性；流式仍只接受 identity。
  网络错误增加固定 phase／Node code 白名单，不保存原始错误消息、栈或账号相关响应。
  gzip 后本轮目录预检成功，不宣称因此已证明此前所有截断的唯一根因。
- `wireRevision=4` 支持纯数据的上下文／KV 双向应答：空工具、空工作区、web search／fetch=false。
  只回显关联 ID；blob 仅请求内存，64 条／256 KiB、128 条控制消息、4 次上下文、320 KiB 应答上限，
  清理时清空缓存；不执行 shell、文件、网络、MCP、交互请求或任何服务端指令。
- 对照当前 schema 修正旧“消息只能有一个字段”的假设：`AgentServerMessage.#8` 是独立 timing
  元数据，`InteractionUpdate.#25` 是独立时间戳，`TextDeltaUpdate.#2` 是 server-notice 标记。
  合法元数据不再误报冲突；重复 oneof／错 wire 类型仍失败，server notice 不计入模型文本。
  诊断仅增加最多 16 个字段 tag 数字，不输出原始消息。

### 真实尝试与验收边界

- 本批两个预检失败回执分别是 `6343c3b3-ee34-4958-b80b-408907577592`（parameter JSON）和
  `0f13b3f0-6c4e-4ee9-ad02-53786fa6cd72`（CLI catalog JSON），二者 Run 均为 **0**。
- 三个独立单次探针：
  1. `98f61cea-bbd4-4557-9142-fdf4da8874dc`：本地传输尝试 1，`transport_failed`、0 帧，
     `delivery_unknown`。当时没有 phase 诊断，不能追述为某一确定网络阶段或已证明零发送。
  2. `254f4765-d60c-4429-9aed-20c9e76df8d2`：收到 6 帧后旧解析假设报
     `unexpected_protobuf_message`，无文本、交付未知。未保存原始帧，不能把具体附加字段回填为已观察值。
  3. **`34c0e8fb-7529-4fe6-a853-c4f8600a5e1b` 成功**：请求 ID
     `f04fae37-2e8d-465d-963c-d582c11f2370`，**44 帧／28 字节**，
     `replyMatchesExpected=true`、`turnEnded=true`、`endStream=true`、无 Connect error。
     固定验收答案为 `CURSOR REMOTE OK - 2 + 3 = 5`；原始生成内容只在内存中比对，不写回执。
  每个探针独立完整预检、最多一个 Run，无程序内重试、刷新或失败重放；没有以失败响应伪造成功。
- 成功回执为 `target/provider-evidence/cursor-remote-34c0e8fb-7529-4fe6-a853-c4f8600a5e1b.jsonl`。
  前后本人身份、个人 Free、按量 off／0、赠金／余额 0、空团队、个人按量禁止与无替代账源均通过，
  总百分比仍为 **0.5%**。**这些账号快照不等于请求级免费扣额证据**，实际模型仍未知。
  CLI 格式版本和类型作为组合实验通过；没有隔离证明究竟只有版本格式还是类型也为必要条件。
- 源码回归 **149/149**，包括真实 loopback TLS／HTTP2 双向握手、gzip 截断／扩容上限、
  新 schema 元数据／notice、protobuf 目录、KV 清理与所有旧准入／取消／不重放检查。
  权威有效行数 ratchet 通过；未改 Rust／桌面、未构建出包或切换 4315／4200。
  成功后只再收紧 TextSession 应答队列的总字节上限，并完成全套本地回归；该收紧未另发真实 Run。
  最终验证与逐文件 hash 见 `target/provider-evidence/cursor-remote-20261126-opensource-validation.json`。
  既有子代理工具通道 403 没有产生复审结果，不计作本批独立复审。
- **本阶段只完成独立探针的真实远端文本调用。** 下一步才是把已验证语义放入 Gateway 自有执行器，
  验收 Local Key／exact 账号／Responses 生命周期及真实 `codex-cli`，并核对可关联 Free 归属。
  不直接启用当前 import-only 账号，不把临时脚本或开源代理作为生产依赖。

## 真实 Codex CLI 正式入口试验（2026-11-26）

- 北京时间 **09:40:45–09:40:46**，使用本机既有 **codex-cli 0.155.1**，直接访问隔离
  `http://127.0.0.1:4315/v1/responses`，没有在 CLI 与 Gateway 之间添加模拟服务器或协议转发脚本。
  CLI 使用独立临时 CODEX_HOME、忽略用户配置、只读沙箱、关闭工具及 HTTP／stream 重试，
  不修改现有 Codex 配置或版本。该试验是正式入口的连通性诊断，不是正式发布或生产切换。
- 通过原管理 API 创建一个五分钟有效的普通 Local Key，只授权 `responses`、
  `models=["cursor-personal"]` 和 `providerIds=["cursor-remote"]`。该短期 Gateway Key 仅在父进程
  内存和子进程环境中传递，没有输出或写入新凭据文件；没有读取 Cursor 令牌。
  `cursor-personal` 是当前保留的私有公开模型名，不代表一个已存在的远端执行器。
- CLI 退出 **1**，观察到 HTTP **503**、`turn.failed`，无验收回复、无重连。
  Gateway 审计 `gw-audit-5c8192de-4cec-4f0b-9511-aa36e4442ef2` 确认已通过 Local Key 鉴权，
  endpoint 为 `responses`、错误为 **`cursor_acp_execution_unavailable`**、
  **`routeAttemptCount=0`**、`fallbackEligible=false`，账号归属及上游状态均为空。
  因而只证实 `codex-cli → Gateway`，**未接通 `Gateway → Cursor`**；不能将本地拒绝归因于上游
  版本、限额、网络或用户没有登录。
- 当前源码阻断与实测相符：`routing/config/cursor_remote.rs` 强制 import-only 并禁止模型路由，
  `provider_compilation.rs` 不向通用出站 payload 投放此类令牌；旧 `cursor_identity` 依赖未安装的
  `lifecycle.cursor_acp`，只接受 ChatCompletions，`stored_create.rs` 对保留名提前拒绝。
  已验证的 `tools/cursor_remote` 仍是独立实验，不能因一次成功而视为上述执行器已实现。
- 下一步需要将已验证远端协议接入 Gateway 自有执行器，完成 exact 账号授权与修订检查、
  Responses 输入及事件终态映射、取消／截止期限／不重放和真实发送审计后，再重验同一正式入口。
  不以取消 import-only 校验、恢复旧本地 ACP 或生产调用临时探针作为替代。
- 本轮只新增试验 Key 和失败审计；结束经管理 API 撤销 Key 并读回 `revoked`，没有删除记录。
  前后配置修订均为 `r2-1664e9a2f8ee`，原账号调用和后台探测继续禁用；无 Cursor 真实 Run、
  无 Rust／前端修改、无构建出包或实例切换。
- 脱敏回执：`target/provider-evidence/cursor-gateway-codex-d6f24808-07b5-4551-a243-d2807bdce3e0.json`。
  一次性运行器位于本机 `linshi/cursor-opensource-20261126/gateway-codex-current-once.mjs`，
  不是生产依赖或长期测试设施。

## 已核实

- [Cursor SDK Bridge](https://cursor.com/docs/sdk/bridge) 提供本地 Connect/protobuf Agent 协议，有官方 Windows x64 二进制与对应版本的 schema。Cursor API Key 与 Bridge 进程的 loopback bearer 是两种凭据，不可混用。
- [Cursor API 总览](https://cursor.com/docs/api) 明确区分 Agent 工作流与独立模型推理／Chat Completions。官方 Bridge 是原卡可评估的 Connect 路线，不证明旧卡的具体线协议或通用模型转发已经接通。
- [Cursor 免费档](https://cursor.com/pricing) 有受限 Agent 请求。[SDK 计费说明](https://cursor.com/docs/sdk/typescript#usage-and-billing) 说明用户 Key 的用量归该用户的套餐；真实 Free 账号能否列模型、创建本地 Agent、消耗正确的免费请求池，分项待验。不能凭有免费档、能创建 Key 或 Bridge 健康就标注免费可用。
- Cursor 支持在 [2026-08-22](https://forum.cursor.com/t/agent-create-local-apikey-model-fails-for-free-users-because-local-model-validation-calls-cloud-v1-models/160839/12) 确认当时本地 SDK runtime 仍要求 Pro；未找到已解除该限制的同等证据。这不等同于 CLI／ACP 的套餐资格。[CLI 文档](https://cursor.com/docs/cli/using) 公开 `agent acp`；若目标是 Free，先用正常登录的 Free 账号验证这条路线是否能完成一次请求并正确归入免费额度。

## 社区项目与首批范围

- [cursor-sdk2api](https://github.com/Sunnyender-org/cursor-sdk2api) 证明有项目用公开 SDK 包装 Chat Completions／Responses／Messages；[cursor-api-proxy](https://github.com/anyrobert/cursor-api-proxy) 使用官方 CLI。仅参考请求、流和终态映射，不移植第三方代理或账号池，也不把其 README 当 Free 额度回执。同名 `cursor2api` 的私有传输或文档助手路线不作为上游。
- 上游是 Agent 不代表下游只能走管理 UI。可用 Gateway 既有 Local Key 与专用路由，把官方运行时结果适配为单轮文本 Chat Completions（JSON／SSE）。仅限本人、显式授权至具体 Cursor 账号；不加公共目录、共享账号池、静默换账号或跨 runtime fallback。Bridge 与 CLI／ACP 若推进，分别验收。
- 固定经校验的 CLI 或 Bridge 版本与协议；按账号隔离进程与工作目录，禁止继承宿主凭据、MCP、Hooks 或其他项目配置。没有验证文件、命令与网络隔离前不开放执行；空工作目录及 `ask` 均不能替代进程隔离。

## 本机预检与实施顺序

1. **先查额度，再发请求。** 本机 CLI `2026.08.11-e8db854` 的 `status` 已显示登录；`about --format json` 没有给出可确认的套餐值。启动 `agent acp` 后，`initialize` 实测返回 ACP v1，声明 `cursor_login`。未发送 `session/new` 或 `session/prompt`。关闭 stdin 后 3 秒仍未退出，探针终止自有进程，随后未发现残留 ACP 进程。正式运行时不能依赖 EOF 回收。
2. **首个真实验收。** 在 Cursor 用量页确认 CLI 会话对应本人 Free 账号、额度可用且无付费回落；用官方 ACP 在隔离目录发一个固定的无工具文本请求。记录最终 `stopReason`、可关联的请求证据及用量归属。若用量页未能确认，标为“调用成功、免费扣额未证”，不放开默认路由。
3. **Gateway 实现。** 参考 `src/pipeline/copilot_identity.rs` 的同记录 Local Key 校验及 `src/pipeline/copilot_dispatch.rs` 的单次发送与 attempt gate，不复用 Copilot 的 token 类型或把 Cursor 登录态当成普通上游 Key。在 Local 模式冻结本次路由修订与目标账号，启动后发送前复查权限；拒绝共享账号池、Hosted/Server 与隐式切换。
4. **协议与进程。** 新建 Cursor ACP owner；使用固定的本机可执行路径和参数，不经调用方指定 shell；每个请求创建新会话。按官方 [ACP 合同](https://cursor.com/docs/cli/acp) 协商版本，关闭客户端文件、终端和 MCP 能力，确认 `ask`、拒绝任何权限请求，以 `agent_message_chunk` 和对应的 `session/prompt` 终态映射 JSON／SSE。限制帧、总输出、等待时间和并发；取消先发 `session/cancel`，超时回收自有进程树。截断不发成功终止帧；发出 prompt 后不重放。不伪造模型或 usage。
5. **验证。** 用合成 stdio peer 覆盖权限拒绝、错误帧、流截断、客户端取消、超时强退、账号撤销和零重放；通过后再验证 Gateway Local Key 到真实 Cursor 的 JSON／SSE。离线、真实回复、免费计费及打包分别记录。

## 本机 Free 调用尝试（2026-11-25）

用户已确认其账号为本人 Free 账号且仍有额度。本机沿用已安装 CLI，ACP 初始化通过；复用本机登录时 `session/new` 返回 JSON-RPC `-32000`（`authentication required`）。显式 `authenticate(cursor_login)` 未在 65 秒整体探针期限内完成；随后 `cursor-agent status --format json` 返回 `unauthenticated`，没有访问或刷新 token。先前 CLI 显示已登录的结果不能作为当前凭据证据。

没有发送 `session/prompt`（0 次）；无真实回复或扣额回执，不判定 Free 套餐不支持 ACP。探针进程已回收。待操作者通过官方 `cursor-agent login` 重新登录同一本人 Free 账号并完成浏览器确认，再重跑单次文本探针。

用户完成浏览器授权后，CLI 返回 `authenticated`。北京时间 **2026-11-25 09:22:14** 在 `ask` 模式发送 1 次 `session/prompt`，收到 12 个文本分片及 `stopReason=end_turn`，完整回复 `CURSOR ACP OK - 2 + 3 = 5`。本次未报告模型 ID 或 token 用量，没有观察到工具活动；关闭 stdin 后仍需终止自有进程，复查 ACP 子进程为 0。该回执证明本人 Free 账号的 CLI／ACP 调用链有效；计费池和实际扣额仍须在 Cursor 用量页核对。

用户随后确认该次请求计入免费额度。这是本人账号用量确认，不是 Gateway 自动读取额度的证据；没有模型 ID、剩余额度数或通用可用性结论。

## Gateway 实现进度

- `src/protocol/cursor_acp.rs` 实现有界 JSON-RPC 帧、ACP v1 握手、明确进入 `ask`、同一会话文本分片与 `end_turn` 终态校验；只允许一次 `session/prompt`，权限请求一律拒绝，工具活动、错误帧及截断不能作为成功。
- `src/protocol/cursor_acp/chat.rs` 在 canonical 与原始请求两侧限制为单轮纯文本 Chat Completions，允许 JSON／SSE 选项，不静默接受历史、工具、媒体或用量选项。
- `src/protocol/cursor_acp/transport.rs` 接管单轮异步 stdio，在 `ask` 确认后、写 prompt 前调用授权检查；区分零写入、部分写入、完整帧写入（不是远端回执）。取消、超时与协议失败时仅在完整帧边界尝试发送取消；不重放，进程回收由执行环境负责。ACP 使用固定 Linux guest 路径 `/tmp/work`，不以 Windows 宿主路径语义校验。
- Local Key 的 Cursor 专用复核从同一 SQLite 记录匹配当前密钥摘要、有效状态、端点 scope、字面 `providerIds` 与模型；不展开账号别名，且省略账号或模型不构成授权。实际发送前晚到的 lifecycle 变化仍须由 owner 核验；异步授权受请求取消和时限控制，首写每次轮询前都会重新检查同步条件。
- 只有经 `end_turn` 验证的 `CompletedTurn` 可以转换为 canonical JSON 输入或 Chat SSE 事件。SSE 保留原分片并在本轮结束后分发，不是边生成边输出；不虚构实际模型、用量或上游 HTTP 状态。
- `src/protocol/cursor_acp/owner.rs` 管理隔离执行生命周期。请求退出只通知监督任务，不中断可能已被 daemon 接收的创建或启动操作；取消后不启动已确认创建但尚未启动的容器。传输结束后关闭 stdio，再确认本地子进程与容器均已回收。无法确认时返回 `CleanupUnconfirmed`（保留 prompt 写入状态），隔离执行槽并只重查回收，不重建或重发。
- `owner/docker_offline.rs` 已绑定真实 Docker Desktop Linux daemon 的离线租约：固定镜像 ID、无网络、无宿主挂载、非 root；使用随机名称和标签登记临时容器，按完整 ID 无 TTY 启动，再按所有权标签和完整 ID 回收、读回。该离线租约没有凭据或网络配置入口，不是生产执行器。
- 定向 Nextest **23/23**（`1138ae6d-7350-454e-82ed-a9cb7517554e`，显式 `--run-ignored all`）：包含两个真实 Docker 测试，分别验证合成 ACP 对端完成一轮后即使不主动退出也会被回收，以及故意丢弃已返回 ID 后按唯一所有权找回并删除容器。测试后在同一 daemon 读回残留租约容器 **0**。ID 丢失测试不模拟 daemon 断连；这也不是 Cursor 上游、真实账号或 HTTP 端到端回执。
- 路由文档现在可声明一个 `auth_mode: cursor_acp` 的个人账号，须给出固定凭据 ID 和 `enabled: false`。不接收 URL、密钥、模型映射或后台探测；私有 `cursor-personal` 不会进入公共模型目录，声明存在时普通选路会在通配匹配前拒绝它，避免回落到其他供应商。三个新增配置测试覆盖有效／无效热加载、活动修订不变及禁用隔离；本轮 Nextest **24/24**（`92fa6429-422b-452e-9643-95b3efcea765`，未重跑两个 Docker opt-in 用例）。
- ACP transport 和隔离监督器已提供实际写入观察点，并适配现有请求预算与账号审计：在写 prompt 前预留预算，首个正字节写入才记录具体账号。握手、授权／预算拒绝及零写入不归属上游发送；部分写入记录一次但仍是交付未知，不重放。定向 Nextest **29/29**（`d6606add-e79e-409c-a912-7e4e76696bd4`，包括两个离线 Docker 用例和普通发送重试回归）。这不等于 HTTP 审计记录已端到端验证。
- 专用 Local Key 已接到实际 HTTP 路由、请求管线和终态审计。仅允许字面 `cursor-personal` 单轮纯文本，冻结路由修订，启动后在写 prompt 前复核同一 Key、账号与账号组；不走通用 HTTP、重试或回落，拒绝缺少可核对用量的现金预付。实例级运行时槽默认为空，路由配置不能启用，也未安装生产执行器。
- 定向 Nextest **28/28**（`a2c80206-cdd1-4e5a-8d77-b0175718a954`）：使用真实 Gateway HTTP router、隔离 SQLite Local Key 与合成 stdio 对端，验证 JSON／SSE、准确凭据归属、每次实际请求一次 attempt、无伪造上游 HTTP 状态或 token 用量、错账号零发送，以及没有实例运行时就拒绝且不回落。
- Gateway 有序关停会关闭 Cursor 执行准入、通知在途执行停止，并在关闭本地存储前等待确认回收。监督者意外丢失或 30 秒内无法确认时，关停返回 `Cursor ACP shutdown: execution cleanup unconfirmed`。该批定向 Nextest **33/33**（`075cdf66-aa91-4580-80b7-5b5455ca2b73`，含两个 Docker opt-in 用例、关停回收及本地存储回归）。真实账号验收仍须先验证受限出站和独立登录态；管理入口、真实免费扣额和整包尚未完成。
- 离线 Docker owner 新增可选独占恢复目录。它在提交创建前持久化 daemon ID 和唯一容器所有权，拿到完整容器 ID 后先持久化再允许启动；恢复只在同一 daemon 中按精确标签、名称和 ID 回收，并在确认不存在后清除记录。无创建回执且没找到容器时拒绝开新执行器，不将一次空查询当成无迟到创建的证据。恢复不搜索或删除其他实例的容器，不发 ACP 请求；目前还没有接入生产启动入口。
- 定向 Nextest **35/35**（`1f782a53-0b5c-4fd8-965c-8a420903135d`，含 Docker opt-in）。同一恢复目录的第二个 owner 被拒绝；丢弃旧 owner 而不调用清理后，新 owner 重开同一目录，回收真实 Docker 容器并允许下一次租约。另一个用例证明无回执且未查到容器时重复开启仍拒绝。这模拟监督丢失，不是进程强杀、daemon 重启或在线凭据恢复验收。
- 定向 Nextest **36/36**（`e4579439-f59e-497f-873b-ef6eceef0927`）进一步覆盖真实进程强杀：子进程确定容器为运行中后回报完整 ID，父进程按子进程句柄强制终止并确认失败退出，再重开同一恢复目录，以完整 ID 和精确所有权查询确认容器已回收。未杀生产 Gateway 或重启 daemon。

## Docker 候选预检（2026-11-25）

- 可复跑：`python tools/probe-cursor-acp-isolation.py`。可用 `--cursor-archive <官方 Linux x64 归档>` 加验固定的 `2026.08.11-e8db854`。探针先校验长度和实测 SHA-256；该哈希用于重现本次输入，不是厂商签名。探针只使用本地镜像，不下载或复制账号材料。
- 本机 Docker Desktop Linux Engine `29.5.3`：`none` 仅有 loopback；独立 `internal`/`isolated` bridge 没有默认 IPv4 路由。两者在显式本地 DNS 配置下均无法解析 `cursor.com` 和 `host.docker.internal`；普通 bridge 正对照能解析两者。合成容器为非 root、无有效 capability、`NoNewPrivs`、独立 PID namespace、只读根，无探针所列宿主共享路径或 Docker socket。
- 固定 Linux CLI 在 `--network none`、新 HOME、无宿主凭据下实际返回版本 `2026.08.11-e8db854`、ACP v1 及 `cursor_login`。仅发送 `initialize`；没有登录、`session/new` 或 `session/prompt`。只读挂载是公开的 CLI 安装归档，不是 Gateway 数据、项目目录或登录态。
- `python -B tools/build-cursor-acp-offline-image.py <归档路径>` 可由已校验的公开归档和固定本机基镜像构建无宿主挂载候选。构建上下文仅含归档、Dockerfile 和固定启动器；启动器清除继承环境，使用新建 HOME／工作目录、非 root 身份和固定 `acp` 参数。本机读回镜像 ID `sha256:2aac368e97cea7941e28f65a59d3c722f89a8d941f251f05cb8484040bee6d11`；在只读根、不可执行的 `/tmp`、无网络的容器内，通过固定版本、ACP v1 和 `cursor_login` 声明验证，探针容器无残留。它不含账号数据，只是后续运行时绑定的离线输入。
- 使用由构建器返回的镜像 ID 设置 `GATEWAY_TEST_CURSOR_OFFLINE_IMAGE_ID`，显式运行 `docker_offline_pinned_cli_initializes_over_owned_stdio`。真实 Docker lease 成功从固定 CLI 的 stdio 收到 ACP v1 和 `cursor_login`，然后按完整 ID 回收容器。Cursor 定向 Nextest **33/33**（`5a19e46f-fe3a-4eeb-ad73-ef445907e3f9`，`test(cursor_acp)`，含显式 Docker 用例）；未调用 `authenticate`、`session/new`、`session/prompt`。
- 固定 CLI 在全新 HOME 和禁网环境中，`status --format json` 实测为 `unauthenticated`；`NO_OPEN_BROWSER=1` 的官方 `login` 可生成浏览器挑战，但因无法出站未完成登录。Gateway 新增有界、单次的挑战解析器，只接受 `https://cursor.com/loginDeepControl` 与该固定版本的四项预期参数；不打开浏览器、不落盘、不在日志中打印挑战。定向 Nextest **40/40**（`7461467a-bab3-4a6f-a465-7b1172d54121`），含真实镜像输出验收。可生成挑战不等于可登录；挑战未交给用户，未发生账号授权或推理。
- `python -B tools/probe-cursor-acp-account-home.py <已核验的镜像 ID>` 验证专用 HOME 的存储候选：仅接受带精确归属标签、无附加挂载选项的本机 Docker 托管卷；不使用宿主目录挂载。隔离容器以 UID/GID `65532:65532` 使用 `0700` HOME。同卷先写合成标记，再用新容器读回；另一卷读不到标记。三个阶段均用真实 CLI 确认 `unauthenticated`；两个卷只存测试数据并保留待读回，没有生产令牌、跨 daemon 恢复或静态加密证明。生产接线不得将这个存储预检直接视为完成账号绑定。
- 离线 Docker owner 现在把 HOME 卷绑定到恢复目录：打开时核验该卷的名字、驱动和精确归属标签，启动前读回唯一的卷挂载并确认只读。同一恢复目录不能改绑另一有效卷。定向 Nextest **41/41**（`2c6a306c-3815-47c1-9e6c-54c63527ffab`）覆盖两个真实合成卷、不挂宿主路径、容器内只读证明和官方 CLI 未登录状态。它没有给运行容器写入登录态的能力，也未启用生产入口。
- 容器直接下载归档返回 `HTTP Error 403: Forbidden`；宿主从官方 URL 下载公开归档成功，仅用于离线探针。网络预检不是运行时鉴证：尚未证明选择性出站能让官方 CLI 完成登录和调用，同时继续阻断宿主、局域网和其他目标。在此之前不把 Cursor 加进可用路由，也不把普通 bridge 当作生产配置。

## 受限出站预检（2026-11-25）

- `python -B tools/probe-cursor-acp-egress.py` 在一次性 `internal` / `isolated` bridge 上启动 worker。双网卡 CONNECT relay 仅监听隔离侧，启动时检查 IPv4 / IPv6 转发关闭；只接受 `cursor.com:443` 和 `api2.cursor.sh:443`。DNS 结果须全部为公网单播 IPv4，连接只用已校验地址；不放行保留网段，不记录请求头或 TLS 内容。
- 本机实测所有拒绝项通过：worker 无公网 DNS、宿主别名 DNS、直接公网或 bridge 网关路由；CONNECT 拒绝回环、宿主别名、局域网、其他域名和端口。单元测试覆盖 DNS 混合结果禁止连接、准确拨号、拒绝不拨号及双向传输。
- 两个真实 TLS 正例都未通过：Windows 和 Docker 当前 DNS 将两域名解析到 `198.18.0.0/15`，relay 在 TCP 拨号前拒绝为 `non-public-dns`。该结果不能证明 Cursor 拒绝账号登录或 Free 套餐；本次未运行 `agent login`、未创建会话、未发送请求。未改宿主 DNS、代理或现有 Docker 网络。
- 探针现在要求初始 TLS ClientHello 的明文 SNI 与 CONNECT authority 一致；拒绝无 SNI、歧义扩展、ECH／ESNI 和超限帧。通过 DNS 校验后也要先检查 ClientHello 才拨号。定向 Python **11/11**，含分片 ClientHello、错 SNI 零上游拨号和原字节转发；真实 Docker 复跑仍是 9 项拒绝检查通过、2 项 TLS 正例为 `non-public-dns`，探针容器和网络无残留。合成 TLS 测试不能代替真实正例。
- `python -B tools/probe-cursor-acp-cli-proxy.py <已核验的镜像 ID>` 在 `--network none`、全新 HOME 中把固定 CLI 的 `HTTP_PROXY`／`HTTPS_PROXY` 指向同容器的拒绝测试桩，并设置 `NODE_USE_ENV_PROXY=1`。真实 `agent login` 对测试桩发出 `CONNECT api2.cursor.sh:443`，收到 `403` 后退出 `1`。仅记录 CONNECT 域名；测试桩没有上游连接，没有公开授权链接、登录、推理或扣额。这个用例只证明被拒绝时观察到的登录目标和代理兼容性，不证明成功路径仅需该域名。
- Windows 代理复测：`python -B tools/probe-cursor-acp-egress.py --windows-proxy-port 42344`。只有 relay 使用 `host.docker.internal` 上的指定 HTTP CONNECT 代理；worker 仍只有隔离网络。relay 在校验精确 CONNECT 目标和初始 SNI 后才建立上游连接，重新生成 CONNECT、丢弃客户端头，不回落直连。实测两个经证书校验的 HTTPS HEAD 均为 `200`，10 项隔离或拒绝检查均通过；定向 Python **13/13**。
- 二级代理模式把目标解析交给操作者选择的 Windows 代理，并不证明其解析结果或运营方策略；直连模式仍拒绝 `198.18.0.0/15`。首个明文 SNI 匹配也不等于约束了加密后的 HTTP authority 或后续握手。此 relay 仍只是探针，未经应用层目的地约束、专用登录态和跨重启回收验证，不接入生产路由。此次未执行 CLI 登录或推理。

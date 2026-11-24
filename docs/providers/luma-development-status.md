# Luma 当前状态与剩余工作

## 最新推进（2026-11-21，Web 会话续期与轮换持久化）

[本批回执](../progress/2026-11-21-luma-web-session-persistence.md)：已接入 Web 认证 GET 维护、成功或失败响应的 Cookie 轮换保存、跨重启复用和不重放边界。定向 Rust **92 passed / 3 ignored**、Node／真实 Edge **39 passed**、Rust → 真实 Edge 回环和隔离 Redis 各 **1 passed**。已交付 `gateway-devtest-20261121-luma-web-session`，包完整性与隔离原生启动通过（不等于原生业务点击和真实账号验收）。

**按本轮范围，已知剩余开发项为 0，另有 1 项暂缓未实现（MCP 分层）。** Web 已完成实现和本地合成验证，但真实账号的 Gateway 自动续期／重启复用未验，不以官网现有会话记录或合成测试代替。Web 真实成功生成及依赖成功受理的完成事件／媒体／远端取消按已确认额度拒绝跳过，不计实际通过。

## 前批推进（2026-11-21，Web 身份与分层预算）

[本批回执](../progress/2026-11-21-luma-web-identity-and-layering-budget.md)：修复 Luma Web 账号 User-Agent 在 Rust → 本机／远端 browser worker 链路中丢失，修复前 1 项回归失败，修复后相关 Rust **60 passed**、真实 Edge loopback **2 passed**、checker **31 passed**，fmt、ratchet、diff 检查通过。只完成源码与定向验证，未新打包；下面的最新已交付 EXE 不包含这项修复。

在该批截止时，尚有 **1 个需继续实现的功能包：Web 自动续期及 Cookie 轮换持久保存**。原 Edge 录制新增证明 `GET …/connectors` 的 200 响应会轮换 `wos-session`；该批尚未接入 Gateway 持久化；随后状态见顶部。

另一个原有未完项 **MCP 分层暂缓，未实现／未验收**：正常管理刷新已成功保存，重新取得的 26 项工具合同未变，仍缺布局 schema；真实 MCP 余额为 5 credits，联网核实的官方分层最低价格为 75 credits（1k）。一次预算 5 credits 的分层调查调用返回未保留明细的 JSON-RPC 错误、没有 run ID；原会话只读补查记账为 0，未重发，不能将该次错误伪装成已确认 402。后续不再为了分层反复生成、要求充值或换账号；详情与公开价格原文见本批回执。

## 当前验收规则（2026-11-21 用户确认）

最新范围：用户进一步允许确因额度不足而无法测试的内容暂不开发。本轮已补齐并交付 **5 个明确功能点**：MCP 的 `approve`／`confirm`／`text_input`／`select` 显式 JSON 答复，以及 Web 凭据 `Keep` 的完整归属校验。Rust 定向 **58 passed / 0 failed / 4 ignored**、前端 **24 passed**、类型检查与规模门禁通过，见[明确缺口回执](../progress/2026-11-21-luma-explicit-gaps.md)。新增源码已进入 `gateway-devtest-20261121-luma-complete-gaps`，6154 项包校验记录、源码指纹、EXE 哈希与隔离桌面／SQLite／管理页启动通过，见[最新交付回执](../progress/2026-11-21-luma-gaps-delivery.md)。原 `gateway-devtest-20261121-luma-handoff` 保留，不包含这 5 项增量。

上述 5 项开发及交付缺口已清零。最初保留的 **2 个未完功能项**，现按顶部最新证据处理：MCP 分层因当前预算无法取得成功样本而暂缓；Web 会话自动续期及轮换持久化由顶部续批实现和验证，真实账号未验。Web 成功生成及依赖成功受理的完成事件／媒体／远端取消按已有 402 证据暂缓，不重试。

用户要求：确认因没有额度而无法继续的测试可放行，不再反复测试。原 Edge 的一次创作 POST 已返回 `402 / Not enough credits`，因此该 Web 真实生成项记为 **用户接受跳过（额度不足）**，从本轮交付阻塞中移除；不继续试生成、催充值、换账号补样本或重跑已通过的本地回归。它不计入实际 `passed` 数，不伪造输出 ID、完成事件、媒体、远端取消或扣费。

现有已实现范围已交付至新包 `gateway-devtest-20261121-luma-handoff`：MCP 18 个模型的创作／编辑／复用／恢复／review、Local／Server 持久管理任务、Agents 自动追账及 Web worker 修复。另修复部署载荷遗漏 004／005／006 SQL，20 项打包相关回归通过；desktop devtest 的 6152 项校验文件、EXE 哈希、源码指纹和隔离 SQLite 桌面启动通过。新增包与结果见[额度放行与交付回执](../progress/2026-11-21-luma-credit-waiver-delivery.md)。

| 工作包 | 当前处理 |
| --- | --- |
| L1 | 图片／视频创作、费用、原任务恢复、review、四种交互显式答复和 Server 管理任务已验证并交付；MCP 分层布局仍未实现，按顶部预算证据暂缓。任意 client tool 不纳入本次扩展。 |
| L2 | Web `Keep` 归属校验已验证并交付；会话续期和持久写回已完成实现与合成验收；真实账号续期未验。已确认额度阻塞的成功生成链及依赖该链的远端取消暂缓。 |
| L4 | 已实现的原账号 GET 补账、SQLite／PG CAS 和 Server 强杀恢复按既有回执收口；不重复验证，不虚构媒体 credits 与 USD 的换算。 |
| L6 | 复用真实只读目录、管理网页、Server／PG／Redis／强杀证据；包含上述 5 项增量的最新包及隔离启动已通过。原生业务点击未新增验收，额度放行不改写其他渠道的历史结论。 |

下方为历史开发记录。其中“必须等额度才能继续”的本轮停止点由上述用户决定覆盖；缺失功能及渠道合同仍如实保留。

## 历史开发记录

2026-11-21 [Web 额度拒绝的 Rust 分类补齐](../progress/2026-11-21-luma-web-payment-classification.md)：固定 worker 错误码与 402 同时出现时，按 `InsufficientQuota` 返回固定提示，保留状态并禁止重试／换账号；无正文不再归为 `Unknown`。先红后绿，58 项相关 Rust 回归和规模门禁通过。本机原 Edge 页面仍显示 Free、`5 of 5 left`，未新增生成或核实账单；L2 成功业务链及四包整体仍未关闭，未打包或部署。

2026-11-21 [已按确认在原 Edge 提交一次真实单图](../progress/2026-11-21-luma-web-existing-edge.md#一次真实提交上游拒绝credits-不足)：`POST …/actions` 路径和 `Uni-1.1` 请求字段已取证；唯一提交为 `HTTP 402`（`Not enough credits`），未取得成功输出 ID、完成事件或最终媒体。Gateway worker 现为 402 返回明确的支付／额度错误，不读取正文、不重试。没有充值或再次提交，实际扣费未独立核实；L2 核心链仍未验收。

2026-11-21 [复查同一 Edge 的事件流与同源 POST](../progress/2026-11-21-luma-web-existing-edge.md#eventstream-与同源-post-复查)：6 条 `grip` 的可读快照共 146 条消息，均为 `ping`。按方法筛查录制，31 条 Board POST 均匹配 `text/x-component`，没有取得同一创作的提交回执、完成帧和最终媒体。没有生成、扣费或改动生产逻辑，L2 核心链仍未验收。

2026-11-21 在同一本机 Edge [续查签名、SSE 和历史资产](../progress/2026-11-21-luma-web-existing-edge.md#再续查签名结构sse-响应类型board-资产回读)：确认 worker 使用的 `cdn_url`、`query_params`、`ws_token` 在 `signature` 抽样中均为非空字符串，同时观察到 `ws_url` 和可解析的 `expires_at`；`grip` 响应端明确为 `text/event-stream`。既有 Board 的批量读回返回 `found_artifacts`、`missing_artifact_ids`、`tombstones` 数组；它不提供原创作受理证明。追加录制仍未发现 `/actions`、`/tasks` 或 `/generations` 样本，未得到同一任务的提交、完成帧与最终媒体关联。没有修改生产逻辑或执行真实生成。

2026-11-21 继续使用用户本机已登录的原有 Edge。[L2 只读回执](../progress/2026-11-21-luma-web-existing-edge.md)已确认同一 realm 的 `menus.json?v=1`、`signature`、`events/stream/grip` 和 `artifacts` 均见 `GET 200`，图／视频／音频默认菜单 ID 与现有 worker 一致，另读到既有 `source: "task"`、`state: "completed"` 图片资产。用实测菜单形状补了 4 个本地回归，相关 29 项通过。未取得创作 POST 或原任务的 SSE／媒体闭环，不视为完整 Web worker 验收；没有生成或扣费。

2026-11-21 通过 [L2 生产 worker 的真实 Edge 回环验收](../progress/2026-11-21-luma-web-worker-edge.md)：正常路径找到原完成产物；真实 307 被识别且未重发；两条路径都关闭 SSE。没有改动生产逻辑。该批尚未接入已登录官网，只关闭本机真实浏览器验证切片。

2026-11-21 推进 L2：[Web worker SSE 修复](../progress/2026-11-21-luma-web-sse.md)解决混合换行导致同一任务的完成事件漏判，跨 chunk CRLF 不会让未终止事件提前完成。40 项 Luma Node 回归通过，包括实际 loopback Fetch 仅提交一次并关闭订阅。浏览器桥接仍未取得已登录官网证据；本地修复不关闭 L2 真实业务链。

2026-11-21 按 L1／L4／L6 先做、L2 最后的顺序推进。[MCP 创作正常管理网页联合验收](../progress/2026-11-21-luma-mcp-creation-browser.md)已通过：未确认费用不生成，确认后只提交一次；重开面板后以原任务 ID 查回结果、链接与合成会话 credits，同一 SQLite 审计行独立读回。L4 复用已有强杀补账证据；不将本地联合验收视为真实收费、远端取消或 L2 当前官网证据。

2026-11-21 修复 L2 现有 Web worker 的单次提交边界：创作 POST 不自动跟随重定向，遇到浏览器 `opaqueredirect` 返回 `lumalabs_action_redirect`，明确提交未确认，不重发或按成功返回，复用原事件流清理。定向夹具修复前失败、修复后通过；`node --test scripts/tests/lumalabs-browser-operation.test.mjs scripts/tests/lumalabs-task-contract.test.mjs` 共 23 项通过。本轮未取得已登录 Web 的协议证据，不宣称当前官网兼容或真实业务通过。

2026-11-21 补齐 [Server MCP 管理 TCP 强杀恢复](../progress/2026-11-21-luma-mcp-server-restart.md)：数据库租约自然到期后，以原管理令牌和新 grant 恢复同一 run；回执前被强杀的提交保留 `unconfirmed`，两者均不重发。费用验证仅为合成会话 credits。

2026-11-21 新增 [Server MCP 持久管理任务](../progress/2026-11-21-luma-mcp-server-journal.md)：隔离 PostgreSQL／Redis 的正常路由验证通过，覆盖原任务恢复、管理令牌绑定、PG 租约和并发仅提交一次；与公共 Agents 审计分开。真实账号生成、取消、媒体和原生交付未在本批验证。

2026-11-21 追加验证见[托管凭据强杀补账](../progress/2026-11-21-luma-hosted-reconciliation.md)：普通公共图像请求的 PostgreSQL 托管账号已通过原任务补账与换 Key 拒绝的合成验证。

2026-11-21 续做见[四个剩余工作包进度](../progress/2026-11-21-luma-four-continuation.md)：新增同 run 会话审阅、Web 音视频结果 URL 准入和已通过的 Server／PostgreSQL／Redis 合成强杀补账验证；不宣称四包整体关闭。

核查日期：2026-11-20。MCP 账号已取得当前 26 项完整工具合同；经用户明确允许的一次显式刷新并保存后，正常管理 API 的 18 项模型列表与全部 18 项模型详情真实读取通过，见[模型接入子批](../progress/2026-11-20-luma-mcp-models.md)。已有合成验收、5 GiB 流式上传与持久追账结果保留。没有运行供应商真实收费业务。四条渠道分别核算，R14、P50/P60/P70 尚未整体关闭，不减少 Magpie 67 项。

## 本轮六项收尾（2026-11-20）

用户随后明确重新启动 L1–L6，并授权读取已有 MCP／Web 账号；收费生成仍不执行。以下新进度覆盖原暂停决定，六项总范围不增加：

| 编号 | 本轮实现与边界 |
| --- | --- |
| L1 | [Local 显式创作](../progress/2026-11-20-luma-mcp-creation.md)包含独立会话／工作区、单次提交、原任务查询、取消／结束、资产、费用与持久任务；[素材复用](../progress/2026-11-20-luma-mcp-workspace.md)支持原工作区连续创作；[模型子批](../progress/2026-11-20-luma-mcp-models.md)扩至全部 18 个目录模型；[配置变更恢复](../progress/2026-11-20-luma-mcp-recovery.md)支持原任务访问；[review 答复](../progress/2026-11-20-luma-mcp-review.md)新增原内容／素材展示、显式 JSON 答复、持久单次提交与会话游标读取。本批补齐同一原 run 的 `session_await.waiting` 审阅；Server 已接持久管理任务和管理令牌绑定；分层布局、其他交互与真实业务仍未关闭 |
| L2 | 已加固本地／远端 worker 共用的音视频 HTTPS 签名链接准入。原 Edge 已登录；真实读取 Board、资产列表、图／视频／音频菜单，核对默认菜单 ID 及同 realm 签名和事件流路径。一次真实单图已确认当前 POST 路径和发出的负载；上游 402 拒绝，未验证成功回执、会话生命周期、原任务的 SSE／签名媒体关联及实际费用或远端取消。Web 核心链未关闭，不要求用户重登 |
| L3 | 签名上传扩至 5 GiB；超过 64 MiB 使用有背压的 multipart → PUT 流，精确长度／结束边界校验，取消停止两端，不写临时大文件；官方单 PUT 合同未承诺 range／multipart 续传，因此不提供断点续传 |
| L4 | 增加原审计行 checkpoint、绑定原账号和有效材料的自动 GET 追账、SQLite／PG CAS 领取和费用回填；不改原请求结论／本地余额，不重提生成。独立 PostgreSQL CAS 回归与 Server／PostgreSQL／Redis 合成强杀均已通过，覆盖路由文档和 DB 托管原账号补账，且换 Key 后不发任何恢复请求；托管路径需要稳定的 `GATEWAY_API_KEY_SECRET`，不覆盖真实收费 |
| L5 | 新桌面 devtest 包 `gateway-devtest-20261120-luma-mcp-options` 已交付，覆盖基础 MCP 创作及本批参数／编辑；6129 项文件完整性、源码指纹、EXE 哈希和隔离桌面→SQLite→管理页启动通过，测试进程已收尾。旧包保留，原生真实业务点击另行验收 |
| L6 | MCP 账号的目录、18 个模型列表／全部详情正常管理 API 真实读取通过；本批一次显式刷新保存成功，真实生成仍暂停。合成 Server 普通图像入口到 PG／Redis 强杀补账已验，包含路由文档、DB 托管 Key 和换 Key 负对照；另通过 MCP 管理 TCP 强杀验证（已知 run 恢复、未知提交不重发）。真实 Web worker、原生业务点击及收费业务均不因此关闭 |

当前实现、实测结果与限制记录在[本轮六项回执](../progress/2026-11-20-luma-six-closeout.md)；长期脚本位于 `tools/luma/`，测试位于对应源码模块，公开合同与验证日志归档在 `docs/evidence/luma/gateway-luma-six-20261120/`。

本轮可收口的是 **L3 的已声明本地实现与 L5 的包交付**；L4 的 SQLite／PG 持久化和 Server 进程强杀切片已验证，包含路由文档和配置了签名密钥的 PostgreSQL 托管凭据；适用的真实收费验收仍未完成。L1、L2、L6 未关闭；本地合成验证不等于四包整体交付。没有新合同或失败证据时，不继续追加本地优化。

## 原阶段收尾决定（后续六项已明确重启）

用户明确要求避免无限优化，能收尾就先收尾。当前按**已验证支持范围的源码阶段收尾**处理，结束连续扩展；不把 R14 全功能、最新原生包或真实业务标成完成。已有有效证据复用，不再为相同路径追加目录工具、预览设施、重复回归或通用恢复系统。

本阶段支持边界固定为：Agents 已实现的图片／视频操作、≤64 MiB Files、任务／费用事实和手工单任务查询；Inference 已实现的文本与无状态协议桥接；MCP 授权／显式刷新及只读目录／模型管理。Web 现有单图合成结果保留为部分成果，不纳入完整业务就绪声明。

完整 Luma 尚余以下 **6 个工作包**，这是工作范围计数，不是六个等量小任务或完工百分比：

| 编号 | 尚余工作 | 收尾处理与重新启动条件 |
| --- | --- | --- |
| L1 | MCP 创作：会话／工作区隔离、生成、等待／取消、资产和费用 | 核心业务未完成；取得当前完整工具合同后继续，不反复进行相同公开调查 |
| L2 | Web 当前业务链：可靠会话／wire／模型动作、真实 worker 和媒体资产 | 核心业务未完成；取得可用当前协议／录制后继续，不以旧合成成功代替 |
| L3 | 超过 64 MiB 的 Files 与相应上传恢复／隔离扩展 | 后置增强，当前保留明确上限；出现实际大文件需求再启动 |
| L4 | 自动追账、审计回填、强制崩溃／跨进程恢复及适用 cash | 后置增强，当前保留有界补查、事实审计与手工查询；需要自动财务闭环时再启动 |
| L5 | 覆盖当前源码的原生 devtest 包、包完整性与隔离启动 | 独立交付项，尚未完成；交付阶段仅运行已有打包流程和必要检查 |
| L6 | 集中真实账号业务，以及承诺范围内的 Server／PG／Redis／进程边界验收 | 独立验收项，尚未完成；有对应材料／环境／费用预算后按已声明能力集中执行 |

L3／L4 的后置不删除总计划承诺；L1／L2 也不转换为“已完成”。没有新缺陷、当前协议证据或明确启用上述工作包时，后续一般“继续”应回到 R01–R16 其余渠道的明确未完成项，不自动重开 Luma 优化。既有范围内出现实际缺陷时，只修复该缺陷并做相关验证。

## 已有成果

最新[工具合同快照子批](../progress/2026-11-20-provider-channel-expansion-luma-mcp-snapshot.md)补齐已保存 MCP 账号成功目录的 JSON 下载，保留完整已接收输入／输出 schema 与 annotations，排除连接身份字段；范围变更／重读／取消／失败后旧结果失效。后端 **27 passed / 1 ignored**、前端 **24 passed**，正常网页实际下载及宿主 **1 passed**；导出不增加管理或上游调用。仍未恢复真实当前目录，不代表 MCP 创作完成。

最新子批：[单任务查询正常网页联合验收](../progress/2026-11-20-provider-channel-expansion-luma-task-browser.md)已通过正常登录、取消审计展示／复制任务 ID、原账号查询、未知／0／12.5、401 保留管理会话、429／错 UUID 拒绝、关闭／切换清空与宽窄屏。六次管理请求与六次合成上游 GET 匹配，原审计／revision／普通 Key 余额不变；显式宿主 **1 passed**。此结果补齐[任务查询实现子批](../progress/2026-11-20-provider-channel-expansion-luma-task-lookup.md)此前仅组件浏览器与管理 HTTP 分别验证的缺口，仍未运行真实供应商业务。

任务查询实现子批的 Agents 定向回归为 **123 passed / 2 ignored**（新增查询六项全通过），前端 **32 passed**，生产／关闭功能编译、正常前端构建和类型检查通过。其组件浏览器覆盖未知／0／12.5、关闭重开清空与宽窄屏；后续联合验收未重复运行这些未变生产路径的检查，未运行真实供应商业务。

| 渠道 | 已完成的范围 |
| --- | --- |
| Agents API | 独立 Key、模板、保存/Keep/清空、图片生成/编辑、视频生成/扩画/重绘、图片分层；逐次确认的媒体测试 UI、资产校验、同 UUID 轮询、本地次数结算；成功及正常失败终态的有界晚到 credits 补查；取消／超时／资产失败时保留已验证任务和费用事实；已保存账号内的 ≤8 MiB 内联／≤64 MiB 签名 Files 上传、TTL/user_id、分页、状态查询、确认删除与 ready 文件 ID 复制 |
| OAuth/MCP | 正常注册、授权保存、显式离线授权和刷新、工具目录／JSON 合同快照、模型列表/详情管理与 UI。11 月 14 日回执记录真实单次刷新、前后 26 项工具及同数据目录正常原生重启恢复 |
| Web board | 单图任务/worker、公共图片、精确图片测试、合成 executor 联合、本地任务绑定和取消释放 |
| LLM Inference | 独立销售 Key/固定域名/空池模板；Chat、无状态 Responses/Messages 桥接、function tools、JSON/SSE、显式账号测试、本地用量审计与次数结算 |

最新 [Files 签名上传子批](../progress/2026-11-20-provider-channel-expansion-luma-presigned-files.md)后端 **16 passed**、前端 **31 passed**，正常前端构建、类型检查、生产／feature-off 编译与规模门禁通过。未运行真实供应商业务。

[Files 正常网页联合验收](../progress/2026-11-20-provider-channel-expansion-luma-files-browser.md)另显式运行 **1 passed**：正常登录／授权、上传、状态／复制、分页、账号切换清空、TTL/user_id、取消及确认删除；7 次管理请求与9次合成上游调用匹配，宽窄屏已检查。持续验证入口在仓库，真实平台仍未验。

[Inference 正常网页联合验收](../progress/2026-11-20-provider-channel-expansion-luma-inference-browser.md)另显式运行 **1 passed**，前端相关 **12 passed**：配置／关闭不调用、显式执行只发送一次、映射模型、精确账号、可见答案、2/3/5 token 审计、普通 Key 余额和 revision 不变，宽窄屏通过。修复了账号测试弹窗执行后不显示结果的问题。

[MCP 只读目录正常网页联合验收](../progress/2026-11-20-provider-channel-expansion-luma-mcp-browser.md)显式宿主 **1 passed**，后端回归 **26 passed / 1 ignored**，前端 **33 passed**。6 次管理请求、18 次 MCP POST 与6次 DELETE 匹配，覆盖工具文本安全展示、image/video 参数、audio 详情禁用、切换清空、空列表、分类错误和宽窄屏；账号与 revision 不变。修复授权拒绝时未处理 Promise 的问题。

前批[取消审计子批](../progress/2026-11-20-provider-channel-expansion-luma-cancellation.md)的 Agents 回归为 **110 passed / 1 ignored**，覆盖原 Files 与六项新增用例；共享审计生命周期相邻 **31 项通过**。Files 前一批前端 **44 个不同测试通过**，见 [Files 回执](../progress/2026-11-20-provider-channel-expansion-luma-files.md)。Inference 19 passed、前端 108 项未在本批整体重跑，仍见[历史归档](../evidence/luma/gateway-provider-resume-20261119/verification.json)。这些均不代表供应商实际业务验收。

## 尚有实现缺口

1. **Agents Files 后续范围**：5 GiB 流式签名上传已实现；真实账号文件隔离、摄取、复用、删除与 TTL 未验。没有官方可恢复 PUT 合同，不提供断点续传、终端用户 ACL 或跨进程重发保证。
2. **媒体费用收尾**：已保存任务在原请求终止后可自动补查并回填原审计，最多 12 次、每次 30 秒，间隔最多 64 分钟。未知保持未知，排队时的零费用仍等终态；路由文档的 revision 不匹配，或托管账号归属、Key／签名密钥不匹配时，停止自动查询。SQLite 复用实例心跳恢复；Server 超过 15 分钟的 running checkpoint 可读取费用，原运行状态不被伪造为成功。供应商未返回可验证任务 ID 前的受理窗口不可恢复。现有 cash 只接受受支持文本计费适配器，不虚构媒体 credits → USD 换算；路由文档和 PG 托管账号的 Server／Redis 强杀已分别通过合成验证，真实计费未验。
3. **MCP 创作流程**：Local 显式任务已扩至全部 18 个有详情证据的目录模型，见[模型子批](../progress/2026-11-20-luma-mcp-models.md)，保留[图片编辑／视频参数](../progress/2026-11-20-luma-mcp-options.md)与[素材复用](../progress/2026-11-20-luma-mcp-workspace.md)。[配置变更恢复](../progress/2026-11-20-luma-mcp-recovery.md)已接入无关配置变化与已发布刷新链；旧任务保留原 revision，缺身份材料或重新登录无链时拒绝继承，不修改存储版本。[review 答复](../progress/2026-11-20-luma-mcp-review.md)已实现有 answer_format 的显式 JSON 提交和持久回执，格式／目标由上游校验，失败不重放；同一原 run 且带明确格式的 `session.waiting` 审阅已接入；其他题型、其他 run、client tool 仍未开放。Server 已接专用持久任务表；尚无分层布局的实际返回证据。未返回身份的受理窗口不重放，真实远端业务取消未验。分层说明提及 index／label／bounds，但当前输出 schema 的 RunAssetOutput 没有这些字段，result 为开放 object；不能凭 Agents REST 布局假造 MCP 布局。26 项工具合同已[归档](../evidence/luma/gateway-luma-mcp-live-contract-20261120/catalog.json)，不再等待账号或目录。
4. **Web board 当前业务链**：仍缺当前官网 wire/session、任务与资产来源、可靠 model/action 绑定；有效音视频、费用/额度、完整 Keep、远端取消/lease、跨请求去重保证均未关闭。现有单图合成验证不能代替真实 worker 运行。

源码锚点：`src/protocol/luma_agents/video_edit.rs` 的 `pack` 限制当前参数范围；`src/upstream/luma_agents/settlement.rs` 的 `collect` 明确只在原 future 内有限补查；`src/protocol/luma_mcp/models.rs` 的 `ModelQuery` 仅有 `List/Describe`。完整边界见[Agents 合同](../plan/provider-luma-agents-contract.md)与[四渠道合同](../provider-luma-channel-contract.md)。

## 已有实现仍待验收或交付

| 范围 | 尚未证明 |
| --- | --- |
| Agents 真实业务 | Files 上传/摄取/引用/删除，分层/扩画/重绘、真实资产与视频解码播放、晚到扣费、平台权益、Web/REST 额度关系；Files 正常网页到管理 TCP 已通过合成联合验收 |
| OAuth/MCP | 模型列表／全部 18 项详情正常管理 API 真实读取已通过；Local 无关配置变化、已发布刷新链及重启后的原任务恢复已有合成验证。独立目录调查的 transport DELETE 成功，产品远端释放／业务取消仍未证。MCP 的 Server TCP／操作系统强杀已经过隔离 PostgreSQL／Redis 验证；跨机器及配置归档分发未验；正常网页点击及窄屏已有合成联合验收 |
| Web | 当前账号会话与真实 browser worker、当前模型动作与真实费用/额度 |
| Inference | 销售开通账号、模型资格、供应商限速/账单、Server/PG/Redis、跨进程；正常前端→管理 TCP 已通过合成联合验收 |
| 共同交付 | `gateway-devtest-20261120-luma-mcp-options` 与隔离启动已交付，包含此前参数／编辑源码，不含后续素材复用、全部模型、配置变更恢复及 review 答复增量；本批没有重新打包。各渠道承诺的适用 Server／持久化／进程边界及原生业务点击仍分别验收 |

11 月 14 日 OAuth/MCP 真实验收来自[已记录回执](../progress/2026-11-14-provider-channel-expansion-luma-explicit-refresh.md)。其原始临时证据目录在本次核查时已不存在，因此保留历史结论，不能声称这次重新核验了原始日志或真实账号。

## 不应擅自扩成必做的事项

- 重绘 EXR/duration 当前明确不在本地 profile；EXR 需要不同资产合同。是否扩展应按已批准范围定，不把所有官方参数自动变为本次必做。
- `10s + hdr` 多关键帧组合曾存在官方文档冲突，仍待合同核实，不能猜测放行。
- OAuth/MCP 目前是**显式刷新**策略，没有启用自动定时刷新；不把未启用视为现有显式流程故障。
- LLM 的 models/余额接口没有公开合同，不能虚构接口；PSD/SVG 编辑导出、旧 Dream Machine API 也未因本轮工作加入承诺范围。

接续以本页顶部“当前验收规则”为准：额度不足已获用户接受，不再循环测试或作为现有支持范围的交付阻塞。新包已交付，已通过且未变化的检查直接复用；只对仍未实现的明确功能或新证据揭示的缺陷继续开发，不把历史未验项重新当作重复测试清单。

## 持续开发入口

[tools/luma/README.md](../../tools/luma/README.md) 给出可重复运行的命令。脚本和预览夹具已经在仓库，使用相对源码定位与显式输出参数；清空临时目录不会删除这些开发入口。

公开合同、关键红绿日志、原源码哈希及合成截图的现存证据保存于 [docs/evidence/luma](../evidence/luma/README.md)，不依赖旧临时目录。其他项目以及 Gateway 非 Luma 的临时资料未在此做完整清理审计。

# magpie 全范围实施看板

当前执行批次：用户已选定 **Command Code → SenseNova Token Plan → SAP AI Core**，逐个推进到仅待真实账号验收；第一项开发中，后两项排队，不同时扩展 67 项。完成门槛与当前阻碍见 [首批三个收口清单](first-three-closure-2026-11-13.md)。

首批 Hosted 接线（2026-11-14）：Command Code DB／缓存原生目录投影、无 preset 身份恢复、显式协议限制及凭证目录整体替换已通过，产品 57 passed、共享回归 36／3／20／17／29 passed，独立 feature 编译与有效行检查通过。尚非 PostgreSQL／Redis 实例或产品包验收，不增加整卡完成数。见 [Hosted 目录接线](../progress/2026-11-14-commandcode-hosted-catalog.md)。

首批目录后续（2026-11-14）：Command Code 原生专用模型自动导入、按声明探测和管理保存后 Local runtime 重建已通过；含手工模型兼容修复的最终产品回归 49 passed、共享目录 35 passed、Responses 60 passed、前端 5 passed 及 typecheck 通过。Server 目录投影与完整交付仍开放，不增加整卡完成数。见 [目录与原生探测](../progress/2026-11-14-commandcode-catalog-native-probe.md)。

首批最新实现（2026-11-14）：Command Code 原生 Messages 文本／函数工具已通过仅声明 Messages 的保存配置、真实公共 JSON／SSE 两回合及额度／失败退款验收。产品回归 44 passed、共享路由 35 passed、Anthropic 60 passed、独立 feature 编译及有效行检查通过；原生目录自动导入／专用 probe、完整保真和产品交付仍未收口，见 [Messages 切片](../progress/2026-11-14-commandcode-native-messages.md)。以下为此前检查点。

首批最新验证（2026-11-14）：经用户窄范围授权，Gemini feature 关闭路径门禁已修复，三个服务商独立构建及无 feature 基线通过，四个关闭入口无网络执行测试 1 passed，见 [门禁回执](../progress/2026-11-14-gemini-disabled-build-gate.md)。Command Code 原生 Responses 文本／函数工具切片产品回归 39 passed、共享 Responses 回归 47 passed，见 [实现边界](../progress/2026-11-13-commandcode-native-responses.md)。原生 Messages、完整输出保真及交付门槛仍开放，不增加整卡完成数。

当前剩余范围总览见 [2026-11-13 服务商剩余功能块清单](magpie-remaining-2026-11-13.md)：67 项尚未完整关闭，其中 13 项已有不同范围本地实现、43 个主项待实现或资格核实、11 个额外项待实现或差异核对。以下历史检查点不代表当前分类计数。

最新公共禁用块（2026-11-13）：真实公共 Chat 启动阶段与 Responses 生成阶段经管理 CAS 禁用账号，两项独立测试各 1 passed；消息预扣退款、后续请求拒绝与未知用量审计通过。独立 RPC／进程证据分别确认零生成与单次生成、各一次 shutdown 且 child 已退出；后者仅阻止结果发布，不撤回已发送请求。仅测试变更，Key 撤销／轮换／公共取消、真实 CLI／产品包／Server 仍未验收，不增加整卡完成数，见 [公共在途禁用](../progress/2026-11-13-copilot-public-revocation.md)。以下为历史检查点。

最新公共接通块（2026-11-13）：Copilot 不再在所有构建中无条件禁路由；仅显式 SDK feature＋明确 enabled＋有效专用 token 才具静态资格。管理 SecretPatch／CAS 保存 → 真实公共 Chat／Responses → 真实 gate／SDK → Local 消息额度与审计已通过；公开流式／多轮／额外参数／现金／缺 CLI 拒绝保持。公共成功 1 passed、缺 CLI 1 passed，feature-on 普通回归 25 passed／17 ignored，feature-off 配置 4 passed；2 个合成 child 单次发送／关闭并核对退出。默认构建不开放，未更新运行实例，真实 CLI／完整撤销取消／产品包／Server 仍未验收，不增加整卡完成数，见 [受限公共接线](../progress/2026-11-13-copilot-public-routing.md)。以下为历史检查点。

最新启动前块（2026-11-13）：Copilot 显式 CLI 路径失效时，Chat／Responses × 消息／Token 四组合的真实预扣退款、Local hold 释放和失败审计通过（1 passed）；无 child／无工作目录残留，执行准入计数不冒称真实生成。仅测试变更。已确认下一公共接通切片是 feature 条件资格与真实公共 gate／prepare 链验收，不再把无条件禁用当作 sender 未实现；默认构建、Server、cash、streaming 等拒绝边界继续保留，公共路由本轮仍禁用。见 [缺失 runtime 额度与公共接通边界](../progress/2026-11-13-copilot-missing-runtime-quota.md)。以下为历史检查点。

最新额度取消块（2026-11-13）：Copilot 收费成功／拒绝矩阵已扩至 Chat／Responses × 消息／Token 额度；另以独立进程验证消息＋Chat、Token＋Responses 在途取消退款、cancelled 审计和 Local pending hold 释放。矩阵／准入 3 passed，两个取消测试各 1 passed；10 个合成 child 核对退出，取消的 2 个无 shutdown RPC，不冒称优雅关闭。仅测试变更，公共路由仍禁用，启动失败／重启及真实 CLI／原生／Server 仍待验收，不增加整卡完成数，见 [Responses 与取消额度](../progress/2026-11-13-copilot-prepaid-responses-cancel.md)。以下为历史检查点。

最新额度块（2026-11-13）：Copilot 已补真实 Local quota admission → SDK sender → finalizer 的 Chat buffered 消息／Token 预扣、成功保留、拒绝退款和零额度拒绝；未知 token 仍仅保留既有估算、不伪记实际用量。现金准入显式拒绝 copilot_sdk，防止通用 adapter 标签误入 HTTP 打包／现金预留。定向 3 passed，现金回归 12 passed、额度规则 3 passed，4 个合成 child 各单次发送／关闭并核对退出。公共路由仍禁用，收费 Responses／取消及真实 CLI／原生／Server 仍待验收，不增加整卡完成数，见 [Local 额度](../progress/2026-11-13-copilot-prepaid-admission.md)。以下为历史检查点。

最新取消块（2026-11-13）：Copilot sender 已验证 SDK 拒绝不重放、调用方在途取消、失败／cancelled 审计及取消后槽隔离；修复本地 503 被失败审计误记为上游 HTTP 状态，客户端 503 与旧 HTTP 兼容保持。新增 2 passed，sender 回归 6 passed、finalizer 回归 16 passed；最终两组共 4 个 child 核对退出，取消场景无 shutdown RPC，不冒称优雅关闭。公共路由仍禁用，收费 Key／真实 CLI／原生及 Server 验收仍开放，不增加整卡完成数，见 [失败与取消](../progress/2026-11-13-copilot-sender-termination.md)。以下为历史检查点。

最新验证块（2026-11-13）：Copilot sender → Chat／Responses 打包 → Local 审计已通过合成执行验证；修复 Responses 被原生 HTTP 分支截走，以及成功审计／归档输入把非 HTTP 状态伪写为 200 的问题，保留旧 HTTP 缺状态兼容。新增定向累计 6 项通过，finalizer 回归 16 passed、packing 回归 6 passed；最终 2 个 child 各单次发送并核对退出。归档仅验证输入，公共路由仍禁用，真实 CLI／收费 Key／原生及 Server 验收仍开放，不增加整卡完成数，见 [sender 与审计](../progress/2026-11-13-copilot-sender-audit.md)。以下为历史检查点。

最新接线块（2026-11-13）：Copilot buffered sender 已接冻结身份、attempt gate 与 owned SDK 文本执行，启动前后／发布前复核、不重放、未知用量保持；未确认关闭时保留目录并隔离文本启动名额。执行内核 7 passed，6 个合成 child 核对退出；runtime 回归 11 passed、21 个 child 核对退出。公共路由仍禁用，HTTP／计费／真实 CLI 验收未完成，不增加整卡完成数，见 [buffered 接线](../progress/2026-11-13-copilot-buffered-dispatch.md)。以下为历史检查点。

最新准备块（2026-11-13）：Copilot 冻结执行身份原语已实现，绑定可信 Local Key、精确账号及配置代次，重查权限与生命周期时点状态；Key scope/group/model/provider 使用同一次事务读取，避免拼接并发权限版本。定向累计 10 项通过，Local Key 回归 20 passed。尚未接 sender、未解除禁路由，不增加整卡完成数，见 [身份栅栏](../progress/2026-11-13-copilot-identity-fence.md)。以下为历史检查点。

最新准备块（2026-11-13）：Copilot 非流式单轮请求准入适配已新增，检查 canonical 与原始嵌套 JSON，拒绝历史／工具／媒体／未知参数，不从用户 metadata 推断身份。真实 Chat／Responses normalizer 集成测试 5 passed。尚无公共调用点，账号仍不可路由，不增加整卡完成数，见 [请求准入](../progress/2026-11-13-copilot-request-admission.md)。以下为历史检查点。

最新功能块（2026-11-13）：Copilot SDK 单轮聚合文本执行 owner 已实现，正常返回等待 stop，超时／取消复用 SDK 生命周期，缺失／工具／多片／秘密回显等结果拒绝，未知用量不补零。独立 crate 11 passed，最终 21 个合成 child 核对退出。公共 PipelineContext 身份绑定、协议转换与路由仍待接通，账号继续不可路由，不增加整卡完成数，见 [文本执行](../progress/2026-11-13-copilot-text-execution.md)。以下为历史检查点。

最新功能块（2026-11-13）：Copilot 专用身份已复用 SecretPatch／CAS 保存，支持按已保存账号读取目录和 revision 代次校验；token 不进入通用 HTTP payload，专用账号在公共推理接通前不可路由。Copilot 定向 9 passed，账号组回归 17 passed，7 个合成 child 核对退出，见 [保存身份](../progress/2026-11-13-copilot-saved-identity.md)。公共文本推理、真实 CLI 与账号验收仍未完成，不增加整卡完成数。以下为历史检查点。

最新功能块（2026-11-12）：Copilot 临时账号只读管理目录已接线，复用管理鉴权、Secret Grant、revision 与后台任务 owner；每操作隔离目录，断连／关闭后由 owner 停止 runtime。不保存凭据、不走通用 HTTP provider、不触发生成。验证进展见 [管理目录](../progress/2026-11-12-copilot-management-catalog.md)；真实 CLI、凭据保存与公共推理仍未完成，不增加整卡完成数。以下为历史检查点。

最新功能块（2026-11-12）：Copilot runtime 已增加显式账号模型目录读取，使用固定 SDK 的 typed RPC 绕过进程账号／全客户端缓存，保留 policy／terms／能力与未知 billing；A/B 并发、重复读取、失败与超时关闭经合成 EXE 验证，crate 共 7 passed，相关 child 均核对退出。管理鉴权、凭据代次、目录保存与公共推理仍未接通，不增加完成数，见 [账号目录](../progress/2026-11-12-copilot-account-catalog.md)。以下为历史检查点。

最新代码批（2026-11-12）：Copilot SDK runtime owner 已入库，非默认 feature 接线、显式配置、单会话身份、禁工具／权限拒绝、stop／Drop 复用官方 SDK；仓库定向 5 passed，两条 child 路径均核对 PID 退出，checker／ratchet 通过。根 feature 编译在锁释放后已通过（0 errors，112 warnings），启用 Copilot 的根 HTTP 客户端定向回归 28 passed，锁文件未漂移；公共推理／管理仍未连接，不增加完成数，见 [runtime owner 入库](../progress/2026-11-12-copilot-runtime-owner.md)。以下为历史检查点。

最新实验批（2026-11-12）：Copilot 正式 `Client::start` 已通过合成 owned EXE 验证：显式路径、Empty 配置、禁已登录回退、隔离目录、会话身份、detach/shutdown/EOF 和 Windows PID 退出；缺失路径不自动发现。启用 runtime feature 并显式禁下载，最终独立实验 5 passed。尚无 Gateway 生产接线或真实 CLI 验收，不增加完成数，见 [owned runtime 实验](../progress/2026-11-12-copilot-owned-runtime.md)。以下为历史检查点。

最新实验批（2026-11-12）：Copilot 已固定发布版 Rust SDK 1.0.31，在独立临时工程用 Rust 1.98.0 完成 3 项离线合成流测试：握手／版本拒绝、创建前订阅事件、显式身份字段及流关闭。发现 runtime feature 仍可能下载 CLI、普通 streams 默认不是 Empty；未把测试专用接口接入生产。Gateway runtime 尚未实现，完成数不变，见 [SDK 兼容性实验](../progress/2026-11-12-copilot-sdk-compatibility.md)。以下为历史检查点。

最新合同批（2026-11-12）：Copilot 已固定官方 Rust SDK 新快照，核实会话级 token／轮换回调、Empty 与多租户权限隔离、默认 runtime 打包依赖、owned child 关闭及明文 TCP 边界；修正旧合同中 SDK 与 raw session exchange 混用的责任描述，明确待实现的单轮文本切片与验证条件。仅合同推进，没有生产实现、构建或完成数增加，见 [Copilot SDK runtime 合同](copilot-sdk-runtime-contract-2026-11-12.md)。以下为历史检查点。

最新合同批（2026-11-12）：已转向未实现渠道并联网核对 MiniMax Code OAuth，固定官方源码提交、区域端点、PKCE／双形态设备响应与 managed Messages 配置；自有客户端资格及最终 sender 仍未核实，不把 M Plan Key 算作 Agent OAuth。另确认 OpenCode 当前官方文档将免费层聊天限制为 OpenCode，其他客户端使用付费／Go，不能据此替换 Zen Free 原卡。仅合同／范围推进，无运行时实现或完成数增加，见 [MiniMax Code 合同](minimax-code-oauth-contract-2026-11-12.md) 和 [Zen Free 边界](opencode-zen-free-boundary-2026-11-12.md)。以下为历史检查点。

最新子批（2026-11-12）：LobsterAI 已补真实管理 API 保存策略闭环：脱敏草稿／keep patches 提交、revision 冲突拒绝、保存不生成、账号／命名自动策略生效、再次保存关闭并在运行时重建后保持关闭；凭据材料和余额保持。定向 Rust 1 passed / 0 failed / 0 ignored，未改生产逻辑；桌面 UI、真实账号及原生／Server 验收仍开放，见 [LobsterAI 策略保存](../progress/2026-11-12-lobsterai-policy-save.md)。以下为历史检查点。

最新子批（2026-11-12）：LobsterAI 已补完整 AppState／LocalRuntime 关闭并从原文件重建的调度恢复证据：独立结果及到期槽原值保留、立即 sweep 不重复调用、真实 timer 到期后继续执行、Key 和路由文件保持。相关 Rust 93 passed / 0 failed / 0 ignored，新增 1 项，未改生产逻辑；这不是原生 EXE 进程重启或崩溃验收，见 [LobsterAI 运行时恢复](../progress/2026-11-12-lobsterai-runtime-restart.md)。以下为历史检查点。

最新子批（2026-11-12）：LobsterAI 自动测试调度已补真实 60 秒 timer 两轮、账号／命名计划独立结果、到期槽防重复、失败未知用量及不重放、自动关闭／停机零发送证据。定向 Rust 3 passed / 0 failed / 0 ignored，未改生产逻辑；进程重启恢复、真实账号及原生／Server 验收仍开放，见 [LobsterAI 自动调度](../progress/2026-11-12-lobsterai-scheduler-probe.md)。以下为历史检查点。

最新子批（2026-11-12）：LobsterAI 管理测试入口已补本地证据：Secret Grant、精确账号与模型别名、单次／临时／命名／账号保存策略、持久结果读取、审计和普通 Key 余额保持。新增定向 Rust 2 passed / 0 failed / 0 ignored，未改生产逻辑；自动调度、真实账号及原生／Server 验收仍开放，见 [LobsterAI 管理测试入口](../progress/2026-11-12-lobsterai-management-probe.md)。以下为历史检查点。

最新子批（2026-11-12）：LobsterAI 专属控制台模型测试已接入，修复显式测试落入通用路径的问题；复用目录／虚拟解析、Chat／Messages sender、精确凭据代次校验与 401 刷新 owner，不隐藏重放。JSON/SSE、严格文本准入、用量审计、敏感信息脱敏与取消已本地验证。Rust 87 passed / 0 failed / 0 ignored，新增 8 项；管理计划／自动调度、真实账号及原生／Server 验收仍开放，见 [LobsterAI 控制台测试](../progress/2026-11-12-lobsterai-console-probe.md)。以下为历史检查点。

最新子批（2026-11-12）：Nous 已补在途授权轮换后的取消组合：durable grant 发布后取消旧请求，响应体释放、取消审计及未知用量、不写健康状态、不重放、新凭据后续成功均已本地验证。Rust 5 passed / 0 failed / 0 ignored，新增 1 项，未改生产逻辑。真实账号及原生／Server 验收仍开放，见 [Nous 在途取消](../progress/2026-11-12-nous-probe-cancel.md)。以下保留历史检查点。

最新子批（2026-11-12）：Nous 已补发送中授权轮换的成功／401 反馈证据：请求抵达后持久发布新凭据，再释放旧响应；旧请求保留实际审计但健康反馈为 StaleConfiguration，不重放，不重复消费 refresh token；重新选择新凭据可正常调用。定向 Rust 4 passed / 0 failed / 0 ignored，新增 2 项，未改生产逻辑。真实账号、轮换与取消组合及原生／Server 验收仍开放，见 [Nous 在途轮换](../progress/2026-11-12-nous-probe-inflight.md)。后文子批为历史检查点。

持续目标：**完成用户最初三图 42 项和后来五图红框 23 张卡片涉及的剩余原生接入**。
优先级：OAuth／订阅账号优先。不得将“首批三项”“模板存在”或“源码参考已读”改写成全目标完成。
卡片、产品、站点和实现线分别追踪；重复卡不重复算厂商，聚合卡也不能隐藏其未完成的子渠道。

最新子批（2026-11-12）：Nous 模型测试已补发送前凭据绑定保护：审计初始化后复用现有 identity 校验，阻止已轮换／已禁用的捕获快照继续发送，不静默切换账号或目标。真实本地 durable refresh 发布后旧快照不发送、重选新凭据仅一次推理／一次 refresh 已验证；Nous Rust 71 passed / 0 failed / 0 ignored，新增 2 项。发送中轮换、真实账号及原生／Server 验收仍开放，见 [Nous 发送前轮换保护](../progress/2026-11-12-nous-probe-rotation.md)。

前批（2026-11-12）：Nous Portal 自动测试策略已补本地证据：Subscription／Welcome 的真实 60 秒循环、账号与命名计划独立结果、到期第二轮、LocalRuntime 重开后的结果与间隔保留、401 不重放、关闭策略及停机不生成。Rust 4 passed / 0 failed / 0 ignored，其中新增 3 项；并发授权轮换、真实账号及原生／Server 验收仍开放，见 [Nous 自动测试调度](../progress/2026-11-12-nous-automatic-probes.md)。

前批（2026-11-12）：Nous Portal 管理测试入口已补本地证据：Subscription／Welcome 各覆盖授权、精确账号及模型别名、单次／临时／命名／账号保存策略、审计与结果查询；未授权、外来账号、禁用和错误模型不发送。新增定向 Rust 2 passed / 0 failed / 0 ignored，未修改生产逻辑；自动调度、并发授权轮换、真实账号和整卡验收仍开放，见 [Nous 管理测试入口](../progress/2026-11-12-nous-console-management-probe.md)。

前批（2026-11-12）：Nous Portal 专属控制台 Chat 模型测试已本地验证：Subscription／Welcome 复用生产固定目标、精确身份头及禁跳转 sender，补齐配置模型准入、严格回复、脱敏、未知用量、失败不重放和取消审计。Rust 64 passed / 0 failed / 0 ignored，其中新增 7 项；durable refresh owner 未改，完整管理计划、并发授权轮换、真实账号及整卡验收仍开放，见 [Nous 控制台测试](../progress/2026-11-12-nous-console-probe.md)。

前批（2026-11-12）：MiniMax M Plan 专属控制台 Messages 文本测试已本地验证：复用生产区域目标与发送器，修复未知用量被当作零的问题，严格检查有界回复、脱敏、计划结果与取消；国内/国际管理用例均覆盖。Rust 22 passed / 0 failed / 0 ignored，其中新增 7 项。普通 Anthropic 集成测试未重跑，真实订阅/额度及 MiniMax Code Agent OAuth 仍开放，见 [MiniMax 控制台测试](../progress/2026-11-12-minimax-console-probe.md)。

前批（2026-11-12）：Agnes AI 国际站专属控制台 Chat 模型测试已本地验证，补齐生产最终 sender、精确账号/模型准入、严格回复与脱敏、用量审计、管理计划及取消边界；公共 Responses 路径未改变。Rust 27 passed / 0 failed / 0 ignored，其中新增 7 项；真实账号/订阅额度、国内站/Agnes Code 及完整产品验收仍开放，见 [Agnes 控制台测试](../progress/2026-11-12-agnes-console-probe.md)。

前批（2026-11-12）：SenseNova Token Plan 专属控制台模型测试已本地验证，复用生产最终 sender，补齐精确账号/配置模型准入、身份头清理、禁跳转、严格回复、脱敏、审计及取消边界。新增 7 项测试，联合 Rust 57 passed / 0 failed / 0 ignored；真实账号/积分池、完整产品验收仍开放，见 [专属控制台测试](../progress/2026-11-12-sensenova-console-probe.md)。

前批（2026-11-12）：Command Code／SAP 自动测试策略的真实定时循环、账号与命名计划独立结果、到期重复执行、存储重开后的间隔保留、失败不重放和关闭策略不生成已本地验证。最终 Rust 60 passed / 0 failed / 0 ignored，新增 3 项跨产品调度用例；不等同 Server／Redis、原生 EXE 或真实账号验收，见 [自动调度验收](../progress/2026-11-12-commandcode-sap-automatic-probes.md)。

前批验收补充（2026-11-11）：Command Code／SAP／无默认功能的三种独立配置均在 Gemini disabled surface 出现相同 15 个编译错误；分别显式开启 Gemini Web 的两个对照配置通过类型检查。独立裁剪仍受阻，未修改保留的 S06 lane 或强制扩大 feature 依赖，见 [feature 矩阵与责任定位](../progress/2026-11-11-commandcode-sap-feature-matrix.md)。

前批（2026-11-11）：Command Code 专属控制台模型测试已接通，默认连通性不生成；精确账号与配置模型／别名、管理授权、单次／临时／命名／保存策略、答案脱敏、用量审计、失败不重放及取消关闭均有本地证据。Rust 63 passed / 0 failed / 0 ignored，其中 Command Code 30 项、SAP 26 项；相关 UI 15 项通过。真实账号／费用与完整产品验收仍开放，见 [控制台测试](../progress/2026-11-11-commandcode-console-probe.md)。

前批（2026-11-11）：SAP AI Core 专属控制台模型测试已接通，复用生产 OAuth／固定部署 sender；精确账号、管理授权、临时／保存／命名计划、脱敏与用量审计、失败不重放和取消关闭均有本地证据。Rust 41 passed / 0 failed / 0 ignored，其中 SAP 26 项；相关 UI 13 项通过。真实账号、部署发现与完整产品验收仍开放，见 [专属控制台测试](../progress/2026-11-11-sap-aicore-console-probe.md)。

前批（2026-11-11）：LobsterAI 普通目录 Anthropic 模型直选 Messages 已接通，三公共入口、JSON/SSE、工具续轮、缓存现金账单、未知用量、401 刷新与取消语义均有本地证据。最终 Rust 142 passed / 0 failed / 0 ignored，其中名称含 LobsterAI 的 79 项。Kimi 官方合同已匿名联网核对，推理历史仍缺完整往返，继续拒绝。见 [直选 Messages 进展](../progress/2026-11-11-lobsterai-declared-messages.md) 与 [Kimi 合同](lobsterai-kimi-contract-2026-11-11.md)。整体仍为 **67 个工作项未完全关闭**。

前批（2026-11-11）：LobsterAI `yd-fast`／`yd-ultra` 的请求内 resolve→普通 Chat／Messages 双协议本地切片已验证。固定目标、当前 owner 重捕获、三公共入口 JSON/SSE／工具续轮、解析失败与取消退款；冻结 yd 报价与实际模型审计分开。缓存读写完整结算、公共协议各自口径、输出上限别名及最终用量未知不按零结算已验证。最终 Rust 134 passed / 0 failed / 0 ignored，其中名称含本产品的 71 项。真实账号、跨请求持久会话绑定、特殊 profile、Server 与原生包仍开放，见 [虚拟模型进展](../progress/2026-11-11-lobsterai-virtual-resolution.md)。范围仍有 **67 个工作项未完全关闭**；12 项已有不同范围本地切片，44 个主项及 11 个额外项待实现／核实，分母见 [范围盘点](magpie-scope-count-2026-11-11.md)。

前批（2026-11-11）：LobsterAI 普通模型的本地公共推理切片已验证。Chat Completions／Messages／Responses 经固定 Chat 目标支持文本 JSON/SSE、两回合与分片函数工具、严格流结束、失败／取消退款。真实 sender 的 exact 401 唤醒唯一持久刷新 worker，本请求不隐藏重发，后续请求使用新 token；三入口与两种 stream 设置均验证 Published／receipt 和迟到 401 隔离。最终 Rust 106 passed / 0 failed / 0 ignored，其中名称含 LobsterAI 的 57 项；相邻 CommandCode／Chat 守卫回归同时通过。真实账号／自有版本头接受性、虚拟模型 resolve、特殊 runtime profile、Server、独立 feature 构建与原生包仍开放，见 [公共推理进展](../progress/2026-11-11-lobsterai-public-inference.md)。范围仍有 **67 个工作项未完全关闭**，其中 12 项已有不同范围的本地集成切片，逐项分母见 [范围盘点](magpie-scope-count-2026-11-11.md)。

前批（2026-11-11）：LobsterAI 的本地管理授权、显式读取目录并保存、稳定账号重授权、目录表单编辑与桌面授权链接校验已接通。新增管理 owner 有容量、取消、到期和 shutdown 边界；重授权即使返回相同 refresh token，也使用独立续期链，保留旧 durable 记录。最终 Rust 230 passed / 0 failed / 4 ignored，其中名称含 LobsterAI 的 45 项通过；UI 56 项、桌面 launcher 10 项通过。真实前端＋合成管理服务的浏览器流程、430px 窄屏、显式单次保存和关闭取消已验证。推理仍在各实际入口明确拒绝；真实推理的 401 信号接线、公共 Chat SSE/工具/结算、Server 和真实账号仍开放，见 [管理与 UI 进展](../progress/2026-11-11-lobsterai-management.md)。范围仍有 **67 个工作项未完全关闭**，逐项分母见 [范围盘点](magpie-scope-count-2026-11-11.md)。

前批（2026-11-11）：LobsterAI 已接入本地共享持久刷新 owner：未知 expiry、exact generation/access 的 401 信号、single-flight、Sending/receipt/Result/Published、重启磁盘恢复和 Secret Keep 绑定。Rust 定向 219 passed / 0 failed / 4 ignored，其中 LobsterAI 34 项；没有伪造寿命或 Redis TTL，见 [本地刷新进展](../progress/2026-11-11-lobsterai-local-refresh.md)。

前批（2026-11-10）：Command Code Provider API 的显式 Key／匿名目录／管理 UI／Chat 文本工具切片已验证。公共 Chat／Messages／Responses 经固定 Chat wire 支持 JSON/SSE、两回合函数工具及错误/取消结算。最终 Rust 联合 254 项通过，其中 Command Code 23 项；本产品 UI 12 项、名称/ID 2 项、相邻 UI 12 项通过。54 条 manifest 全部保持 metadata-only；真实账号、原生 Responses/Messages、浏览器签发 Key、System One/Jev 与完整产品验收仍开放，Go 私有接口不在本路线。见 [Command Code 切片](../progress/2026-11-10-commandcode-chat-runtime.md)。

前批（2026-11-10）：SAP AI Core 自有 Service Key → client_credentials → 显式 Azure OpenAI Chat 部署的本地管理／UI／推理切片已验证。公共 Chat／Messages／Responses 支持文本 JSON/SSE 和两回合函数工具；通用目录、账号识别及探测在使用秘密前拒绝 SAP，专属控制台测试仍开放。最终 Rust 联合 207 项通过，其中 SAP 19 项；SAP UI 8 项与名称合同 1 项通过，相邻名称合同保留对照复现的 5 项失败。整体仍为 `partial_pass`；53 条 manifest 全部保持 metadata-only，真实账号、自动部署发现、其他 SAP 协议及独立 feature 构建未验。见 [SAP 切片](../progress/2026-11-10-sap-aicore-chat-runtime.md)。

前批（2026-11-10）：SenseNova Token Plan 显式订阅 Key 的本地管理／目录／UI、三公共文本入口 JSON/SSE、两回合函数工具及失败／取消结算已验证。最终 Rust 40 项、相邻 Rust 152 项通过；两组 UI 分别 19／18 项通过。共享 Chat 守卫补齐坏工具流校验，Qwen 原错误诊断与重试策略保留。整体仍为 `partial_pass`；当时 52 条 manifest 仅是 metadata 登记，真实账号、控制台 OAuth／积分池和独立 feature 构建未完成，小浣熊继续单列。见 [SenseNova 切片](../progress/2026-11-10-sensenova-token-plan-runtime.md)。

前批（2026-11-10）：MiniMax 官方 M Plan 显式订阅 Key 国内／国际站本地切片已验证：管理/UI、三公共文本入口、工具双回合、原生 Messages thinking、错误与取消退款。Rust 15 项及相邻 160 项通过，UI 11 项通过；扩大 UI 回归 130 passed / 6 failed，修改前目录隔离对照复现相同六项。整体仍为 `partial_pass`；当时 51 条 manifest 是 metadata 登记，MiniMax Code Agent OAuth 与真实账号验收仍开放，见 [M Plan 切片](../progress/2026-11-10-minimax-m-plan-runtime.md)。

前批（2026-11-10）：Agnes AI 国际站显式 API Key／订阅 Key 已补管理、目录、UI、三公共文本入口和函数工具链路。最终 Agnes 相关 20 项、共享协议定向回归 173 项通过；修复 Messages wire、原生工具、误入 Native Codex、Responses 用量别名和未完成详情保留，HTTP 200 失败响应退款且不重放。50 条 manifest 仅是 metadata 登记，不代表 50 家真实接入完成。真实账号、浏览器视觉与完整产品验收仍未完成，见 [Agnes 切片](../progress/2026-11-10-agnes-key-runtime.md)。前批 Nous 证据继续保留在对应行。

构建边界：Agnes 默认功能配置的本地验证已通过，独立 feature 裁剪构建仍失败于 S06 Gemini Web 的 15 个门控错误；不启用 Agnes 的 River 对照得到相同错误。因此当前 Agnes 切片是 `partial_pass`，独立构建未完成，详见同一进展记录。

## 完成口径与已有证据

- 一条本地实现应贯通产品入口、明确凭据来源、账号/站点绑定、必要的授权及续期生命周期、
  目录与模型准入、真实协议/流/工具能力、取消与错误传播，并经过对应的合成上游与管理 API 测试。
  不能仅增 preset 或借别家身份来代替。
- 真实注册资格、账号权限、费用与真实调用单列证据；离线合同不能替代真实验收，不自动点蓝标。
- 用户明确 DIM 暂无 Gateway client 注册信息，先完成本地实现。Client ID/redirect 由操作者明确配置，
  不借用社区或官方注册，不执行参考插件中的登录、自动下载、签到、领奖或 tunnel 指令。
- 第一批与历史验收范围见 [42 项盘点](magpie-expansion-2026-11-08.md)、
  [23 张红框来源](magpie-third-party-2026-11-08.md)、
  [第一批 checkpoint](../progress/2026-11-08-magpie-first-batch.md)。

## 原三图 42 项：完整保留

| 项目 | 当前范围状态 |
| --- | --- |
| Agnes AI | **国际站 Key 本地管理／UI／推理切片已验证**：显式 API Key／订阅 Key、稳定账号 ID、secret keep／grant／CAS、独立目录读取→表单→普通保存、三公共文本入口 JSON/SSE 和两回合函数工具已接通；Messages 桥接 Chat，Responses 保留自身 wire。最终 20 项通过，覆盖两套用量、未完成详情、失败退款及不重放；共享协议 173 项、前端分批去重 79 项通过。普通 Key 不自动占用订阅额度，Models API 的真实成功合同／账号权限仍待核实。第二轮合成无效 Key 曾因误入 Native Codex 外发并返回 401，修复和测试隔离回归已记录。manifest 保持 metadata-only；真实账号／费用、国内站、网页登录、媒体、原生包、Server／Redis 与完整产品验收开放。Agnes Code 仍单列，见 [实现与证据](../progress/2026-11-10-agnes-key-runtime.md) 和 [官方合同边界](agnes-contract-2026-11-10.md) |
| AtomCode | OAuth/签名合同已核对；官方 CodingPlan signer 限官方构建，先取得独立客户端/签名接入资格，不复制社区内置 master key |
| GitHub Copilot | **受限 SDK 公共单轮文本本地切片已验证**：专用身份保存、真实 Local Key／公共路由、Chat／Responses buffered、额度审计及启动／生成阶段管理禁用已通过合成 CLI 验证。仅显式 SDK feature＋明确 enabled＋有效 token 具备资格，默认构建禁用。真实 CLI／订阅账号、公共 Key 撤销／轮换／断连取消、原生包与 Server 仍开放；流式／历史／工具／媒体／现金当前不支持。见 [公共接线](../progress/2026-11-13-copilot-public-routing.md)、[在途禁用](../progress/2026-11-13-copilot-public-revocation.md) 和 [SDK runtime 边界](copilot-sdk-runtime-contract-2026-11-12.md) |
| JoyCode | 待原生实现 |
| Kimi Work | 待原生实现；不是 Moonshot Key API |
| OpenCode Zen Free | 待实现；2026-11-12 官方当前文档明确免费层聊天要求 OpenCode，其他客户端使用付费／Go；不伪造身份绕过，也不以付费产品替换原卡，见 [当前边界](opencode-zen-free-boundary-2026-11-12.md) |
| ZCode | 待原生实现 |
| Qoder | 待实现，含 CN 与 QwenWork/enterprise 产品分支 |
| Devin | 待 Connect/protobuf 原生实现 |
| Zed | 待 envelope/NDJSON 原生实现 |
| Factory | 待原生实现 |
| Grok Build | 已有产品 owner，本轮补 device；完整 runtime-defaults 编译与定向联合回归已收口，真实账号未验 |
| Command Code | **Provider API Key 本地管理／匿名目录／UI／Chat 切片已验证**：固定官方目标、grant／Keep／稳定 ID／CAS，目录只导入明确声明 Chat 的模型，三公共文本入口 JSON/SSE、两回合函数工具、失败不重放与取消恢复本地预扣；前批本产品 Rust 23 项、联合 254 项通过；本批专属控制台模型测试已验证，最新 Command Code 30 项、联合 63 项通过，相关 UI 15 项通过，见 [控制台测试](../progress/2026-11-11-commandcode-console-probe.md)。manifest 保持 metadata-only；真实账号/消费、原生 Responses/Messages、浏览器签发 Key、Jev、完整 reasoning/media、ZDR、原生包及独立 feature 构建开放。Go 套餐不具备第三方 Provider API 资格，不回退私有 alpha 路由。见 [实现与证据](../progress/2026-11-10-commandcode-chat-runtime.md) 和 [官方合同](commandcode-chat-contract-2026-11-10.md) |
| MiMo | 待原生实现；他处同名模型不算接入 |
| Cursor | 待双向 Connect 原生实现 |
| Kiro | 原有 Key/OAuth 部分接入；完整产品/跨应用验收仍待补 |
| MiniMax Code | **Agent OAuth 仍待原生实现**。已新增独立官方 M Plan 订阅 Key 本地切片（国际/国内固定目标、管理/UI、Messages 文本工具、错误结算），最终 Rust 15 项通过；不能把订阅 Key 当成 mcode-public / mavis / 免费 credits 接入。见 [M Plan 进展](../progress/2026-11-10-minimax-m-plan-runtime.md) 和 [官方合同](minimax-m-plan-contract-2026-11-10.md) |
| Warp | 待 protobuf 原生实现 |
| WorkBuddy | 待专用账号接入，含国内/国际；现有软件上下文不算 |
| Cline | 待原生实现 |
| Trae | 待原生实现 |
| Open Design | 待原生实现 |
| CatPaw | 待原生实现 |
| Loomy | 待原生实现；短信/session，无伪 refresh |
| Nous Portal OAuth | **本地管理／目录／UI 与双目标推理切片已验证**：显式 device session、receipt 先保留后准入、取消／到期／shutdown 接收保护、原 revision 单次 CAS 保存、稳定 ID 重新授权、独立目录读取→表单→普通 CAS，512 字节模型 ID 原样保留。Subscription／Welcome 的三公共文本入口 JSON/SSE、两回合函数工具、Embeddings 原生 JSON、单字符串 legacy Completions、取消退款及未知拒绝不重放已有本地证据；不支持的 legacy 控制明确拒绝，不伪称原生完整 Completions。最终 Nous 库 r3 为 57 passed；此前 55/2 失败已保留并修复，相关 OpenAI 66、结算 16、原始模型观察 5 项通过。UI／桌面 launcher 仍沿用上一轮证据，不冒称本轮重验。独立 manifest 与 inventory 已登记，但保持 metadata-only。真实 client／账号、浏览器视觉、Server／Redis、原生包、媒体及完整产品验收仍开放；初始 device receipt 仅有界进程内保存，不借 `hermes-cli`，不猜能力／价格。见 [最新推理与清单切片](../progress/2026-11-09-nous-inference-metadata.md)、[管理与 UI](../progress/2026-11-09-nous-management.md)、[公共推理](../progress/2026-11-09-nous-public-runtime.md)、[worker](../progress/2026-11-09-nous-worker-lifecycle.md)、[typed storage](../progress/2026-11-09-nous-typed-material.md) 与 [资格/合同](nous-contract-2026-11-08.md) |
| 商汤小浣熊 | 待原生实现，不与 SenseNova 合并 |
| SenseNova Token Plan | **显式订阅 Key 本地管理／目录／UI／推理切片已验证**：固定官方目标、grant／secret keep／稳定 ID／CAS、独立 Models API 筛选文本输出，公共 Chat／Messages／Responses 经 Chat wire 支持 JSON/SSE 与两回合函数工具。最终 Rust 40 项、相邻 152 项通过；坏 JSON／工具流／截断／错误尾部明确失败，退款且不重放。52 条 manifest 保持 metadata-only；真实 Key／订阅权限／消费、控制台 OAuth／积分池、完整 reasoning、媒体、原生包、Server／Redis 与独立 feature 构建仍开放，不与小浣熊合并。见 [实现与证据](../progress/2026-11-10-sensenova-token-plan-runtime.md) 和 [官方合同](sensenova-token-plan-contract-2026-11-10.md) |
| InkStone | 待原生实现 |
| AutoClaw | 待原生实现，国内/国际隔离 |
| CodeArts / OfficeAce | 待 PKCE/DPoP/STS/签名原生实现，产品身份分开 |
| GCP AI | 已有 Vertex OAuth/Express；ADC 导入/发现仍待补 |
| Cloudflare Workers AI | 首批代码与定向合同已落地，真实账号/推理未验；非 AI Gateway |
| River | 首批代码与定向合同已落地，真实账号/推理未验 |
| SAP AI Core | **自有 Service Key 与显式 Azure OpenAI Chat 部署的本地管理／UI／推理切片已验证**：复用 grant／secret keep／稳定 ID／CAS，client_credentials 有界缓存与刷新，三公共文本入口 JSON/SSE、两回合函数工具、失败不重放及取消恢复本地预扣。部署 URL／资源组／API 版本显式绑定，通用目录／识别／默认连通性在使用秘密前拒绝。最终 SAP 19 项及相邻 188 项在同一 Rust 联合轮通过，SAP UI 8 项与名称合同 1 项通过。manifest 保持 metadata-only；真实账号／权限／费用、自动部署发现、其他云 endpoint、x.509、Orchestration／Bedrock／原生 Responses、完整 reasoning／媒体、原生包、Server／Redis 与独立 feature 构建仍开放。专属控制台模型测试已补齐，最新 41 passed / 0 failed / 0 ignored，其中 SAP 26 项；见 [控制台测试](../progress/2026-11-11-sap-aicore-console-probe.md)、[实现与证据](../progress/2026-11-10-sap-aicore-chat-runtime.md) 和 [官方合同](sap-aicore-chat-contract-2026-11-10.md) |
| OrcaTerm | 待原生实现 |
| AStudio 讯飞 | 待 AstronStudio 专用接入，非 Google AI Studio/讯飞 MaaS |
| Providers Hub | 来源已固定；11 个随包子产品展开见下，不计一家新厂商 |
| Agnes Code | 待原生实现，与 Agnes AI 分开 |
| Tabbit | 待原生实现 |
| Codar | 待原生实现 |
| MClaw | 待原生实现 |
| Antigravity CLI | 待 local agy ACP；现有 direct HTTP 不等于 CLI 实现线 |

## 红框 23 张卡：完整保留

| 卡片 | 当前范围状态 |
| --- | --- |
| Our Free Model | 聚合渠道展开见下；不嵌入原 runtime，不复制自动 tunnel |
| Muse Web | 待独立浏览器 owner；非普通 OAuth/公开 API |
| CodeArts | 与原 42 同产品，作为独立参考版本核对，不重复计数 |
| Magicrew | 待国内/国际接入；保留停维护、WAF/ESA405 已知阻碍 |
| DIM | **本地文本/函数工具切片已验证**：两站隔离、管理/UI/持久刷新、独立目录读取/表单应用→CAS、四 wire 到公共三入口 JSON/SSE/两回合工具与取消退款，67 库内 + 3/3/1 集成筛选测试通过；相邻 Vertex 196 passed / 2 ignored。Gemini 是聚合 SSE，真实 client/账号、媒体、桌面/Server/跨应用未验，不算完整厂商验收，见 [DIM 进展](../progress/2026-11-08-dim-management-routing.md) |
| DeepSeek Web | 待 Web token/session/PoW/SSE 原生实现；非 DeepSeek 官方 API |
| OpenCSG | 待网页登录换 Key；非长期 OAuth refresh |
| Notion | 待独立 Cookie/私有 AI API；保留文本与工具限制 |
| TeleAgent | 待手动 session 与签名接入；无第三方 refresh，不自动解密客户端 |
| ModelScope msmoli | 辅助扩展，非推理服务商；签到/额度等原生功能待实现，不能默认执行副作用 |
| Marvis | 待浏览器账号/refresh/业务签名原生实现 |
| QwenWork device | 待 COSY 原生实现；与第二张 QwenWork 卡去重 |
| Loomy | 与原 42 同产品，仍待实现 |
| AtomCode | 与原 42 同产品，仍待实现 |
| SenseNova | 与原 42 Token Plan 同产品，不重复计数；显式订阅 Key 的本地文本／工具切片已验证，真实账号、控制台 OAuth／积分池及完整产品验收仍开放，见 [SenseNova 进展](../progress/2026-11-10-sensenova-token-plan-runtime.md) |
| EdgeOne | 待匿名 agent session/RPC/events.mux，非普通腾讯 Key API |
| AMD Radeon Cloud | **资格核实发现共享接入阻碍**：官方 Token Factory 禁止生产、向第三方提供访问及账号池，个人开发限定用途另行界定。当前合同含 Chat、Messages、live models、pts 用量；仍未原生实现或完整验收，不移出范围。见 [AMD 资格与合同](amd-radeon-qualification-2026-11-12.md) |
| LobsterAI | **授权、持久刷新、管理/UI/目录、普通目录及虚拟模型双协议公共推理的本地切片已验证**。普通 Anthropic 模型可直选 Messages，yd-fast／yd-ultra 每请求解析后按真实协议推理。三入口 JSON/SSE、工具续轮、缓存现金计量、未知用量与失败／取消账单语义、401 刷新无隐藏重发均有证据。最新 Rust 142 passed / 0 failed / 0 ignored，其中名称含 LobsterAI 的 79 项；UI 与桌面沿用前批证据。真实账号／订阅／费用／自有身份及能力头、跨请求持久绑定、特殊 profile／Kimi、reasoning／媒体、Server／Redis、独立 feature 与原生包仍开放。见 [直选 Messages](../progress/2026-11-11-lobsterai-declared-messages.md)、[虚拟模型](../progress/2026-11-11-lobsterai-virtual-resolution.md)、[Kimi 合同](lobsterai-kimi-contract-2026-11-11.md) |
| DuMate | 待 Cookie→临时 gateway token 与 Chat；Cookie 本身不伪造续期 |
| Cloudflare AI Gateway | 待 BYOK/provider 路径原生实现；官方文档独立实现，不复制未许可源码 |
| HF SSH Chat | 待 SSH/PTY/host-key/队列 owner；非 HF HTTP Router，不默认建 tunnel |
| QwenWork 第二实现 | 同产品，比较签名/官方 WASM 依赖，不重复建第二套账号池 |
| OpenCode 2 Free | 与 Zen Free 同受限产品路线，不照搬身份伪造/盲重放/截断假成功 |

## 聚合卡隐藏的实际范围

Providers Hub 固定到 [7c4ad5a9](https://github.com/Gyl1219/magpie-providers-hub/tree/7c4ad5a9d0a729c66678e8dcbf93695e052dfd38)，MIT。
固定 HEAD 的 package 为 0.12.2，而联网 npm latest 为 0.12.3；未安装包，不把版本号当成功证据。

11 个随包 provider 为 `astudio`、`accio`、`lobsterai`、`magicrew`、`magicrew-intl`、
`autoclaw`、`autoclaw-cn`、`loomy`、`monkeycode`、`monkeycode-intl`、`qiniu`。
其中 Accio Work 必须核对既有 Accio 的产品/协议差异；其余重合项保留国内/国际目标隔离。
**新增需单列 MonkeyCode 两站、七牛云 MaaS/Modelink**，均待实现。
Hub 另有百智云源码但不随该版本 npm files 发布，单列待核对；灵犀、豆包工作、Dola
在固定快照仅提及/条件入口、缺实现文件，暂缺可用协议合同，不冒称有可直接移植实现。

Our Free Model 的 13 账号渠道是 CodeArts、CodeBuddy、WorkBuddy 国际、LobsterAI、Qoder、Qoder CN、
Trae、Cline、Loomy、小浣熊、MiniMax Code、ZCode、Gemini Code Assist/Antigravity。
原固定社区包本就含 WorkBuddy 国际与 Qoder CN，不能重复算新厂商；Gemini 应核对既有 Cloud Code 路线，
不是新 Gemini Web。**CodeBuddy IDE、Kilo 免费池、EAC 授权聚合网关**另列待原生实现。
EAC 不按其底层模型品牌虚增已接厂商，也不下载执行原运行时来替代 Gateway 原生 owner。

所有上述项目都保留在持续目标内；当前没有任何“聚合卡已整体完成”的回执。

## OAuth／订阅客户端资格的补充核验

2026-11-08 匿名联网核对 [AtomCode 官方 README](https://github.com/atomgit-atomcode/atomcode/blob/main/README.md)
的 “About the official CodingPlan (closed-source signer)”：真实签名实现仅由官方 release pipeline 注入，
开源仓库为占位实现；官方说明此限制用于防止免费计划在官方构建外被滥用。社区公开的 master key、
UA 或客户端版本不能代替 Gateway 的接入资格。通用 [AtomGit OAuth 注册](https://docs.atomgit.com/docs/apis/oauth/)
也不能证明可使用 AtomCode 专属 broker／CodingPlan。此项继续保留范围，但不实施冒用官方构建身份的签名路径。

固定 Nous 社区 `packages/nous/README.md:25-30` 自己声明，Nous 尚未确认独立客户端可使用
`hermes-cli`。本轮匿名核对 [官方插件文档](https://hermes-agent.nousresearch.com/docs/developer-guide/model-provider-plugin)
与 [Portal API docs](https://portal.nousresearch.com/api-docs)，没有找到 Gateway 的 client 注册或使用许可。
后续本地实现应要求操作者提供获准的显式 client，而不是把公共客户端常量当作产品许可；不读取 Hermes 账号文件、
不与其他客户端争用一次性 refresh token。以上是资格和实现边界，不是已发生的真实登录失败或推理验收。

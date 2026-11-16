# 服务商扩展：Grok Web 当前 WS 单轮文本子链

日期：2026-11-06。工作区 `C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`，`main` / `7969f12c85efceacf2a5e384e143b5c39a550981`，包含继承及本批未提交改动。**这是 R03 的限定文本子批，不是 Grok Web 整卡、P50 或总计划完成。**

证据根：[gateway-grok-web-20261106](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-grok-web-20261106)。合同及固定来源见 [Grok Web 合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/provider-grok-web.md)。最终结果和来源以本批阶段回执、`audit.json` 与 `closeout.json` 为准，不以中间通过或仅 HEAD 代替当前 dirty 源码。

## 1. 本批改变

- 联网固定 `chenyme/grok2api@e2fd7ab481a30a1965bb61395d437154cf247c92` 当前公开实现及历史 POST 对照；当前文本直接走 WS，没有证据支持其回退旧 NDJSON。参考项目没有被执行，Gateway 没有增加依赖或另起代理服务。
- typed `grok_compatible` + `special_adapter: grok_web` 在原三适配器准备之后进入独立 WS owner：固定 audience、同账号 Cookie/UA 的身份 GET、同一现有 Chrome136 wreq client 的 HTTP/1.1 Upgrade、匹配 session.created/conversation.attached 后只发送一次 turn。使用已锁定 `wreq::Response::upgrade()` 和 `tokio-tungstenite`，不借 Codex 专有传输。
- 仅接受单个 user/纯文本和 `fast / auto / expert / heavy` 模式。公共 Chat/Responses 的 buffered 与真增量 Chat SSE 复用原 Responses 转换、TrackedStream、审计/计费/取消 owner；新 Responses 准入只适用 typed Grok Web，不扩大旧 generic、Messages 或 Completions。
- 正常管理的显式模型测试按原 Grant、账号启用和模型归属检查进入同一 dispatcher；默认连通度检查不发送生成，别名不触发换号。原始传输错误不回显会话/私有 body；probe 保留原脱敏和结果记录。
- 只在明确 completed 终态、非空答案和 response ID 下成功；提前 close/EOF、坏 JSON、超限、错误或未知事件不能补成成功。usage 未知保持 None/省略，不估算或填零。无自动 retry/replay/跨账号或跨渠道 fallback；旧非 typed generic 兼容保留，typed raw request-plan 不可退回旧 POST。
- Cookie/SSO/固定受众、握手 nonce/Accept、可选已出现 response ID 的一致性、总 deadline/帧/正文上限有明确边界。单个 pull-driven socket/future owner，无后台 relay/无界队列，Drop 关闭连接；没有平台 response.cancel 的证据，不承诺远端立即停费。媒体与 side-by-side 请求 false 是本地策略，其平台接受性仍未验。

## 2. 验证与失败保留

- 真实行为红测 `red-plan-v2` 复现 typed 配置仍能构造旧 HTTP plan；首个 `red-plan` 是测试 crate 名误写，不能算行为红测。早期 WS 编译错误和输入漂移分别保留，不作最终绿色来源。
- `ws-full-v3` 为 15 passed/3 failed：公共 fixture 的备用 provider 复用 credential ID，被原 strict route validation 正确拒绝。修正 fixture 后 `ws-public-v4` 继续失败，真实 SQLite audit 显示 Chat 已成功、Responses 的 candidateCount=0；因此最小修复 typed Responses 的路由准入，而非放宽断言、改 upstream surface 或添加平行转换。
- 最终 `ws-final-v5` **19/19**；扩大正常 Grok 定向 **134/134**（包含这 19 项）、正常 integration **1/1**、相邻路由 **35/35**、legacy Grok 协议 **10/10**、生产 `runtime-defaults` lib check 与官方 Rustfmt check 通过。正常/代表性关闭的九个最终阶段各 **3273** 项选定输入无漂移，不将重叠测试简单相加。
- 首次仅关 Grok 的 broad filter 为 **104 passed/5 failed**：一个测试硬断言 default build 开线，四个旧 preset/header 测试也未按 feature-off 条件约束，均为未改动的默认开线用例；不是新 WS 执行器在关线时发出了请求。五项在正常配置的 134 项中全部通过。关线适用用例显式排除这五项后 **104/104**，新增关线 integration **1/1** 证明 typed dispatcher 返回 `grok_web_line_disabled` 而非退回旧 transport；不改生产或测试源码来掩盖失败，不报完整 feature matrix 绿。
- 公共测试使用生产 `build_router(...).oneshot`、真实 LocalRuntime SQLite、实际随机回环身份 HTTP + WS peer，覆盖 Chat/Responses×buffered/SSE、精确账号/别名、caller 材料隔离、未知 usage、失败和读到答案后 Drop。Gateway 入站不是独立 TCP 进程；管理 probe 也不是 UI→真实管理 TCP 联合验收。本地 HTTP+WS 不证明真实 TLS 指纹或平台接受性。
- 正常路径等待审计收尾、state 引用释放、SQLite close、socket peer 与本批 listener 退出。没有读取真实 `.ng`、seal 或历史凭据；只读审计抽取仅针对本批合成数据库，不是平台账单或真实费用验收。
- 官方 lexer 测得 **24 项源码、16 项新增**，最大 **494** 有效行，新增最大 **247**；八项既有文件的前后值在 `lines-final.json`，没有 >700 文件增长，也没有修改 baseline/exception。
- checker **31/31**、ratchet **4648 files** 已通过，14 项 immutable runtime 单列、governed source >700 为 0。审计发现临时 wrapper 错把输出路径视为 `scripts/artifacts`，实际 checker 写根目录 `artifacts`；错误历史原样保留。已归档真实本批输出，按紧邻前批 audit 已验证的备份恢复旧报告，再以正确路径直接备份/重验/原字节恢复，两次实际输出 SHA256 相同。不能宣称本批最初已有正确的直接快照；详见 `line-gates-corrected.json`。临时行数脚本 import 路径错误也已更正，不涉及产品源码。
- 最终审计逐项核对九阶段源码 hash、24 项源码和四份文档的 UTF-8 无 BOM/本批 diff、本地链接、固定公开来源 SHA256 及全工作区 `git diff --check`；仅抽取本批合成 SQLite 的审计字段，不读取 seal。具体计数与 hash 见 `audit.json`，不宣称全仓回归、零 warning、平台费用或真实业务成功。

## 3. 未完成与接力

R03 仍缺多轮/system/history、模型目录/订阅/额度、搜索引用/工具/附件等完整能力、SSO/challenge 录制与恢复，以及真实平台接受性、费用/权益、原生 devtest 和独立进程验收。当前四种模式不是固定模型版本或已证账号资格；不得用这条单轮子链宣称完整 Grok Web。

原始其他渠道及 P50/P60/P70 按[总状态](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-06-provider-channel-expansion-status.md)继续。无需现在逐项索要凭据，也不靠不断扩展本子批替代 Qwen Web、ChatAIBot、Luma Web、Suno、Udio 等原始入口的核心闭环。

模板仍 **16**、无新增整卡 `offline_ready`，总目标仍 active。本批未提交/推送/部署、未生成 EXE、未修改在线实例或操作 Docker；P34 Redis 授权已使用并清理，不重放历史批准。

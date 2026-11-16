# LobsterAI 独立浏览器会话协议切片

本批已完成本地协议执行与合成上游验证，整体状态为 `partial_pass`。最终 Rust 联合 86 项通过，其中 LobsterAI 20 项；管理 API/UI、持久凭据与 401 刷新 owner、公共 Chat SSE 尚未接线，不代表已可在控制台使用 LobsterAI。

## 实际改动

- 增加 `src/protocol/lobsterai`：独立动态 loopback → state/code 校验 → 一次 exchange → received receipt → 明确个人身份准入 → 动态目录 → exact-access 401 refresh 的协议链。
- token 未提供的 expiry 保持未知；不能接入现有按到期时间调度的 refresh worker。缺 successor refresh 时保留旧值，未明确个人模式或企业上下文不发布会话。
- 成功 receipt 仅交付一次 `SessionTokens`；准入失败保留原始回执供调用者审查。内存 receipt 尚无崩溃恢复能力。
- 协议 client 接收网络 builder，强制 `Policy::never()` 后构建自己的私有 HTTP client；传入的 500 POST 重试策略不能重放 exchange/refresh。默认头、redirect 和响应预算另有明确边界。
- 提取 DIM 已有的严格 loopback HTTP 解析和拒绝重复 JSON 成员的解析器，复用其行为；DIM 本地回归纳入本次联合测试。
- 增加 [67 项范围清单](../providers/magpie-scope-count-2026-11-11.md)，保留主卡、去重与聚合子项分母。

## 实际验证

首轮 `cargo-r1` 为 84 passed / 0 failed / 0 ignored，其中 LobsterAI 18 项。随后独立只读审查发现可重复准入 receipt 与继承 client retry policy 的漏洞，均已修复并补测试；最终 `cargo-r2` 为 **86 passed / 0 failed / 0 ignored**，其中本产品 **20** 项，相邻 **66** 项。首轮证据不代替最终源码验证，两轮日志均保留。

最终轮覆盖真实 loopback socket 发送/接收、错误 state 后正确回调、exchange/目录/refresh 串联、自有 attribution、无 ambient 身份、企业身份和刷新身份变化拒绝、缺 refresh 保留、只允许 401、重复成功准入拒绝、非流式 JSON 错误、嵌套重复字段、解码长度限制、超时、取消关闭端口，以及重试策略阳性对照。全部使用 synthetic token 与非转发 loopback peer，没有真实登录或生成。

`npm run test:effective-lines --prefix scripts`（31 项）、`npm run check:effective-lines --prefix scripts`、定向 rustfmt、scoped diff 检查和 UTF-8 无 BOM 校验通过。最大新文件为 351 有效行（wire 合同测试）；原 DIM listener 144→98、reply 70→13，协议索引 114→117。没有修改 baseline、增加 exception、升级依赖或运行全量/付费测试。

本批未改变前端、manifest 或库存元数据，因此不重复上批 UI/Python inventory 回归；54 条 manifest 的上批 metadata-only 状态不升级。未重跑独立 feature 构建，也未以本批验证消除已有 S06 feature 门控问题。

## 仍需推进

1. 将未知 expiry/401 驱动策略纳入共享 durable grant owner，复用 generation、single-flight、reservation、receipt 与 CAS；不捏造六小时寿命。
2. 接入管理授权会话、保存凭据、稳定账号 ID、grant/revision fence、目录表单和桌面浏览器 launcher。
3. 公共 Chat/Messages/Responses → Lobster Chat SSE、非流式聚合、工具双回合、审计/结算、错误/取消行为。
4. 明确授权的真实账号、Gateway 自有 attribution 接受性、企业上下文、订阅权益/消费、原生包及 Server/Redis 验收。

范围总数仍为 **67 个未完全关闭的工作项**。本批使 LobsterAI 从未实现推进到授权协议切片，不算整项关闭。

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-lobsterai-20261110`（本批跨日继续）。关键文件为 `contracts/sources.json`、`scope-inventory.json`、`cargo-r1.json/.log`、`cargo-r2.json/.log`、`source-before.json`、`source-r2.json`、`source-final.json`、`scoped.diff`、`final-validation.json`。

保留开始时的 inherited dirty 工作区；本批证据只覆盖 scope，不声明 whole-working-tree provenance。没有提交、推送、出包、部署、重启实例或访问真实 `.ng`。

官方合同与资格边界详见 [LobsterAI 会话合同](../providers/lobsterai-session-contract-2026-11-11.md)。

# 三适配器框架第二轮修复

## 范围与结果

本轮修复上一轮审查确认的五处缺口，保留此前未提交的界面、滚动加载、4300 手测隔离和
框架改动。不构建 EXE，不提交或推送，不修改运行实例、真实凭据或数据库。

1. **卡片指纹编辑**：真实 `usePilotPolicyEditor` 改用既有同步写入 owner，更新新 flow 和旧
   profile；卡片显示优先读取 flow，无 flow 时继续读取旧字段。Claude/自动切换不再被旧策略覆盖。
2. **official 格式所有权**：仅有效 OpenAI adapter 可进入 ChatGPT official planner/executor。
   固定 Anthropic 不再因地址包含 official 标识而被流式快捷分支改回 Responses。
3. **official 发送合同**：三条发送分支复用统一 planner 后处理及 profiled send，执行 extra-body
   合并、CPA/CPA Full 准备与传输准入；保留 official/Codex 网络错误归因和原响应、SSE、大小限制 owner。
4. **Redis 候选**：preset/raw 两分支读取 typed flow/指纹，先准备有效 adapter，再依据原始模型
   allow/block 计算协议能力；错误策略排除候选并记录非敏感原因，不悄悄降回旧策略。
5. **Hosted 与直达执行**：普通路由、凭据合并、access-catalog 在原始 JSON 仍可读时准备候选。
   请求级协议选择前同步 payload/adapter/family/profile/endpoint execution mode；发送资格检查
   及 FreeBuff 直达入口在 keepalive、session、run lease 或网络动作前防御性校验。

## 权限与兼容边界

- 不在已丢失原始模型规则的请求级 finalization 中重新放宽 supported families。
  原始规则解析后为空直接排除，避免后续将空列表误解为“不限协议”。
- access-catalog 的候选与额度来源行仍按 tuple 成对过滤和重排；没有新增授权账号或凭据。
- 保留 discovery 负模型证据和固定格式约束，不提升 Gemini/Cohere discovery-only 能力。
- Redis 不新增信任 raw `discovered_protocols`。直接读取它会绕过正规 discovery 的凭据绑定、
  origin、协议 base 与模型证据检查；本轮只补必需的 flow/profile，并添加禁止该扩张的回归。
- 无 flow 的请求级准备保持旧路径；不升级存储版本，不迁移或重写数据库，不新增共享状态。
- 专用执行器仍受原 preset/transport 约束，本轮未接管 Gemini executor 或开放任意插件组合。

## 实际验证

| 检查 | 结果 |
| --- | --- |
| `cargo test ... --lib call_flow` | 27 项通过 |
| 回环格式/指纹/凭据 | 普通地址与 official 识别地址各 6 种入口，共 12 个场景；独立固定 Messages 路径断言，检查身份头、body、凭据与内部策略不外发 |
| official CPA 准入 | 独立子测试进程配置 retired Go transport；三条发送均先报配置错误，监听器未收到连接，且确认子进程确实执行 1 项测试 |
| FreeBuff 直达 | buffered、stream、probe 三入口先报 `Invalid call_flow`，随机回环监听器无连接 |
| `routing::credential_routing` | 29 项通过，含 raw/preset、坏 flow、模型 block 和不信任 discovery 目标 |
| `routing::protocol_resolution` / `routing::config` | 分别 34、96 项通过 |
| `db::routing` | 19 项通过，含合成 Hosted row/凭据合并、所选密钥与模型限制；未连接 PostgreSQL |
| candidate policy / stage-send 单元测试 | 分别 2、39 项通过，保护投影配对和已有发送决策 |
| `provider_discovery` / official API | 分别 9、36 项通过 |
| request profile / native Codex / FreeBuff config | 分别 5、18、5 项通过 |
| Rust 编译 | `cargo check --locked --all-targets --jobs 4 --no-default-features --features runtime-defaults` 通过 |
| 前端 | TypeScript 通过；5 文件、25 项测试通过，含真实编辑 hook 和 ledger 读回 |
| 规范 | Rustfmt、checker 自测 31 项、有效行 ratchet、UTF-8 无 BOM、`git diff --check` 通过 |

过滤器之间有交集，不把上述数量相加作为独立测试数。编译仍存在原有 unused/dead-code 等
warning；本轮引入后不再使用的三个 import 已移除，没有清理不相关执行器。

先记录前端 flow-only 显示失败，以及 Redis 丢策略、错误策略回退、候选协议元数据不同步的
回归失败，再修复转绿。中间验证还发现 legacy `anthropic` 与 wire family 投影不一致，已统一
固定格式的 wire canonicalization。回环夹具增加路径前缀后曾因旧断言误报；已改为两组独立
固定路径合同，不把该夹具问题计为生产缺陷。

新增源码模块最高 128 有效行；本轮改动文件最高 430，有效行基线和豁免未修改，依赖锁未变。
已对照修改前快照检查本轮生产 diff，并进行两路独立只读复核。

证据：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-call-flow-round2/` 中的
`final-call-flow.log`、`affected-results.json`、`check-all-targets.log`、`ui-regression.log`、
`typecheck.log`、`source-validation.json`、`expanded-before-counts.json`、`after-counts.json`、
`production-round2.diff` 及规范检查日志。早期失败日志保留，不覆盖成成功记录。

## 未执行

- 未连接 4200/4300，未读取 `.ng` 或真实账号，未调用付费上游。
- 未运行真实 PostgreSQL/Redis 服务链、逐模板上游或浏览器会话验收。
- CPA 测试证明本轮不再绕过准入，不冒充新的 CPA Rust 传输成功实测。
- 未运行全量业务回归、正式发布审计、EXE 构建；未提交、未推送。
- `TEMPLATE_ADAPTATION_RECEIPTS` 仍为空，框架测试不等于模板验收，不批量标蓝。

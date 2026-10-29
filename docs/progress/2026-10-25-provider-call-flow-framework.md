# 三适配器串行框架检查点

后续审查发现的候选入口、official 快捷分支及前端编辑缺口，已在
[第二轮修复](2026-10-25-provider-call-flow-round2.md)中处理；本页保留首次实现时的检查记录。

## 交付与兼容

新增 `protocol::call_flow` 的独立调用、指纹、专用适配器策略和请求局部准备合同。
路由配置与运行 payload 保留 typed `call_flow`；前端保存真实策略及旧 transport 投影。
配置编译、账号级指纹/Basispoints 覆盖、请求级协议发现、buffered/stream/JSON/binary/native
入口均已接入。专用执行 owner 继续负责多步骤会话、响应、流和取消，不重写 Gemini 内部流程。

固定格式不能被发现链或调试协议改成别的格式，模型负证据保留；无 `call_flow` 的旧文档
保持原行为和序列化。Web/Basispoints 明确使用执行器内指纹，未支持的显式 API 指纹或调用
格式组合在网络/会话工作前拒绝。详细字段和边界见 [调用框架](../provider-call-flow.md)。

现有二进制回退读取仍依赖同时保存的旧字段，不要求数据库迁移；旧版本不认识 `call_flow`，
也不具有本轮新增的固定格式约束。没有修改运行中的配置、凭据或数据库。

## 实际验证

| 检查 | 结果 |
| --- | --- |
| `cargo test ... --lib call_flow` | 14 项通过；先有目标格式仍为 Responses 的预期失败，再修复转绿 |
| 回环真实请求 | 1 项测试内覆盖 6 种入口：buffered、stream、JSON、wire stream、native Claude 快捷路由、wire JSON；检查 Messages 路径/请求体、Claude 身份、所选密钥、内部策略不外发；buffered SSE 聚合回复为 OK |
| 无网络拒绝 | buffered/stream/JSON/binary 对错误专用组合先返回 Invalid call_flow |
| `routing::config` | 96 项通过，包含 6 项新框架测试 |
| `routing::protocol_resolution` | 32 项通过 |
| `provider_discovery` | 9 项通过，使用回环或纯数据 |
| `upstream::request_profile` | 5 项通过 |
| `upstream::client::native_codex` | 18 项通过 |
| `upstream::basispoints` / `console::document` | 分别 2 项通过 |
| Rust 编译 | `cargo check --locked --all-targets --jobs 4 --no-default-features --features runtime-defaults` 通过 |
| 前端 | TypeScript 通过；6 文件、44 项测试通过，含调用策略、面板、凭据编辑和站点健康兼容 |
| 规范 | Rustfmt、checker 自测 31 项、有效行 ratchet、UTF-8 无 BOM、`git diff --check` 通过 |

各 Rust 过滤器存在交集，不将上表数量简单相加作为独立覆盖数。编译仍有原有的 unused 等
warning，未为了消除不相关警告改写执行器。前端首次测试误在仓库根启动，缺少 desktop 的
jsdom 配置；已从 `apps/desktop` 使用其真实 Vitest 配置重新运行并通过，没有改生产代码迁就错误入口。

为新增可选 payload 字段，67 个既有文件仅补充 `call_flow: None`；与本轮修改前快照对比
确认没有其他语义变动，其中包含 Gemini 等测试构造器，不表示接管其执行器开发。
新增源码模块最高 168 有效行；`providerCatalog.ts` 从 UI 阶段的 490 降为 480，其他相关文件
均在门限内；没有改基线、豁免或依赖锁。现有未提交的 UI/滚动加载等改动完整保留。

证据存于 `C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-call-flow/`：
`rust-results.json`、各过滤器日志、`cargo-check-all-targets.log`、`source-validation.json`、
`counts-before.json`、`counts-after.json`。源码验证覆盖当前工作区变更文本，不只本阶段文件。

## 未执行

- 不连接当前 4200 网关，不启动 4300，不使用真实账号或付费上游。
- 未进行逐模板真实调用验收；`TEMPLATE_ADAPTATION_RECEIPTS` 仍为空，不批量标蓝。
- 未运行全量业务回归、浏览器会话实测或正式发布审计。
- 未制作新 EXE、未提交、未推送；后续手测包仍使用 4300 隔离规则。

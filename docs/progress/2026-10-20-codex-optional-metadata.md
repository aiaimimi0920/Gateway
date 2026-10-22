# Codex 原生可选元数据保真

## 目标与差异依据

目标是消除转发层引入的上游可观察差异，不是保证两个独立 TLS 连接的随机字节相同。
本次继续使用固定参考版本 Codex CLI 0.155.1，没有改变运行实例或账号配置。

联网核对该版本官方源码发现两项原生请求元数据未被 Gateway 白名单覆盖：

- [`ResponsesClient::stream_request`](https://github.com/openai/codex/blob/rust-v0.155.1/codex-rs/codex-api/src/endpoint/responses.rs)
  设置 `x-openai-subagent`；来源包括 review、compact、memory_consolidation、collab_spawn 和自定义标签。
- [`default_headers`](https://github.com/openai/codex/blob/rust-v0.155.1/codex-rs/login/src/auth/default_client.rs)
  在配置驻留要求时设置 `x-openai-internal-codex-residency: us`。

新增 loopback sender 回归在修复前失败，实际收到的 HTTP 请求同时缺失这两个头。
不是从静态白名单推断测试通过。失败日志及参考源码保存于
`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-codex-fidelity-followup-20261020/`。

## 修改与安全边界

只扩展 `src/protocol/codex_wire.rs` 的固定名单，复用现有入口校验、顺序记录及 native sender。
不增加后台任务、重试、连接或新存储；每项仍限定为非空、至多 1024 bytes 的可见 ASCII。
不会根据网关自己的执行状态生成子代理身份或驻留要求。
服务商显式头配置保持较高优先级，上游凭证仍由选定账号提供。
不开放 wildcard、Cookie、入口认证、Gateway 控制头或任意 `x-openai-*` 头。
此变更不赋予管理权限，也不把驻留要求解释成上游已经满足该要求。

回归覆盖五种子代理来源、服务商覆盖、缺失时不补头、原生头顺序，以及凭证和控制头隔离。
另有入口顺序测试及长度、空值、CRLF 和非白名单负例。

## 验收状态

修复前定向测试：预期失败，接收端缺失两个原生头。
修复后 `cargo test --locked --offline --lib native_ -- --test-threads=1`：48 passed。
`cargo test --locked --offline --lib protocol::codex_wire::tests -- --test-threads=1`：2 passed。
checker tests：31 passed；effective-line ratchet、formatter、`git diff --check` 通过。
修改的源码和文档均已校验为 UTF-8 无 BOM。
`cargo check --locked --offline --all-targets` 通过，0 errors、43 warnings；
warning 来自未改动区域的 unused/dead-code 项，不将本轮描述成零 warning。
本轮未执行全量测试或重新制作 EXE；旧全量结果不代替本轮全量回归。

有效行数：生产模块 68 → 90；sender 测试 151 → 227；入口测试 72 → 92。
未修改行数 baseline、policy 或 exception。

本记录仅涉及两项元数据差异，不代表其他可选头、完整 HTTPS、重试和连接复用行为已完成对齐。
未使用真实服务商账户；未发布、部署或重启实例。旧 debug EXE 与旧抓包记录不代表本次源码。

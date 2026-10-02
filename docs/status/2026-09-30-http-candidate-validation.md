# HTTP 客户端候选与有界读取接线，2026-09-30

接续 [HTTP 迁移基线](2026-09-30-http-client-migration-baseline.md) 和
[SQLx/RSA 闭环](2026-09-30-sqlx-rsa-removal.md)。本批取得官方候选、工具链和锁图，
完成隔离 TLS/transport 验证，并在现有客户端上迁移三个有界读取点。
**生产依赖仍为 rquest，整体依赖安全未关闭，不生成新正式 release。**

## 联网阻塞已恢复，但旧错误根因未确证

本批显式 IPv4 直连和现有系统代理两条路径均成功访问 crates.io metadata；
官方 sparse index、crate archives 和 Rust 分发 manifest 也下载成功。
请求保持证书验证。代理只通过本次子进程参数选择现有的
`http://127.0.0.1:42344`，没有修改代理、DNS、路由、证书或 FlClash 配置。

此前的 TLS EOF 不能据此归因于某一具体设置。上一个检查点的联网阻塞已解除，
不是旧失败原因已得到证明，也不是生产迁移已完成。

## 官方候选和隔离工具链

官方 metadata 与校验后的源码确认：

| 包 | 本次官方状态 | MSRV | 选择理由 |
| --- | --- | --- | --- |
| wreq 0.15.3 | stable，未 yanked | 1.85 | 仍依赖 LRU 0.13，不能清除当前告警 |
| wreq 0.16.1 | 最新 stable，未 yanked | 1.98 | 依赖 LRU 0.18，进入隔离验证 |
| wreq-util 0.2.0 | 最新 stable，未 yanked | 1.98 | 对应 wreq 0.16 的官方 profile 实现 |

三个 archive 的 SHA-256：

```text
wreq 0.15.3     b1248467bcb6aafde3dae847973df892e656ed376c014d11743fd437efac6560
wreq 0.16.1     16454175c8cb23d4a7285bab53b72460686195f5aef655d1abad70ab97ef46d4
wreq-util 0.2.0 833a5528fd9346f1f61a351104878be96024b5905f39774fc66909502da3acd5
```

Rust 1.98.0 官方 manifest 日期为 2026-08-20；其 SHA-256 与官方 `.sha256` 一致。
工具链仅安装在本批证据目录的 `rustup/`，通过子进程 `RUSTUP_HOME` 使用。
仓库 pin、正常用户 Rustup home 和原默认 Rust 1.95.0 均未修改。

候选只在 `client-probe/` 解析，保留消费端依赖别名。
明确启用 roots、Tokio、stream、JSON、form、query、charset、系统代理及四种解压 features。
候选锁图解析到 `wreq 0.16.1`、`wreq-util 0.2.0`、`lru 0.18.5` 和 `btls 0.5.6`，
没有旧 rquest。**官方传递依赖 `wreq-rt 0.2.2-rc.4` 是 prerelease；
不能把 stable 顶层客户端描述为全 stable 依赖图。**

## 隔离验证结果与明确差异

- 3/3 TLS wire 基线通过。仅将 fixture 参数类型由 Emulation 改为 Profile，
  cipher、curve、signature、ALPN、TLS 版本和 Chrome ALPS 期望不变。
- 8/8 API/transport 检查通过：http 类型互通、简单 query/form、GBK/BOM、URI 秘密剥离、
  增量限额、来源链、redirect 最终 URI、connect/timeout 分离，以及收到响应后的
  截断/停滞 body 不变成 connect retry。
- 候选 151 项依赖的 `cargo audit --no-fetch --deny warnings` 退出 0，
  vulnerabilities 为 0，warnings 为空；没有 ignore、no-yanked 或 target 过滤。
  RustSec 本地 checkout clean，HEAD 与本批联网核实的官方 HEAD 一致：
  `9b3a3b73a7f42606494c943e95f8196e9994df46`。复用该 checkout，不声称本批重新 fetch。

首轮 body-error 测试错误地假设 `is_body()`，旧客户端实际运行证明其为 `is_decode()`。
进一步运行发现候选的重建响应确实不同：`Response::from(...).bytes_stream()` 将注入的
IO 错误标为 `is_request()`，不是 `is_decode()`。官方新实现没有旧 Decoder 包装，
这一差异已在最终候选测试中明确断言和记录，**没有把低层类型变化算成保留原语义**。

当前 `src/error/classification.rs::classify_network_error` 只按 `is_timeout()`、
`is_connect()`、status、source/message 形成 Gateway 决策，没有 request/decode 分支。
候选来源链和这两个控制标志保留，真实 loopback 截断和 body deadline 也有保护。
这不证明所有原始错误文案一致；生产接入仍需验证 Gateway 的实际脱敏、分类和重试合同。

## 已实施的生产兼容切片

三个消费 owner 从 `.chunk()` 改为被 pin 的增量 `.bytes_stream()`，没有改成无界 `.bytes()`：

| owner | 保留的 decoded-byte 上限 | 保留的业务边界 |
| --- | ---: | --- |
| `src/protocol/chatgpt/codex_client.rs::read_json` | 2 MiB | 目录 JSON、固定错误文案、模型选择 |
| `src/provider_runtime/codex_model_probe.rs::generate` | 256 KiB | 单凭证调用、completed 非空输出、usage、脱敏、60 秒 deadline |
| `src/provider_runtime/console_model_probe.rs::generate` | 64 KiB | NVIDIA 单凭证/模型归属、拒绝空响应和截断、usage、脱敏、60 秒 deadline |

每个读取在添加 chunk 前检查大小；超限、错误、取消会释放局部 stream，不创建后台任务，
不读取后续 chunk。不改变凭证回退、记录 owner、HTTP status 或 probe 成功条件。
限额约束累计保留字节，不是底层单个 decoded chunk 的分配量或进程总内存的上界。

新 `tests/http_client_body_contract.rs` 直接测试现有 `read_json`，不是替代实现：
精确上限/跨 chunk JSON、超限后不再 poll 并释放、固定秘密无关错误、取消释放，
以及旧客户端真实 incoming-stream 错误分类和来源链。
测试在修改前的 `.chunk()` owner 和修改后的 owner 上均要求执行相同 5 项。
初次精确上限 fixture 多计算了一字节，已按实际 JSON 固定字节数修正；失败日志保留，
没有为该 fixture 改生产限额。

有效行数分别为：目录 reader **90 → 90**、Codex probe **231 → 234**、
NVIDIA probe **237 → 240**，新 body 合同 **99**。都在 250 行以内，
没有新增软例外，没有修改 checker、baseline 或 immutable runtime assets。

回滚材料是本批证据目录中的三个 `before-*.rs` 原始字节及 `before-source-hashes.json`。
旧客户端仍在使用，新增合同已经验证旧 owner；这次回滚无需改依赖、数据库或凭证。
恢复前仍需检查是否有后续 owner 修改，不能直接覆盖别人的新工作。

## 尚未切换的接口与下一阶段

生产 manifest、三份 Cargo lock、工具链、安全策略、S06 Rust/Gemini、真实数据库、
凭证和现有 release 都不在本批写范围内。此前 SQLx/RSA 工作继续保留。

余下核心迁移边界是 Gateway 自己的协议 stream error：SSE、Anthropic、Bedrock 和
tool injection 不能继续借用新客户端不可公开构造的 Error。建议边界是携带原始
transport error 与本地 protocol/decode error 的小型领域类型；在真实响应进入协议层时
转换一次，协议/观察层共享该类型，保留 source 和终止/释放语义。
该建议尚未实施，不能通过构造假请求取得 Error 或增加大型客户端 facade 绕开。
接入还需核对 URI 最终地址、OAuth/quota 秘密剥离、serializer 输入和系统代理行为，
并与保留的 S06 owner 协调涉及其 public stream 类型的改动。

桌面 GLib 是另一条 owning-parent 链。Tauri/Wry/GTK 当前限制 GLib 0.18；单加新版
只会并存，Windows 不匹配该 GTK target 边也不能把全锁图审计改绿。本批不修改桌面图。

## 证据与验收边界

生产兼容切片最终门禁：

| 检查 | 实际结果 |
| --- | --- |
| 原 owner 的新 body 合同 | 5/5 |
| 修改后 owner 的相同 body 合同 | 5/5 |
| 目录模型选择 / Codex 完成响应判定 | 1/1、3/3 |
| 根 Windows `cargo check --offline --locked --all-targets` | 通过 |
| 根 `cargo fmt --all -- --check` | 通过 |
| checker 测试 | 31/31 |
| 权威 ratchet | 通过；2610 文件、14 个单列 runtime assets，受治理源码 `>700=0` |
| `git diff --check`、新增文本 UTF-8 无 BOM / 尾空白 | 通过 |

编译仍有既有 Gemini 等 unused/dead-code warnings，不把成功编译描述为零 warning。
checker 的 `--json` 只允许仓库内相对路径；首次指定外部证据路径被正确拒绝。
最终直接运行同一 ratchet、将 stdout 回执保存到外部目录，没有改 checker 的路径边界，
没有重跑已通过且源码未变化的六项前置门禁，也没有覆盖既有 dirty cache。

最终保护核对确认：本批仅修改三个已有 reader，**3755 个其他既有输入 hash 不变**，
三个既有 dirty cache 全部保留。Gateway 未提交；独立状态为 Neuro 93、Gateway 41、
Platform 1、Talk 2 项，Hook/Loom/Tea clean。其他仓库没有被本批写入或清理。

全部临时源码、候选工具链、target、请求、初次失败和最终回执放在：

`C:\Users\Public\nas_home\AI\GameEditor\linshi\gateway-http-resolution-20260930-172133`

候选回执：`candidate-gate-receipts.json`、`candidate-audit-receipt.json`；
生产读取切片：`reader-before-receipts.json`、`reader-after-receipts.json`；
有效行数：`reader-lines-before.json`、`reader-lines-after.json`；
源码保护和独立仓库状态由最终 `completion.json` 汇总。

本批 loopback/内存测试不使用付费上游或真实凭证，不写数据库。
隔离候选成功不等于 Gateway 新依赖编译、真实 Grok/ChatGPT/NVIDIA、桌面、Linux、
系统代理、原生产品和 release 验收成功。没有 commit、push、Docker 重启或发布操作。

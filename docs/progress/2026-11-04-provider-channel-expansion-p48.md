# P48：Kiro credential 授权上下文与持久刷新接线

日期：2026-11-04（Asia/Shanghai）。状态：本批实现、主配置及代表性 feature-off 定向验证通过。总目标与 R02/W02 继续 active，模板数量保持 12，没有新增整卡 `offline_ready`。

## 1. 本批范围与不变量

- 新增 credential-only optional `kiro_oauth`：认证方法、directory Region/Start URL、注册 client ID/secret、注册绝对 expiry、显式 scopes 和固定 64 位小写 hex machine ID。字段缺省不改变旧 document、grant JSON 或 material ID；不升级数据库版本，不自动改写旧凭据。
- registered client secret 进入原 Console redaction、内嵌秘密拒绝、SecretPatch replace/Keep/clear、稳定 credential ID 和最终完整目的地比较。只显示非秘密 metadata；Debug 不输出注册材料。清空 secret 可保留 metadata，但不再拥有 refresh capability，不能回落 generic form。
- 注册 context 在一次授权内不可变。refresh Result 从原 material clone，不提供更换注册 context 的输入；Result/replay、runtime owner、持久投影与 cache 都比较完整原 context。注册 expiry 不等于 access expiry，恢复时仅从原绝对注册 expiry 重建剩余 deadline。
- material identity 包含存在的 Kiro context；consumable RT family 仍为 endpoint/client/RT，不能通过修改 scope、注册 secret/expiry 或机器 metadata 绕过未决 Sending。
- 替换 P47 的临时 gate 为专用 prepare，复用原 prepare→reserve→execute→Result→authority→cache/restart；无 context 的旧 Kiro/AWS OIDC 仍在 reserve 前 recoverable 拒绝，不新增 scheduler 或私有存储。缺 access expiry 不制造寿命、不发布新 access，也不重放同一已发送 grant。
- typed 业务 payload 只允许固定标准商业 AWS Kiro/Q 服务入口、generate path 与 bearer；拒绝 provider access 继承、legacy auth body aliases、认证/Host/UA override、external keepalive 和非 direct 执行。固定 machine ID 被投影给既有 headers owner，不随 RT 轮换变化。商业 OIDC Region allowlist 是目的地策略，不是该 Region 的 Kiro 模型权益证明。
- 业务最终发送禁止 Kiro redirect；缓存 normal CAS 与 restart repair 均绑定完整原 context，typed normal cache 不把已清空的 RT 当作旧 legacy 缺省恢复。

## 2. 实际红测与修复

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-provider-expansion-20261104-p48`。

1. `red-context.log` 首次测试 import 错误，不算行为红测。修正测试路径后的 `red-context-v2.log` 是真实 **0 passed / 3 failed**：context roundtrip 丢失、secret descriptor 缺失、注册 secret 变化未改变 material identity。`red-source-verification.json` 证明当时最初 26 项既有输入与 before 原字节一致。
2. `green-context-initial.log` 与 `red-header-and-context.log` 为新增测试 module 路径/私有 API 的夹具编译失败，均保留，不算生产行为失败或绿灯。
3. `red-lib-kiro.log` 是实际 **47 passed / 1 failed**；最终发送跟随 307 得到 200，明确暴露原 Kiro 业务 redirect 缺口。
4. `red-header-context-v2.log` 是实际 **8 passed / 5 failed**；分别暴露整个 optional context 无法一次删除、typed access 缺少原子/长度检查、durable 原材料同类检查遗漏、`api-key` 旧 header 可继承发送、external keepalive 可继承并接收 bearer。
5. 对应修复在 owning boundaries 完成，不修改测试期望来掩盖失败。normal cache 缺失 RT 的旧 generic 兼容不适用于新 typed context；新增保护仅限制 typed Kiro，不改变无 context 的旧 cache 语义。
6. 扩大的 `regression-grok.log` 运行 75 项时，在既有完整 buffered/stream wire 测试触发 `0xc00000fd / STATUS_STACK_OVERFLOW`，不是一次完整通过。本轮驱动漏设 P39/P40/P44 已记录的 16 MiB 测试线程栈；保留原日志，按相同源码和编译 binary 用原测试预算执行 `stack-replay.log`（1 passed）与 `regression-grok-v2.log`（75 passed）。不修改生产栈、不忽略用例、不把原异常退出纳入通过计数，也不报告默认栈问题已经修复。
7. 代表性 line-off 首次 refresh 为 3 passed / 2 failed，原因是两个新材料夹具错误地依赖已关闭的 `kiro` preset，实际在配置准入即以 `Unknown preset 'kiro'` 拒绝，尚未测到 prepare。只把这两个夹具改为显式 raw Kiro adapter/bearer，继续测试相同的 typed context 与 feature gate，不修改生产准入或弱化断言。`source-green-input-v3.json` 相对 v2 只变 `src/token_refresh/tests/kiro_material.rs`；修正后 feature-off refresh 5 passed，主配置材料 2 passed。其他已绿检查的生产输入未变，不重复编译全部回归。

## 3. 最终验证与来源

主配置合计 **445 个不同 Rust 测试通过，其中本批新增 31 项**。另有 **11 ignored**：10 个要求显式一次性 Redis URL 的 opt-in，1 个要求显式导出合成前端 fixtures 的旧 Session roundtrip；均未计入通过，本轮未为它们创建 Redis 或录制材料。代表性 Kiro line-off 另有 **15 项通过**，其中仅 `compiled_out_line_rejects_registration_without_network` 是主配置未执行的测试名，其余不叠加到 445。

| 检查 | 实际结果与证据 |
| --- | --- |
| Kiro 首轮核心 lib / context 合同 | `green-lib-kiro.log`：49 passed；`green-context.log`：14 passed |
| 扩大 Kiro 协议、业务与持久链 | `regression-kiro.log`：94 passed |
| 共享 refresh 生命周期 | `regression-refresh.log`：124 passed / 4 ignored |
| 路由 refresh 材料与交接 | `regression-routing-refresh.log`：9 passed |
| 相邻 Accio / Grok | `regression-accio.log`：40 passed；`regression-grok-v2.log`：75 passed |
| Hosted credential merge | `regression-hosted-merge.log`：9 passed |
| Console/旧格式/秘密/grant 合同 | `regression-console-contracts.log`：2 + 10 + 70 + 7 + 5 passed，合计 7 ignored；与其他 suite 重叠的名称不重复累加 |
| 代表性 Kiro line 关闭 | `feature-disabled-protocol.log`：10 passed；`feature-disabled-refresh-v2.log`：5 passed，均 exit 0 |
| 最后材料夹具主配置重验 | `green-kiro-material-v2.log`：2 passed；`feature-refresh-v2-exits.json` 两项 exit 0 |
| checker | `checker.log`：31 passed；checker 实现未变，不无故重复 |
| formatter / ratchet / diff | `formatter-final.log`、`ratchet-final.log`、`diff-check-final.log`：exit 0；ratchet 4278 files、14 immutable runtime artifacts、governed source >700=0 |
| 来源与新增测试核对 | `verification-summary.json`：45 项 Rust 输入与 v3 逐项 SHA256 相同，31 项新增测试全部有主配置通过记录，mismatches/failures 为空；v2→v3 只变上述私有测试夹具 |
| 本批文本与范围 | `source-frozen-final.json` / `audit-final.log` 保存文档补齐后的 UTF-8 无 BOM、whitespace、有效行和 `scope.diff`；文档补齐后另做 `git diff --check` |

`source-green-input.json` 冻结初始绿测输入；只读复核后补充“删除 context 并省略已删 nested patch，仅 Keep access/RT 必须命中 `secret_keep_destination_changed`”的精确外部合同测试，再由 v2 冻结。该补充不改 lib 输入，外部合同在其后编译。v3 仅修正最后两个私有测试夹具，两种 feature 组合均已重验；所有生产 Rust 沿用扩大回归时的原字节。最终统计按完整测试名去重，不叠加重跑和重叠 suite，也不计入失败日志。

后端 Cargo 显式 `--locked --jobs 4 --no-default-features`，主配置使用 `--features runtime-defaults`；代表性关闭组合展开 runtime-defaults 后仅移除 `line-kiro-official-vendor-api`，不再启用会把它加回的 runtime-defaults。未触发默认 management-ui 发布/清理 hook，未运行全仓回归或正式发布门禁。

本批所有新增/修改 Rust 均不超过 500 有效行；新增最大 250，影响范围内既有最大 492→493，没有新 501+ 例外、baseline 重写或 oversized debt 增长。较大的共享 owner 为 `server_cache.rs` 249→300、`secrets.rs` 375→383；各自仍只承担既有 cache CAS 与秘密生命周期职责。

测试只使用合成材料和随机 loopback。HTTP fixture 复用 P47 的有界、可 join peer；替换 origin 的 task-local 仅 test 编译，不开放生产 OAuth 目标配置。cache RESP witness 只证明生产 GET/CAS wire 与本地 patch 合同，不冒充真实 Redis Lua 执行。

## 4. 尚未完成的 Kiro 与总计划范围

1. 旧 Kiro folder/历史材料归一化、标准字段与 aliases 冲突处理及首次 machine ID 保真仍未实现；无 typed context 的旧数据不被静默改写。
2. 普通 Console 管理登录、device receipt/poll/cancel、SecretPatch/CAS 保存及 UI 未接；本批配置/context 与刷新接线不等于可从界面完成新登录。
3. 当前最小 scope、Builder ID verification URL 真实样本、profile/subscription 发现与选择、profile ARN/业务 Region、第三方客户端使用边界及各产品额度仍分别未证。P47 固定公开依据见 [Kiro 调查合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/plan/provider-kiro-oauth-contract.md)。
4. Accio/Grok 余项、Qwen/Vertex 未决认证、其余原范围和 P50/P60/P70 不变，不能因 Kiro 这一子链转绿就删除或改称整个计划完成。真实凭据仍按用户要求最后集中录制和逐渠道验收。

本批没有读取 `.ng` 或历史凭据、访问真实账号/模型、消费付费额度、创建/删除容器、出包、替换运行实例、提交或推送；未撤销并行改动，未接管 S06 Rust/Gemini lane。

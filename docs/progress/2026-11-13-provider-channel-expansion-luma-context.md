# Luma MCP 授权材料往返增量

## 范围与选择

实现非秘密 `luma_oauth::AuthorizationContext`：从已接受的 TokenMaterial 取得 client、redirect 和原绝对 expiry，序列化固定 issuer/resource/token endpoint；从持久化 metadata 与单独提供的秘密重新组合 TokenMaterial，继续适用于原有 inspect_tools。

不把 bearer 或 refresh/id token 放入 context，不从 JWT 推断用户或团队，不因保存/恢复重新计算有效期。未知 expiry 保持未知，已过期 metadata 可以反序列化供管理查看，但恢复可用材料失败。固定authority、严格loopback redirect、client和bearer形状均作入口校验；未知字段、重复已知字段被拒绝。Context Debug 不输出client/redirect。

这不是防篡改凭据容器，也不能证明任意metadata与bearer原本属于同一次授权。正常往返不延长期限；未来接入存储时，metadata（包括expiry）必须纳入完整目的地/Keep绑定和正常秘密批准，不能对任意管理请求直接开放restore。

## 为什么尚未开放账号保存

现有 `protocol/luma_agents.rs::owns_base` 以整个 `agents.lumalabs.ai` host识别静态Key产品；`routing/config/luma_agents.rs` 和统一 `call_flow/preparation.rs` 会将 `/mcp` 同样送入静态Key校验。仅添加schema、保存按钮或放宽host判断，不构成安全完整的MCP账号。

下一保存切片必须同时完成：独立MCP产品身份与路由准入、credential-only context、Keep完整绑定、普通sender/probe不可消费的边界、管理target/Ready/import/CAS及专用目录消费者。不沿用Web/Agents REST凭据，不以tools/list结果写模型健康成功。

本批仅修改授权底座，没有新增schema字段、管理API、UI、运行账号、模板或storage version；也没有外部请求、凭据/数据库写入、出包或Git提交。

## 验证

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-luma-context-20261113`。

- `red.log`：新AuthorizationContext API尚不存在时的编译失败（E0432，退出101），不是已执行的行为失败。
- 独立只读审查未发现本切片阻断问题。
- 格式、checker测试、有效行ratchet通过。
- `green.log`：Luma OAuth定向17 passed、0 failed、0 ignored（新增3项），执行0.45秒；含共享锁等待的Cargo总时间26m49s，不作为构建性能基准。
- 本批有效行：luma_oauth入口52、context109、context_tests97、material94；不需要例外，不修改baseline/policy。
- `production-check.log`：非测试 `cargo check --locked --offline --lib --no-default-features --features runtime-defaults -j 4` 通过（含共享锁等待15m15s）；113项既有warning，未指向本批luma_oauth模块。
- `scope-hashes.json`的4项源码SHA256复核一致，本批源码及文档空白检查通过。未运行全量回归、真实上游、包或界面验收。

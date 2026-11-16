# Luma 只读 MCP 目录链路增量

## 范围

新增 `src/protocol/luma_mcp.rs` 及其子模块，从已验证的 `luma_oauth::TokenMaterial` 读取授权材料，仅对固定 MCP resource 执行 initialize、notifications/initialized、分页 tools/list。不是通用 MCP 客户端，也不提供 tools/call。

本批不重新授权、不读取现有凭据、不发起真实上游请求，不修改数据库或运行实例，不构建安装包、不提交 Git。此前 26 项真实工具定义的调查证据仍独立保留，不将本地测试当真实业务验收。

## 实现边界

- JSON/SSE 响应均校验 JSON-RPC 版本、请求 ID 与重复字段；SSE 得到对应结果立即结束，不依赖服务端 EOF。仅忽略规定的 progress/message 通知，不执行反向请求或目录中的文本、URL、schema 引用。
- 每次检查独立持有 session，禁止跳转和默认 headers；bearer/session 标记 sensitive，错误不回显上游内容。调用方仍必须提供专用无 Cookie jar、无自定义重试的 HTTP client。
- 整次 30 秒、单响应 2 MiB、累计读取 4 MiB、SSE 单帧 1 MiB/最多 256 帧、最多 8 页/256 工具。分页失败、重复名字或 cursor 循环不返回部分目录。
- 已知过期材料不发请求；取消/超时释放本地 HTTP body，不创建后台读取任务。未实现远端 DELETE、远端撤销、GET/SSE 监听或断线恢复，不能声称服务器会话已清理。
- 工具目录是未经信任的数据，不是工具可用性、费用或执行许可的证明。

## 验证状态

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-luma-mcp-core-20261113`。

- `red.log`：新 API 尚不存在时的预期编译失败，退出 101；不是已运行的行为失败。
- 独立只读源码审查未发现阻断问题。
- `green.log`：12 passed、0 failed、0 ignored，测试执行 0.57 秒；Cargo 报告包含共享构建锁等待的总耗时 28m22s，不作为冷构建或热增量性能基准。
- `production-check-retry.log`：非测试 `cargo check --locked --offline --lib --no-default-features --features runtime-defaults -j 4` 通过，3m32s，113项既有warning，未指向本批luma_mcp模块。首次检查仅等待锁，工具会话更新后进程已不存在，保留原`production-check.log`；未把中断当成功。
- 格式检查、checker 测试、有效行 ratchet、tracked diff 与新增源码空白检查通过。`scope-hashes.json`记录9项源码 SHA256。
- 有效行：入口32、catalog60、decode93、inspection77、transport137、decode_tests42、fixture168、tests255。tests只承担该只读检查链的本地HTTP契约测试。既有protocol/mod.rs从119增至120，仅新增模块声明；没有修改行数baseline或policy。

## 未完成

管理保存、重新授权 UI、动态注册入口、refresh、实际 model_list、生成/查询/取消/费用、完整公共渠道和包验收仍未完成。正常保存必须保留独立 MCP 身份与受众绑定，不能把 bearer 放入原有 App Web 或 Agents REST 账号。

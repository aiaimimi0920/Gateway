# Luma MCP 只读目录会话收尾

本批仅补只读工具目录的 HTTP session 清理，不注册客户端、不登录、不生成或下载作品、不消费积分。

## 实现与边界

- `inspection.rs` 将连接所有者放在目录 timeout 外：目录 future 和响应 body 释放后，成功／失败／超时均进入一次有界清理；保留原目录结果和管理 API 形状。
- `cleanup.rs` 只向原固定 MCP endpoint 发送 DELETE，携带本次 session、原 bearer 和协议版本。单次最多 2 秒、禁止重定向、隔离默认 headers，不读取响应正文；目录原有 30 秒预算不变，总上限 32 秒。
- 无 session 不请求；已知过期令牌不发送。200／204、404、405、其他结果分别记录 Closed、AlreadyClosed、Unsupported、Unconfirmed。日志仅含分类枚举，不含 token、session、URL 或响应正文；清理失败不覆盖目录错误，也不丢弃成功的目录。
- 强制 drop／abort 不另起任务，不承诺远端关闭。运行实例停止和用户取消后的受监督远端清理仍未完成；不会借此调用业务 `session_cancel`，也不等于注销／撤销 OAuth。

## 协议依据

本轮匿名联网读取 [MCP 2025-03-26 Streamable HTTP 规范](https://modelcontextprotocol.io/specification/2025-03-26/basic/transports)：不再需要 session 时 SHOULD 发 DELETE，服务端 MAY 返回 405；已终止 session 的后续请求返回 404。没有访问真实 Luma 账号。

## 验证

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-luma-mcp-cleanup-20261114`。

- `red.log`：先新增 4 项 loopback 测试，全部因 DELETE 次数为 0（预期 1）失败，再实现生产代码。
- 第一轮 `green.log`：131 passed、3 failed、2 ignored。失败来自新增测试假设：既有 HTTP 内层 timeout 与外层 deadline 竞争，合同允许 Transport／Timeout；取消测试只等待请求到达，尚不能保证响应流已构造。修正测试以遵循原错误合同，并让夹具在 stream 首次 poll 时提供就绪证据，没有修改生产错误语义。中间运行不当作最终通过证据。
- 新增 9 项测试覆盖 session／默认凭据隔离、无 session、404／405／错误、不重试、真实可达跳转目标不被访问、过期令牌、目录／清理双超时，以及目录读取中／DELETE 等待中取消。独立只读审查未发现确定的生产缺陷；不将取消测试当作远端资源释放证明。
- checker 31 项、ratchet 与 scoped rustfmt 已通过；本批最大文件为夹具 214 有效行，测试 208 行，生产清理模块 48 行。未改 baseline／policy／例外。
- `green-final.log` 和最终 `verified-tests.log` 均为 **134 passed、0 failed、2 ignored**；后者运行 29.60 秒。两项 ignored 是既有独立浏览器专项，不冒充通过。
- `production-check.log`：非测试 `cargo check --locked --offline --lib --no-default-features --features runtime-defaults -j 4` exit 0，存在 warnings。本批 5 项源码哈希随后复核一致；额外排队的同命令 `verified-check.log` 仅等待其他任务的共享锁，已按进程父子链确认后停止本任务的重复检查，不作为通过证据，也未停止其他任务。无需用重复构建替代已有同源码的成功编译。
- `scope-hashes.json` 固定最终源码；范围 UTF-8 无 BOM、空白与 `git diff --check` 通过。未提交、推送、出包或修改运行实例、凭据、数据库。
- 没有执行真实供应商、浏览器／原生整包、跨机器授权或付费业务验收。

## 仍未完成

注册回执跨进程恢复、refresh、实际模型发现、生成／取消／结果／费用、真实账号联合，以及窄屏和原生包等继续保留。不将本次 session 清理增量计为整个 Luma 渠道完成，也不减少总清单整产品数量。

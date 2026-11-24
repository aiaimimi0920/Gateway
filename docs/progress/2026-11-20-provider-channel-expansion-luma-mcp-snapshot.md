# R14：Luma MCP 工具合同快照

日期：2026-11-20（Asia/Shanghai）。承接只读目录与单任务查询验收，本批补齐成功工具目录的保存入口，避免当前合同再次只留在临时目录或单次界面中。未使用真实账号、刷新 token 或执行创作工具。

## 合同核查与实现

联网核实官方 MCP 页面、Claude／ChatGPT 指南及[官方插件仓库](https://github.com/lumalabs/luma-plugins/tree/e02d83de796d56525ef64376fd812b1f2cce036b)。该固定 commit 的递归目录树未截断，只有连接配置、说明、license 与图标；README 明确插件自身无代码，`.mcp.json` 指向 `https://agents.lumalabs.ai/mcp`。这些材料没有提供当前工具的输入／输出 schema，不能补齐 MCP 创作的业务合同。公开插件原文、URL、UTC、HTTP 状态及 SHA-256 纳入本批归档。

原 `ToolDefinition` 会丢弃 `annotations`，本批将其保留为可选对象，并在目录解析边界拒绝非对象值。缺省仍为未知；不据 hints 授权执行、安全重试或费用语义。原单响应 2 MiB、累计 4 MiB、256 项工具及分页预算继续生效；后页失败仍不返回部分成功。

正常已保存账号面板增加“导出工具目录”。读取成功后可下载全部工具，包含名称／说明、输入／输出 schema、annotations、固定 resource／协议版本和浏览器读取完成的 UTC。导出复用内存结果，不额外发送管理或上游请求。重新读取立即丢弃旧结果，取消／失败／关闭或账号、连接、授权、revision 变化后禁止导出旧范围结果。下载异常显示短错误，不回显异常正文，并释放临时链接。

文件是规范化目录快照，不是原始 HTTP 回执。通过字段投影排除账号 ID、revision、token、grant、session header 和工具顶层 `_meta`；schema／说明原文作为数据保留，不执行或加载其中引用，也不保证供应商任意文本天然不含敏感信息。真实合同归档前仍需检查其内容。没有新增数据库、后台任务、配置开关或生成调用。

## 验证

- 前端目录与快照回归 **24 passed**：完整多工具字段、中文、nested schema、未知 annotations、空目录、无额外请求、七类范围变更、取消／晚到、错误隔离与下载资源释放。
- 正常 main 网页构建和最终 TypeScript 检查通过。首轮 TypeScript 失败为新增测试中 spy 的 DOM instance 类型为 unknown，补充 `HTMLAnchorElement` 标注后通过，原失败日志保留。
- MCP Rust 定向回归 **27 passed / 0 failed / 1 ignored**；ignored 的正常网页宿主另行运行，**1 passed / 0 failed**。首次测试编译 13m13s；启动宿主复用同一构建，Cargo 准备 2.80s，不据此外推其他构建速度。
- 正常 main → 管理 TCP → 合成 MCP → JSON 实际下载通过。下载前后管理请求数保持 1；完整旧流程仍为 6 次管理请求、18 次 POST 和 6 次 DELETE。输入／输出／annotations／中文字段及身份排除核对通过，账号／revision 不变，错误和外网请求均为空，宽窄屏截图已检查。
- 浏览器与代理关闭、宿主 PID 和管理端口退出、未发现本轮临时目录对应的浏览器进程残留。
- 生产配置 `cargo check --locked --offline --lib --no-default-features --features runtime-defaults -j 4` 通过。Rust formatter、Node 语法、checker **31 passed**、最终有效行 ratchet 和 diff 检查均通过；未重复未改动的 Agents／Inference 回归或 feature-off 编译。

生产／测试有效行：`catalog.rs` 60→62、`tests.rs` 255→283、`browser_fixture.rs` 140→142、`LumaMcpToolsPanel.tsx` 72→78、`mcp-browser-check.mjs` 133→162；新增 `lumaMcpSnapshot.ts` 18、快照组件测试 97。没有修改规模 baseline 或引入超限文件。

## 接手与边界

持续入口仍为仓库中的 `tools/luma/mcp-browser-check.mjs` 和原合成 Rust 宿主；下载字段及失效行为已加入原正常 main 流程。[工具说明](../../tools/luma/README.md#保存-mcp-当前工具合同)给出真实合同保存与长期归档位置，不依赖 `linshi` 中的脚本或旧测试 EXE。

本批回执、源码哈希、公开证据、原始失败／通过日志和实际下载的合成 JSON 见 [verification.json](../evidence/luma/gateway-luma-mcp-snapshot-20261120/verification.json)。56 份证据（不含索引）、353,328 bytes 已按 SHA-256 归档，7 个范围源码文件最终哈希不变。归档不包含运行数据库、浏览器 profile 或构建载荷。

本批提供获取后可交接的合同快照，**没有恢复真实的当前 26 项目录，也没有完成 MCP 创作**。仍需当前完整工具合同及会话／工作区隔离、生成／等待／取消、资产、费用语义；Web 当前协议、自动追账／审计回填／跨进程／cash、真实供应商业务和新原生包继续见[剩余清单](../providers/luma-development-status.md)。R14、P50/P60/P70 与总计划均未整体关闭。

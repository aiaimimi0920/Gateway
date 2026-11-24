# R14：Luma MCP 只读目录正常网页联合验收

日期：2026-11-20（Asia/Shanghai）。承接 Files 与 Inference 网页联合子批，本批完成已保存 MCP 账号的工具目录、模型列表和 image/video 参数读取；使用正常 main 网页、真实管理 TCP／授权与合成 MCP 上游，没有运行真实供应商业务或创作工具。

## 实现与验收范围

修复工具目录面板请求敏感授权失败时遗漏 Promise 错误处理的问题。定向测试先复现缺少错误提示，再验证修复后显示可重试的中性错误、不回显拒绝原文、不发送目录请求，也不在授权到达后自动重放。处理方式与现有模型面板一致，账号／连接范围变化后不写入旧范围的错误。

持续入口为 `tools/luma/mcp-browser-check.mjs` 与 `src/protocol/luma_mcp/browser_host.rs`，夹具拆在同目录 `browser_fixture.rs`，复用正常网页构建和透明管理代理。仅测试配置下可逐请求映射到 loopback MCP；未绑定测试作用域会在发送前失败，生产端点仍固定。脚本、夹具及[双终端说明](../../tools/luma/README.md)随仓库维护，不依赖临时目录中的 EXE、数据库或浏览器 profile。

浏览器正常登录并进入账号的专用授权弹窗，展开／关闭／切换账号不触发读取或 OAuth 登录；显式操作覆盖工具目录、模型列表、image/video 参数、未知费率、audio 参数禁用、账号切换清空、空模型列表和上游非法目录的分类错误。带 script 标签的工具说明原样显示为文本，不执行脚本。1440px 与 390px 截图已实际检查，参数区不横向溢出，关闭操作可完成。

六次管理请求中五次为 200，一次预期分类错误为 503，全部绑定同一已保存账号和 revision。后端独立核对 18 次 POST、6 次 DELETE、六个独立 MCP session、固定工具及精确参数、账号 Bearer 与管理凭据头隔离，两个账号及配置 revision 均保持不变。DELETE 记录只证明合成服务收到对应收尾请求，不证明真实远端业务释放。

## 验证记录

- 浏览器与 Rust 宿主双通过，宿主 **1 passed / 0 failed**。
- MCP 后端回归 **26 passed / 0 failed / 1 ignored**；ignored 是本批已单独运行的 opt-in 浏览器宿主。
- 前端工具目录、模型面板与 OAuth 面板合计 **33 passed / 0 failed**。
- 正常 main 构建、TypeScript、生产配置 `cargo check`、Rust formatter、Node 语法、diff 检查及有效行门禁通过；checker **31 passed**。本批未额外重跑 feature-off 编译。
- 两次宿主的进程和监听端口均已退出，浏览器与代理关闭，没有本批临时 profile 对应的浏览器进程残留。

首轮浏览器脚本误认为“编辑账号”后还需点击二级按钮，实际页面已经直接打开专用 MCP 授权弹窗；当时目录请求为零。修正脚本入口及关闭按钮定位后通过，没有为测试修改生产导航。该失败及授权错误修复前的红测试均保留原始日志。

最终回执及长期证据见 [verification.json](../evidence/luma/gateway-luma-mcp-browser-20261120/verification.json)。51 份证据、346,086 bytes 已逐文件 SHA-256 校验归档。源码范围哈希在 Rust 编译期间采集，最终仅浏览器脚本因上述入口修正发生变化；八项 Rust／前端文件复核未变，不代表全仓编译前冻结。归档只包含相关日志、摘要、AX、截图与哈希，不复制数据库、profile 或完整 UI 构建目录。

## 边界

本批关闭只读目录／模型参数的正常网页联合与窄屏验收缺口。真实模型读取、真实远端 DELETE 效果、强制崩溃／跨机器／远程 Server 恢复仍未验。MCP 创作缺当前完整工具合同，Web 当前 wire/session、媒体追账／跨进程／cash 及新原生包仍列于[剩余清单](../providers/luma-development-status.md)。R14、P50/P60/P70 和 R01–R16 总计划尚未整体完成，Magpie 67 项不变。

# R14：Luma Files 正常网页联合验收

日期：2026-11-20（Asia/Shanghai）。承接 Files 签名上传子批，新增 opt-in Rust 宿主与可重复浏览器验证入口；生产实现未在此子批修改。

## 实际流程与结果

独立 Edge 会话加载正常 `main.tsx` 构建，经本机透明代理访问真实管理 TCP、登录和敏感操作授权，再由 Rust 测试层把上游流量重映射到不转发外网的合成服务。未使用组件预览或伪造管理 API 响应。

- 在已保存的 `luma-pipeline / exact` 账号上传三字节小文件：显示 pending、禁止复制，刷新后 ready，可复制精确 file_id。
- 带已有文件结果切换到另一账号再切回，结果及本地文件选择清空；该操作没有新增 Files 请求。
- 文件列表和下一页通过；后端核对 cursor 正确编码。
- 上传 8 MiB+1 字节文件，附带 TTL 与 user_id：浏览器发送原生 multipart，后端按 presign → PUT → complete 执行，完整字节和元数据匹配。
- 点击删除后取消没有新增请求；再次确认后只有一次 DELETE，页面显示“没有文件”。宽屏与 390px 窄屏截图已保存并检查，窄屏完成删除操作。
- **7 次 Files 管理请求均为 HTTP 200；9 次上游调用精确匹配。** Luma API 只收到精确账号凭据，签名 PUT 不携带授权、Cookie 或管理凭据。
- 后端独立断言通过：普通 Key 余额仍为 2，配置 revision 未变。浏览器、代理、管理 TCP、SQLite 和合成服务完成收尾；最终核对无测试宿主、监听端口或本轮浏览器进程残留。

## 验证及可重复入口

长期证据：[verification.json](../evidence/luma/gateway-luma-files-browser-20261120/verification.json)。正常 UI 构建成功；Rust 宿主显式运行 **1 passed / 0 failed / 0 ignored**。原 Files 的 16 项后端和 31 项前端结果仍引用上一子批，不算本次重跑。checker 31 项、最终规模门禁、Rust formatter、Node 语法和 scoped diff 检查通过。

首次 Cargo 命令对短测试名使用 `--exact`，实际运行零项；该回执只证明编译成功，不计入通过测试。随后使用完整名称。第一轮浏览器因沿用旧账号入口定位而失败，未发送 Files 请求，宿主收到 abort 后正常收尾并以失败退出。修正为正常“账号名称 → 编辑账号”导航后第二轮通过；失败日志均保留，没有修改生产行为或放宽断言。

构建约 19 分钟；同源码后续 Cargo 启动命中缓存。4487 项源码哈希在编译期间采集，Files 结束后的漂移核查只有浏览器脚本的导航修正；这不是编译开始前的全仓冻结证明。随后 Inference 子批新增了独立宿主并修复测试结果展示，这些后续变化不属于该漂移回执的时间范围。归档另记录最终相关源码及测试 harness 哈希，不把含继承改动的工作树全部归于本批。

持续使用的 `tools/luma/files-browser-check.mjs`、`normal-main-server.mjs`、Rust 宿主和运行说明均在仓库。删除 `linshi` 后仍可按 [工具说明](../../tools/luma/README.md)重建运行；不依赖旧日志或固定哈希 EXE。

## 仍未证明

三字节及填充二进制是运输夹具，不能证明真实媒体摄取、账号隔离、TTL 到期、资产复用或删除生效。未运行真实供应商业务、费用、Server/PG/Redis、跨进程或新原生包。64 MiB 本地上限不变；MCP 创作、Web 当前协议、截止后费用追账等继续列在[剩余清单](../providers/luma-development-status.md)。R14 整卡和 R01–R16 总计划仍未全部完成。

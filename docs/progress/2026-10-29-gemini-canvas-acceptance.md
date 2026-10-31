# Gemini Canvas 独立调用路径复测（2026-10-29）

> 本文保留首次失败现场，不代表当前修复状态。2026-10-30 已完成 browser-relay
> 文字链路修复与普通/SSE 真实验收，见 [后续修复记录](2026-10-30-gemini-canvas-host-repair.md)。

## 当前结论

**真实 Canvas 调用未通过，不登记模板完成标记。** 普通 Gemini Web 已通过的 HTTP
验收不覆盖 Canvas；本轮也没有用普通聊天、官方 API Key 或网页中发现的 Key 代替 Canvas。
此次没有修改生产执行器，没有构建新 EXE；只新增复测记录与隔离诊断材料。

用户要求先提交推送，再测试 Canvas。前一步已完成：

- 分支：`main`。
- commit：`fd74db9c374d8caa8309686592a0908c70376773`。
- 标题：`fix(gateway): complete Gemini Web import and HTTP acceptance`。
- `git push origin main` 成功，远端 `refs/heads/main` 已核对同一 SHA。
- 本文及后续 Canvas 诊断不在上述提交中；未再次提交或推送。

## 隔离边界与真实调用

证据目录：`linshi/gateway-gemini-canvas-acceptance-20261029-r1/`。
使用独立后端 4310、源码 UI 1437、浏览器池 HTTP 43121 / TLS 43122；数据、管理材料、
storage state 与原始页面材料留在该目录的 `private/`，不提交、不打包。
日常 4200 和上轮 Web 4300 均未重启、未修改数据或凭据；不使用默认浏览器池端口。

后端复用上轮冻结的 API devtest r3，不声称它包含最新桌面 UI：

- SHA256：`cf58d556cd8b4ca421002657c76f5160939eb2049bbc34bd2760df6f112a2751`。
- backend source fingerprint：`5a1a4e9172b73956aea80d04596434791a40723f479597ecc674530d3b946e41`。
- 浏览器 worker 来自当前 checkout；安全 wrapper 仅投影固定日志事件，不改变请求逻辑。

沿管理配置创建一个显式单账号、单模型精确路由，关闭自动测试，复用用户上轮专用登录
保留的 storage state。没有读取日常浏览器 profile，也没有要求重新登录。
本轮配置通过管理 API 建立，不能记作完整模板 UI 导入验收。

| 项目 | 实际值 |
| --- | --- |
| preset | `gemini-canvas-browser-relay` |
| adapter | `gemini_canvas_web_reverse_compatible` |
| provider | `canvas-browser-acceptance` |
| credential | `canvas-isolated-1793287600` |
| requested model | `gemini-3-flash-preview`；不证明底层模型已执行 |
| 普通非流式结果 | HTTP 403 / `Authentication`，15,453 ms |
| 审计终态 | `failed`，`upstreamStatus=403` |
| 路由尝试 | 1；唯一 provider 和 credential 均匹配 |
| 审计 ID | `gw-audit-05e557ee-4815-44d6-aa24-2b19503eb4c3` |
| 调用时间 | `2026-10-29T15:30:14.4151696Z` 至 `15:30:29.8035488Z` |

Google 错误的安全分类为 `unregistered callers` / `without established identity`，要求
API consumer identity。它不等于网页登录超时，也不足以判断用户上轮没有登录成功。
已消费 `browser-buffered.started.json`，没有重放该请求，也没有继续发送 SSE。

## 确认的执行身份断点

`src/upstream/client/canvas_relay.rs` 的 browser-relay 文字分支使用 `canvas_proxy`。
该模式在 `scripts/gemini-canvas-browser-pool-fetch-execution.mjs` 中加载分享页，
自动建立本地 connected client，再派发生成请求。

`scripts/gemini-canvas-browser-pool-connected-client.mjs` 把自有客户端注入 `entry.page`
主页面，并未明确选取真实 Canvas preview iframe。客户端
`scripts/gemini-canvas-connected-client.js` 最终调用原生 `fetch`；本地 WS 的 `apiKey`
只用于本地连接认证，不是 Google consumer identity。因此池健康、`connectedClientCount=1`
只能证明本地传输连接，不能证明请求经过 Google Canvas 宿主身份代理。

失败后的只读 worker 投影显示：仍在 share 页面，没有 `canvasProgramUrl`、`appPath`
或 `canvasProgramInvokeContract`。当前失败不能算“具体 Canvas app 执行过但回答错误”。

另有两条容易混淆的旧路径：普通 `gemini-canvas` / `gemini-canvas-chat` 的文字分支及
program relay 的部分文字执行仍可走 Web StreamGenerate。即使这些入口返回正确文本，
也不能证明 program-owned Canvas steady-state 正常。

## 真实预览应用的进一步只读检查

另建本轮专用、临时 Edge 检查会话，加载相同的保留状态。没有向聊天框发送提示词。
在分享页完成“继续”后，确实出现 Google 的 `scf.usercontent.goog` Canvas iframe，
其界面为 `Browser Proxy Client` / `Routing Google API requests via WebSocket`。

页面检查观察到：

- iframe 内 `window.fetch` 为 native，且没有 controlling service worker。
- 应用向宿主发送 `requestAuthIndex`，界面最终报告
  `Failed to get authIndex: authIndex postMessage timeout (10s)`。
- 应用默认显示 `ws://127.0.0.1:9998`；这不等于已经连接本轮 43122。
- 该 context 内存在主认证 Cookie 名称，但页面仍显示“登录”；进入账号限定 app 路径后
  返回访客界面，提示“登录即可保存活动记录”。Cookie 存在不等于浏览器会话恢复成功。
- 点击“试用 Gemini Canvas”没有取得具体 program path，因此未继续假定 materialization 成功。

检查阶段拦截生成端点及 loopback HTTP 请求，不执行仓库的 preview 自动探测 helper。
这些观察仅证明该隔离浏览器恢复与宿主桥接的现状，不否定此前 Gemini Web 已完成的 HTTP
验收，也不证明所有 Canvas 账户或当前 Google 实现都不可用。

需要特别注意：`tryOpenCanvasProxyPreview` / `stampCanvasProxyPreviewFrames` 并不是纯导航；
默认会发送 `Reply with exactly: ok` 的真实 generateContent 请求。不能把这类 helper
当作无上游副作用的页面检查，也不能在恢复现场时无界循环调用。

## 验证与下一步边界

五个既有定向 fixture 文件全部通过，**118/118、0 failed、0 skipped**，耗时 3,441.6266 ms：

- `gemini-canvas-browser-pool.text.test.mjs`
- `gemini-canvas-browser-pool.program-handles.test.mjs`
- `gemini-canvas-browser-pool.proxy-capture-binding.test.mjs`
- `gemini-canvas-browser-pool.connected-client.test.mjs`
- `gemini-canvas-browser-pool.bootstrap.test.mjs`

这些验证合同和分派行为，不覆盖 Google 当前宿主身份注入，不能覆盖真实 403。
主要安全证据为 `browser-buffered.json`、`audit-browser-failed.json`、
`failure-classification-safe.json`、`worker-page-safe.json` 与最终安全汇总。
具体页面脚本及状态不复制到仓库文档。

后续正确修复需要分别闭环：从既有源凭据恢复可用的 Canvas 浏览器状态；materialize 并
确认具体程序与 preview；证明真实宿主代理身份进入该程序的请求；最后再以单次、有 guard
的普通 relay 和审计证明 app-owned 执行。不能仅增加 `credentials: include`、发现网页 Key、
拥有本地 WS client 或返回 Web 聊天答案就宣布修复成功。

本轮尚未证实一个可以直接上线的最小身份修复，因此不盲改执行器、不标蓝、不发布新包。

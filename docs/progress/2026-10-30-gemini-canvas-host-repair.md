# Gemini Canvas 宿主身份通道修复与真实验收（2026-10-30）

## 结论与范围

`gemini-canvas-browser-relay` 的文字调用已修复，普通响应和 SSE 均通过真实 Gateway
`/v1/chat/completions` 验收。两次请求都经过当前凭据独占的已登录 Canvas preview
宿主 fetch；没有使用官方 API Key，没有用普通 Gemini 聊天或其他账号作为 fallback。

本记录不覆盖 `gemini-canvas-program-relay`、旧 Canvas presets、多模态、完整模板 UI
导入或所有账号/模型。Canvas 这条路径仍依赖后台浏览器环境；它不是普通 Gemini Web
HTTP 凭据模式。2026-10-29 的首次失败记录保留，不重放已消费的请求 guard。

## 根因与实证

1. **诊断恢复材料过旧。** 首轮测试加载 `capture-recovery.json`，它是成功 Web 导入之前
   的恢复输入，不是验证后刷新的输出。与最终成功导入的凭据比较，有 17 项 Cookie 值
   不同。在新的私有诊断 state 中使用最终凭据后，真实 Gemini 页面恢复已登录状态，
   无需用户再次登录。没有修改原始恢复文件或扩张 Gemini Web 导入合同。
2. **生成请求在错误的执行环境发出。** 旧 `canvas_proxy` 在主页面注入自有 WS client，
   最终调用 native fetch，未进入 Google 注入身份代理的真实 Canvas preview。WS apiKey
   仅认证本地连接，不能提供 Google consumer identity，因此得到 HTTP 403 / unregistered
   caller。这不等于用户网页登录失败。
3. **旧连接选择未绑定当前账号。** pool-global 的 first authenticated client 可以把当前
   请求交给另一账号。新增的三项路由红测实证旧代码返回其他账号的合成响应；新分支不再
   选择全局 client。其他旧模式的 connected fallback 未在本轮一并重构。
4. **预览 iframe 的实际地址是 blob URL。** 登录后的真实页面使用
   `blob:https://<host>.scf.usercontent.goog/<document>`，其中 fetch 为 Google 注入的非
   native wrapper。只认普通 HTTPS frame 会漏掉正确的执行环境。

第一次显式宿主诊断取得 HTTP 200 和 host RPC 200，但 `maxOutputTokens=64` 的结果为
`MAX_TOKENS` 且无答案；该结果仅证明身份通道恢复，不登记业务通过。后续验收使用
`max_tokens=1024`，保留原请求语义，不通过改写 thinking 等参数掩盖结果。

## 改了哪些内容

- 新增 `gemini-canvas-browser-pool-host-fetch.mjs`，限定 Google Gemini source origin 和
  Generative Language target origin，只选择当前 entry 内可见、未 detached、具有宿主
  注入标记且 fetch 非 native 的 Canvas iframe；支持 HTTPS 和 blob 地址。
- readiness 仅检查状态及点击“继续/预览”，不发送隐式探测生成。访客、缺少宿主、页面
  变化均 fail closed；不自动填写 API Key，不回普通聊天，不切换其他 WS client。
- 在 fetch-execution 的 `canvas_proxy` 生成分支接入新 owner，删除旧的全局 WS 分派。
  既有 GET 下载资产分支、body admission、capture finally 清理和非相关模式保留。
- 为每个已核验文档记录不可枚举、不可重写的随机 document token。在实际 fetch 的
  **同一次 evaluate 内、没有中间 await** 同步复核 origin、href、document token、注入
  标记和 fetch wrapper，关闭同一 Frame/同一 URL 重新导航的 TOCTOU。失配返回固定
  `gemini_canvas_host_identity_changed`，不执行生成。
- 新日志只包含 HTTP 状态和固定 `canvas_preview_host` surface，不记录密码、验证码、
  Cookie、token、请求头或上游原始错误。frame element handle 在 finally 释放，导航中
  dispose 失败不会遮蔽原结果。

Gateway 新分支只调用一次执行函数，不做失败重放。此保证不延伸为 Google 自有 wrapper
内部绝无重试。已有 AbortController/timer 仍管理超时，本轮未声称实现调用方主动取消。

## 验证

- 账号路由保护：先 3 项失败，修复后通过。
- 新 host owner/atomic identity 测试：首次 19 项中 7 项失败；修复后 19/19 通过。
- 两项真实离线 Edge 测试通过：blob iframe 成功且仅发送一次；同一 Frame/URL 重导航后
  拒绝旧 document identity，实际请求计数为 0。
- 受影响 Node 回归共 177 项通过、0 failed、0 skipped，覆盖 host routing、host frame、
  fetch operation/execution、no-key fetch、connected client、capture binding、bootstrap、
  program handles 和 text。
- 四个生产 MJS 文件通过 `node --check`。仓库 scripts 未配置单独的 JS formatter，保留
  邻近风格；没有为本次修复安装格式化依赖或格式化无关源码。
- effective-line checker 测试 31/31；ratchet 和 `git diff --check` 通过。计量：pool
  486→491；fetch-execution 440→372；新 host owner 107；no-key-fetch 175→188。
  新增/修改源码均未超过 500 有效行，未修改基线或豁免规则。

## 真实业务验收

证据根：`linshi/gateway-gemini-canvas-repair-20261030-r1/`。普通和 SSE 各一个独立
create-new guard，各一个 client request，配置关闭自动测试；没有失败重放或付费回归循环。

| 项目 | 普通响应 | SSE |
| --- | --- | --- |
| preset | `gemini-canvas-browser-relay` | 同左 |
| requested/resolved model | `gemini-3-flash-preview` | 同左 |
| provider / credential | `canvas-browser-acceptance` / `canvas-isolated-1793287600` | 同左 |
| HTTP / audit upstream | 200 / 200 | 200 / 200 |
| 非空且精确答案 | 通过，含该次唯一 marker | 通过，含该次唯一 marker |
| finish reason | `stop` | `stop` |
| SSE 终止 | 不适用 | 收到 `[DONE]`，2 个 data events |
| client 耗时 | 14,812 ms | 11,125 ms |
| routeAttemptCount | 1 | 1 |
| audit ID | `gw-audit-c5a76d8e-7d6d-47d8-8b43-9a85718a52b2` | `gw-audit-68697fa4-2b9a-4a33-b3ce-3738cd76333a` |
| audit 终态 | `completed`，无 errorCode | `completed`，无 errorCode |

worker 安全日志包含恰好两条成功的 `canvas host fetch completed`，均为 status 200 /
`executionSurface=canvas_preview_host`。它与宿主诊断、真实非空正确答案及单一账号审计
共同构成通过证据，不单独以 wrapper 外形、池健康或 HTTP 200 判业务成功。

安全收据：`buffered-production.json`、`stream-production.json`、`audit-2.json`、
`runtime-new.json`、`setup-new-runtime.json`。原始状态和所有 secret 留在 `private/`，
不提交、不打包。

## 运行与交付边界

原隔离后端 4310 的凭据对象引用已用管理 API 更新到新私有 state 对象，保留旧对象和
revision；没有在该实例重放生成。停止旧 worker 的工具调用被策略拒绝后，没有换方式
停止它。保留原实例，另起后端 4311 与 worker 43131/43132 加载当前修复源码验收。

后端复用已冻结 API devtest r3，SHA256
`cf58d556cd8b4ca421002657c76f5160939eb2049bbc34bd2760df6f112a2751`，source fingerprint
`5a1a4e9172b73956aea80d04596434791a40723f479597ecc674530d3b946e41`；JS worker 使用本轮
工作树源码。没有声称旧 EXE 内嵌了新 worker，也没有新建 EXE、手测包或正式发布。

日常 4200、此前 Web 4300 未重启、未修改数据或凭据。专用 CLI 诊断浏览器已正常关闭，
没有操作用户日常浏览器。新增测试服务保留，worker context 由既有 idle eviction 管理。
本轮 Canvas 修复未提交、未推送；此前用户要求先推送的 Web 提交已完成，不混为同一步。

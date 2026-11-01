# AI Studio 普通浏览器认证与 HTTP 验收停点（r34）

## 范围与结果

目标是 `https://aistudio.google.com/prompts/new_chat`，不是旧 Build app，也不是
`generativelanguage.googleapis.com` 的用户 API Key 调用。本轮未宣布服务商实现完成，
未编译 EXE、提交或推送，未改动正在运行的 Gateway 实例或既有账号。

- **passed**：用户手动登录成功；必要 Cookie 已保存到隔离验收实例的正常凭据对象根。
- **passed**：只导入 scoped Cookie records；未调用 `storageState()`，未导出 localStorage、
  IndexedDB 或完整浏览器 profile。真实 Cookie 和认证头未进入日志或本报告。
- **passed**：导入子进程 exit 0；本次专用浏览器已关闭，临时 profile 已清理。
- **passed**：无浏览器、无 JS 引擎的 HTTP bootstrap 与 `ListModels` 均返回 200，
  解析到 37 个模型。目录成功不是生成成功或完整服务商验收。
- **failed**：一次不带 WAA proof 的 `GenerateContent` 请求返回 403，没有真实答案。
- **unverified**：403 的现有安全分类没有给出明确 WAA 原因；不能仅凭 403 宣称引擎必需，
  也不能宣称账号或平台不支持。fresh-proof 路线与生产接入仍待推进。

## 登录问题与改动

用户在 Playwright 直接启动的 Edge 和 Chrome 上均看到 Google 的
“此浏览器或应用可能不安全”。旧 Gemini Web 导入器也使用可见浏览器、人工登录、
随后捕获验证请求，但带有本次没有复用的隐藏自动化标记、忽略证书及 bypassCSP 参数。
Google 风险判定的内部原因未知；不能把这些参数当作经证明的唯一修复原因。

本次新增 `scripts/aistudio-manual-login-browser.mjs`，把正常登录与程序导入分成两阶段：

1. `spawn` 已安装的 Chrome，使用本次独占临时 profile；没有自动化驱动连接、
   `--enable-automation`、远程调试参数或安全检查关闭参数。
2. 用户正常登录 AI Studio，成功进入界面后关闭本次专用窗口。
3. 用同一临时 profile 短暂重开浏览器；仅在此阶段使用非零 loopback CDP 端口。
   核对 CDP browser PID 与本次子进程 PID 一致，才读取必要 Cookie。
4. Cookie 由 `scripts/import-aistudio-http-cookies.mjs` 写入随机、不可覆盖的凭据对象键。
   关闭自有浏览器，仅清理 `mkdtemp` 独占子目录，不接触日常浏览器资料或既有凭据。

用户确认“已登录并关闭”后，程序完成实际导入。该办法解决了本次登录阻塞；
不是对所有账号、设备或 Google 风险状态的永久成功保证。

## 无浏览器 HTTP 证据

临时原型和安全 receipts 位于：

`C:\Users\Public\nas_home\AI\GameEditor\linshi\gateway-aistudio-http-20261031-r34`

- `native-login-launch-safe.json`：实际 Chrome 进程没有上述自动化/调试/安全绕过参数。
- `native-login-complete-safe.json`：专用浏览器停止、临时 profile 数量为 0、凭据对象存在。
- `direct-http-canary-r1.safe.json`：bootstrap/catalog 成功；旧默认模型不在当前目录，
  未发送生成请求。
- `direct-http-canary-r2.safe.json`：只读模型目录，未发送生成请求。
- `direct-http-canary-r3.safe.json`：选择目录实际存在的 `models/gemini-3.5-flash-lite`，
  一次生成请求，HTTP 403。未启浏览器、未用 JS 引擎、未自动 retry 或 redirect。

原型按目标 domain/path/expiry/Secure 选择 Cookie，以三段 SAPISID 签名认证，
从网页 bootstrap 发现公共项目 key；没有使用用户官方 Gemini API Key、旧 Build
CodeAssistant、静态 proof、fixture 答案或 browser fallback。
生成请求有独占一次性 guard；未保存原始错误正文、认证头或 bootstrap 正文。
后续不能为读取已丢弃的错误正文而无依据地重放该生成请求。

## 验证与未完成项

- 导入／启动参数／临时目录边界定向测试 6/6 passed；Node syntax checks passed。
- HTTP 原型 Cookie scope、sparse protobuf JSON、完成帧合同测试 3/3 passed。
- effective-line checker tests 31/31 passed；ratchet passed；`git diff --check` passed。
- 本轮四个源码/测试文件分别为 110、91、28、29 effective lines，均未超限。
- 尚未把 AI Studio auth family 接入管理 UI；保存了验收用凭据对象不等于新增服务商账号完成。
- 尚未替换旧 Playground/Build 配置、旧 CodeAssistant 执行或生产 browser fallback。
- 尚无成功的 AI Studio HTTP `GenerateContent` 证据；JS 引擎、协议接入与其他模态仍未交付。

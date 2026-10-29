# ChatGPT Web browser relay 认证与两阶段协议接线

## 发现与修复

前一包已更新 Rust 普通 conversation 的 prepare/finalize 握手，但浏览器后备链仍有两个独立遗漏：

1. `chatgpt-web-session-worker.mjs` 已取得 `effectiveAuthToken`，调用
   `runChatGptBrowserRelay` 时却没有传入 `accessToken`；后者因此无法生成 Bearer header。
   新增 worker 回归已先复现 `fixture_missing_relay_bearer`，补上传参后通过。
2. `http-relay.mjs` 仍请求旧单阶段 requirements。新的 `http-requirements.mjs`
   让普通 browser relay 默认使用 prepare/finalize，生成只使用 finalize 返回的 token。
   Rust worker 消息及 JS 调用都传递显式 requirements mode，保留 `legacy` 配置。

requirements HTTP 失败、畸形 token、过大/非 JSON 响应立即停止，不发 conversation，
不降级或转向 UI 再发同一轮。会话/浏览器验证相关的 401/403/429 也不自动转 UI 提交。
requirements 响应上限 1 MiB、token 上限 32 KiB；错误不包含上游正文或 token preview。
沿用已有有界 proof handler，只增加输入尺寸检查；未引入新的挑战求解、反检测或换出口能力。

这部分修复尚未包含在上一包 `gateway-devtest-20261029-041427-web-two-stage` 中，
不可将旧包的源码 fingerprint 当作这些新修改的交付证据。

## 验证

证据目录：`linshi/gateway-chatgpt-browser-session-20261029-continuation/`。

- `relay-bearer-red-r2.log`：修复前明确缺少 relay Bearer 参数；首次夹具遗漏
  `randomUUID` 的失败保留在 red 日志，不作为产品故障证据。
- `browser-requirements-red.log`：修复前实际仍发旧 requirements，再发 conversation。
- `browser-requirements-final.log`：相关 Node 定向测试 59 passed / 0 failed，覆盖默认两阶段、显式旧模式、
  token 来源、HTTP 拒绝停止、输入边界、worker token/mode 接力及原清理合同。
- `browser-loopback-result.json`：真实 Chrome 的本地页面执行该 relay，通过三阶段、
  Bearer、Set-Cookie 接力和最终 token 来源检查；真实 provider 请求为零，浏览器和 fixture 已关闭。
- checker tests、ratchet、scoped rustfmt、nested worker package contract、diff check 已通过。
- Rust `chatgpt` 定向回归：222 passed / 0 failed / 3687 filtered out，编译 12 分 10 秒，
  测试 152.47 秒；这是本次测量，不是后续构建的性能承诺。

## 真实外部状态与剩余工作

正常启动的独立 Chrome 使用该目标账号已保存 Cookie 做只读探测，首页加载后会话接口仍为
HTTP 403，且 `cf-mitigated: challenge`。这比 Python 客户端探测更接近浏览器真实环境，
但不证明账号密码错误、token 已过期或目标模型不可用。没有发新生成请求或自动处理验证。
临时 profile 已清理，现有 Gateway 未关闭或重启。

另一次明确使用保存 Bearer 的独立只读目录探测也返回 HTTP 403，未发生成请求。
其诊断 Chrome 稍后正常退出，但超过脚本最初 5 秒观察窗口；后续只读确认无该诊断 Chrome
进程。遗留的一个临时 profile 未删除：清理命令被工具策略拒绝后未绕道重试。
该目录含私有浏览器状态，不得外发或打包，见 `native-probe-cleanup.json`。

为定位“人工登录后的原始 profile”与“仅导出 Cookie 后重建的 profile”之间的差异，已准备
独立保留现场诊断入口，并请求用户配合一次正常登录；在用户答复前不自动打开登录窗口。
目录、真实生成和新包原生 UI 验收仍未完成。总体目标保持未完成，不能把局部回归当作可用调用。

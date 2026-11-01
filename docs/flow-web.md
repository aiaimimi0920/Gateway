# Google Flow Web：严格 HTTP-only 试接入

`flow-web` 对应 <https://flow.google.com/>，不是音乐服务 `flowmusic-web`，
也不是 Gemini Canvas。当前仅完成模板、Cookie 导入与只读 HTTP 控制面；
**没有完成图片／视频生成，不应将该模板标蓝或称为生成可用。**

## 当前开发状态：未完成，已暂停

2026-11-01 用户明确暂停 Flow 生成路线开发。保留独立 `flow-web` 标准模板、创建服务商和
专用账号导入入口；目录保持未验收颜色，悬停与可访问状态为“未完成（已暂停）”。
暂停不禁用配置与正常登录／JSON 导入，但不会自动导入账号、启动浏览器、获取 proof 或尝试生成。
图片／视频仍没有可交付闭环，重新推进生成开发需用户后续明确恢复。

## 已实现的边界

- 独立 preset `flow-web`、adapter `flow_web_compatible`、special adapter `flow_web`。
- 添加账号复用 Google Web 专用控件，支持普通网页登录和受敏感访问授权保护的 JSON 导入。
- 登录期间不连接自动化或远程调试。用户关闭本次专用窗口后，短暂重开该专用 profile，
  仅取适用于 Flow 的 Cookie，经私人子进程管道交给凭据 owner；不导出完整浏览器状态。
- 网页登录导入结束后关闭所拥有的浏览器，再用 Rust/rquest 执行只读额度 RPC；
  仅在只读请求通过后保存凭据草稿。此结果不代表图片、视频或具体模型的生成成功。
- 离线 JSON 只校验材料并返回草稿，不联网、不证明账号有效。保存走既有 revision/CAS 流程。
- `GET /projects` 获取当前 bootstrap，再调用
  `POST /_/AiSandboxAngularFrontend/data/batchexecute` 的 `nzlxg` 只读额度 RPC。
  限定官方 HTTPS origin，禁止重定向，Cookie 保留域／路径／到期语义；
  整条只读交换限时 45 秒，页面限 8 MiB、RPC 响应限 1 MiB。
- 手动连通测试可以执行上述只读请求，但生成测试结果仍返回 `Unsupported`，不显示生成验收通过。
- 模板和账号均不注入未经真实验收的模型目录，不自动建立生成路由，不启动定时账号探测。

## 生成阻塞与失败合同

2026-11-01 联网检查的最新参考实现中，当前 Flow 原生前端使用 batchexecute，
图片生成 `ogiZ0b`、文生视频 `YhhmEf`，每次提交的 project context 携带 fresh reCAPTCHA proof。
参考一的 proof 获取仍依赖浏览器或第三方服务。参考二也优先启动 Chrome/CDP；
其 HTTP fallback 是否能被当前生成端接受，没有真实验收证据。

用户明确选择维持严格 HTTP-only。因此 Gateway 不增加浏览器取 proof、打码服务、
浏览器回退，也不将登录 Cookie 当作长期可复用的 fresh proof。

当前图片／视频调用一旦进入该执行器，会在任何网络、凭据读取或进程启动前返回：

```json
{"error":{"code":"flow_web_fresh_proof_unavailable"}}
```

该错误为 HTTP 503、`retryable=false`、`FallbackHint::Abort`。不提交生成，
不重试、不切换浏览器，不返回占位图片／视频，也不将拒绝生成解释为平台没有相应能力。
当前没有实现生成 wire encoder、上传、视频轮询或成品回收；这些不是已通过的能力。

后续解锁条件是：在不启动浏览器的条件下，通过当前 Google Flow 真实生成端验证
fresh proof 获取与一次真实成品回收。只有完成该条件后，才接通生成执行与模型路由并考虑标蓝。
本次并没有证明所有可能的 HTTP-only 路线都不可行。

2026-11-01 后续研究继续核对了 HTTP 兜底及另外两个项目的运行边界，仍未取得符合约束的
生成闭环；没有移植或运行 reCAPTCHA 绕过逻辑，也没有将官方 API 或外部浏览器伪装成 Flow
HTTP-only 成功。详见 [HTTP-only 路线研究](progress/2026-11-01-flow-web-http-only-research.md)。

同日后续进行了真实 HTTP 实验，并修复了导入链误过滤普通 Cookie 的问题。
浏览器关闭后，额度查询和项目创建均通过；一次图片提交收到 Google 的
`PUBLIC_ERROR_UNUSUAL_ACTIVITY`，之后停止进一步生成，视频未提交。
这是独立 HTTP canary 的拒绝证据，不是公共生成 API 已通过，也没有证明所有 HTTP-only 路线不可能。
详见 [真实 HTTP 实验](progress/2026-11-01-flow-web-http-live-canary.md)。

## Cookie JSON 格式

仅支持账号索引 `authUser="0"`，避免未经验证的多账号切换误用身份。
以下全为示例占位值，不能用于真实登录：

```json
{
  "schemaVersion": 1,
  "provider": "flow-web",
  "authUser": "0",
  "cookies": [
    {
      "name": "__Secure-1PSID",
      "value": "EXAMPLE_ONLY_NOT_A_REAL_COOKIE",
      "domain": ".google.com",
      "path": "/",
      "expires": -1,
      "secure": true,
      "httpOnly": true,
      "sameSite": "Lax"
    }
  ]
}
```

只接受 `.google.com`、`flow.google.com`、`.flow.google.com` 的 Cookie，
且必须有能到达 Flow 根路径的当前 `__Secure-1PSID`。保留各 Cookie 的原始 `secure` 标记，
普通非 Secure Cookie 也只能发送到已限定的官方 HTTPS origin；`__Secure-`／`__Host-` 前缀仍要求 Secure。
不接受完整 storageState、localStorage、浏览器可执行路径、任意 endpoint 或 reCAPTCHA token 字段。
材料存入既有 `credential-runtime/flow-web/.../http-cookies.json` 私有对象；
管理响应只返回对象引用，不返回 Cookie secret edits。

## 参考来源

只研究协议和依赖边界，没有引入参考项目的应用框架或执行第三方代码。

- [TheSmallHanCat/flow2api](https://github.com/TheSmallHanCat/flow2api)，
  固定 commit `b410e373d4e90772a31ce8d33b5a5e3172c6ca28`，MIT；当前 Flow 前端 RPC 路线。
- [HL320217/flow2api](https://github.com/HL320217/flow2api)，
  固定 commit `95d9cabea3dc539478bbae6c101691a4344adab4`；未确认许可证，不复制源码。
  其 Labs/aisandbox 路线不能当作当前 flow.google.com 的真实验收证据。

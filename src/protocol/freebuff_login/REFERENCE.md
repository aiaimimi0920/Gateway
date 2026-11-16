# FreeBuff CLI approval protocol reference

本模块依据官方公开源码的 wire 行为独立实现，不复制 TypeScript 源码，不引入或执行外部项目，不采用其 analytics、硬件采集、管理面或默认重试策略。

固定来源：[CodebuffAI/freebuff @ 35c6bb288d2af2efc85616c572774fd8ab47e3db](https://github.com/CodebuffAI/freebuff/tree/35c6bb288d2af2efc85616c572774fd8ab47e3db)。仓库 Apache-2.0；2026-11-06（Asia/Shanghai）重新联网取得根 LICENSE 和下列文件，完整 bytes/hash/HTTP status 保存在 `C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-login-20261106/public/receipt.json`。这是公开源码的版本核验，不是真实平台登录或客户端使用许可证明。

## 采用的协议事实

| 固定文件 | 事实与采用方式 |
| --- | --- |
| `common/src/constants/hosts.ts`、`cli/src/login/constants.ts` | 所选 FreeBuff 产品生产 API origin 为 `https://freebuff.com`；不采用环境变量任意改目标或 Codebuff 产品回退 |
| `cli/src/utils/codebuff-api.ts` | POST `/api/auth/cli/code`，JSON 只有 `fingerprintId`；GET `/api/auth/cli/status`，query 为 fingerprintId/fingerprintHash/expiresAt；两者不带已有用户认证 |
| `common/src/constants/auth.ts` | `expiresAt` 是服务端 HMAC 输入，必须原样 echo；`expiresInMs` 为相对时长；官方相对等待常量为 60 分钟 |
| `cli/src/login/login-flow.ts` | 第一次立即查询；pending/无 user/401 后等待默认 5 秒；取得 user 才有可保存的材料 |
| `common/src/util/credentials.ts` | user 中有 id/email/name/authToken/fingerprintId/fingerprintHash；不是 OAuth access/refresh grant |
| `cli/src/utils/fingerprint.ts` | 官方设备标识有自己的采集与 fallback 策略；本模块不复制采集、不伪装硬件、不擅自为每次请求换 ID，调用方必须显式提供并保留目标设备标识 |

API tree 未包含 `/api/auth/cli/code` 或 `/status` 的服务端 route，不能宣称已读到其 HMAC 实现或完整服务端错误合同。源码 response interface 尚未声明 `expiresInMs`；本地有相对 hint 时使用它，缺省采用已证 CLI 的一小时等待上限，不把绝对字符串按本机时钟解析。等待上限从 POST 开始计算，不延长网络已耗时间。

## 本地准入与资源合同，不包装为上游保证

- 浏览器链接仅接受 HTTPS、精确 `freebuff.com`、默认 TLS 端口、无 userinfo/fragment。公开参考直接返回 loginUrl，没有实现这个 allowlist，也未证明合法路径/跨域跳转合同；真实录制若显示另一个官方 origin，须先核实再精确修订，不能直接放开任意 HTTPS。
- receipt 的两个 fingerprint 必须与本 attempt 完全相等。这是 Gateway 的绑定检查，不是官方源码已实现的验证或“上游永不规范化 ID”的保证。id/email 只是 endpoint 返回的 hint，不授予管理权限、不跨账号/产品合池、不证明额度或账号所有权。
- 新 transport 独占无 cookie 的 HTTP client，并显式 `.retry(rquest::retry::Policy::never())`；不采用官方 API client 的 3 次自动 retry。当前 wreq 0.16.1 默认还会重试低层安全 NACK，不能把“没有写重试循环”当作已禁重试的证据。
- 构造复用 `crate::http_client::builder()` 的代理来源规则，不继承已有业务 client 或秘密默认头；仅合成测试明确 `.no_proxy()`。有效 HTTP 2xx 仍须通过 JSON 和材料校验，204/空 body 或畸形 user 不产生材料，不能把“非 200 成功”放宽成只看状态码。
- 单步 send/read 最多 15 秒、解码后最多 64 KiB；状态等待和请求共用原 challenge deadline。禁止重定向、任意 endpoint 和认证头继承；错误只导出固定本地码，不包含 query/HMAC/token/上游自由文本。
- 只提供一次 GET 的可取消 future，不为每个账号另起轮询 task。401 是 Pending，不据错误文字标记业务 token 失效；其他 HTTP 错误固定拒绝，不在此模块偷偷重试。调用方若明确继续同一 GET capability，仍受原 deadline 和前一步完成/取消后的 5 秒间隔限制。
- `poll` 独占借用 challenge；在途取消由 owner drop/select 取消该 future，再调用 `cancel` 关闭 capability。`cancel(&mut self)` 本身不是跨任务的主动中断 API。
- 创建和状态均有且只有一个应用层 HTTP send；一次可用 receipt 之后 capability 被消费，不再领取第二份材料。没有推断 token 的到期时间、refresh endpoint 或业务 agentId；agent 不是登录 receipt 字段。
- 本模块没有管理 HTTP/session registry、Secret Grant/provider/revision 实查、SecretPatch/CAS 保存、UI 或推理接线。不得以协议测试通过报告完整 FreeBuff 登录完成；后续必须由正常 owner 接管，不把 authToken/fingerprintHash 塞到公开 document。

Credentials 和 challenge 不实现 Serialize/Clone，Debug 固定脱敏。源代码、测试和本说明使用 UTF-8 无 BOM；所有 HTTP 测试仅访问随机 loopback 合成 peer。

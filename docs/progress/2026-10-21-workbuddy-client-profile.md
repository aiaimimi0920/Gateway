# WorkBuddy 国际版调用风格

## 需求边界

调用风格与服务协议、认证并行：前者描述客户端软件身份，后者负责可调用性。
新增显式 `request_client_profile: workbuddy`，不改变 auto、Codex CLI、Claude CLI
的默认选择，不修改 DSH，也不生成缺失的会话 ID。不接入 WorkBuddy 官方账号后端。

## 官方基线来源

本轮联网访问 https://www.workbuddy.ai/，从官网脚本读取下载接口：
`https://www.workbuddy.ai/v2/update?platform=workbuddy-win32-x64-user`。
接口返回国际版 5.8.4.41366630，下载地址为：
`https://download.codebuddy.ai/workbuddy/saas/win32-x64-user/WorkBuddy-win32-x64-user-5.8.4.41366630-73952edb.exe`。
Windows Authenticode 校验为 Valid，签名者 Tencent Technology (Shenzhen) Company Limited。

官方安装包释放在 linshi/gateway-workbuddy-profile-20261021/app；未覆盖用户安装或配置。
首次使用 7-Zip 22.01 无法解压附带 ARM64 组件，已用本机 7-Zip 23.01 完整重解压成功。
应用 package.json 为 5.8.4，捆绑 CLI customPackage 版本为 2.161.0。

通过安装包原始 WorkBuddyAI.exe（Electron Node 模式）运行其原始 lite-wb 模型引擎，
按照桌面 sidecar-manager 的启动代码设置产品身份环境；不修改第三方 bundle。
使用隔离 HOME、合成 API key、回环接收端，不登录账号、不发送付费模型请求。
这验证的是桌面捆绑模型引擎，不冒充已经完成桌面 UI 端到端操作。

## 实测与字段归属

实际引擎请求 POST /chat/completions。第一次接收端主动返回 400 用于收集字段；
随后接收端返回合法 SSE，两次独立调用均成功；引擎输出 result=OK、subtype=success、
is_error=false。两份抓包的软件身份字段相同，变化限于会话/请求/追踪标识及回环端口。

| 字段 | 实测软件值 |
| --- | --- |
| User-Agent | WorkBuddy/5.8.4 workbuddy-ai/5.8.4 CLI/2.161.0 |
| X-IDE-Type / X-IDE-Name | WorkBuddy |
| X-IDE-Version | 5.8.4 |
| X-Product | SaaS |
| x-codebuddy-request | 1 |
| X-Requested-With | XMLHttpRequest |
| x-stainless-package-version | 6.25.0 |
| x-stainless-runtime / runtime-version | node / v22.21.1 |
| x-stainless-lang / os / arch | js / Windows / x64 |
| x-stainless-retry-count | 0 |

SDK 字段描述此次实测的 WorkBuddy 模型引擎软件栈，不决定 Gateway 的目标 API 协议。
OS/arch 按 Gateway 平台生成；非 Windows 平台未做 WorkBuddy 实机基线验证。

不注入的字段：X-Conversation-ID、消息/请求 ID、traceparent/B3、X-User-Id、
X-Agent-Intent/Purpose/Type、X-Private-Data。这些分别属于会话、追踪、认证或业务策略。
不复制样本中同时存在的 Authorization 与 X-API-Key；服务商 adapter 继续拥有认证。
Accept 的已有显式值保留；没有值时使用 application/json。
Accept-Encoding/Connection/Host/Content-Length 仍由实际传输能力管理。

## 实现与验收入口

- 独立 workbuddy 身份模块；配置枚举、服务商继承与账号覆盖复用现有机制。
- 控制台增加 WorkBuddy 选项；切换到其他套件会清理 WorkBuddy 软件标识残留。
- 显式 WorkBuddy 不进入 Claude 原生传输分支，也不追加 Codex Responses 缺省参数。
- custom_http 等自定义认证头与软件字段重名时，认证优先。
- 三个新增合同测试先运行并因 unknown variant workbuddy 失败，之后实现。
- 实际出站回环测试发现共享 Chrome 客户端补入 sec-ch-ua；失败证据 runtime.log 保留。
  在通用请求 builder 中仅对完整匹配的 WorkBuddy 软件标识关闭 client default headers，
  避免 Chrome 默认字段污染。保留明确设置的头，不更换 TLS 配置、代理、认证或目标地址。

## 静态及前端检查

控制台四个定向测试通过，TypeScript typecheck 通过；首次 UI 测试启动目录不正确导致
setup.ts 无法定位，改用 apps/desktop 工作目录后通过，失败日志未删除。
有效行 checker 31 通过、ratchet 通过、cargo fmt --check 与 git diff --check 通过。
本轮生产修改均低于 250 有效行：新增身份模块 34 行，共享请求 builder 146→148，
headers 167→175，body 35→38，配置枚举 48→49；没有扩大历史超限文件。

## 最终定向 Rust 验证

`final-rust.log` 的最终编译与测试成功，包含：

- WorkBuddy 合同 5/5：跨模型/协议身份、认证隔离、无 Codex body 缺省值、
  配置继承/账号覆盖、风格切换清理与显式策略保留。
- 现有 request_client_profiles 合同 8/8、Claude wire 合同 6/6、Codex fidelity 合同 4/4。
- 实际 HTTP 回环矩阵 1/1：5 风格 × 3 模型类别 × 3 接口 × 2 流式状态，共 90 次发送；
  WorkBuddy 18 次均无 Chrome sec-ch-ua/移动端/平台、sec-fetch-dest/user 默认头污染。

本轮没有运行全仓所有测试、正式打包或真实付费服务商三路对照；上述回环结果不能扩大为
服务商已接受全部 WorkBuddy 特征，亦不表示 TLS/完整网络指纹等同。

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-workbuddy-profile-20261021`。
安装包、第三方源码、抓包正文和运行数据仅保存在该隔离目录，不加入仓库。
本轮不宣称 TLS 指纹、头顺序、辅助接口及完整会话行为与 WorkBuddy 完全相同。
未正式部署、提交或推送。

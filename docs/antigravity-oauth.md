# Antigravity OAuth

本实现借鉴公开协议行为，使用 Gateway 自有凭据存储、路由、协议转换和 HTTP 传输；
不运行参考项目的服务，不导入其可执行文件，不修改 Antigravity 客户端或设备标识。

## 三个职责

1. **凭据导入**：控制台的 `antigravity-oauth` 模板及专用登录按钮。
   使用 Google 官方 OAuth、随机 state、PKCE S256、offline refresh token。
   登录由用户在普通浏览器中手动完成；回调只绑定 loopback，
   远程控制台可粘贴本次授权的完整回调 URL。
2. **调用适配**：独立的 `antigravity_oauth_compatible` executor。
   上游是 Google Cloud Code `v1internal`，不是普通 Gemini API。
   Canonical 请求转换为 Gemini 内层请求，再加入可信的 project/model/requestId/sessionId；
   JSON 与 SSE 响应先解开 `response` 外层，再进入现有 Canonical 解析。
3. **指纹适配**：独立的 `fingerprint_adapter=antigravity`，只能搭配该 executor。
   最终发送前禁用共享客户端默认 HTTP 头，应用实测 Antigravity User-Agent、Bearer 和 JSON 头；
   不添加原生没有的通用 Accept。调用使用独立 native TLS profile、客户端和连接池，
   不经过 Chrome TLS，也不借用 CPA 的业务身份、单例或 CA 配置。
   本机 Windows hub 2.15.0 的首个 ClientHello 结构已实测对齐；
   不宣称随机密钥字节、全部握手行为和应用层内容完全相同，详见
   [原生指纹对照与边界](antigravity-fingerprint-parity.md)。

## 使用与生命周期

先通过模板保存一个服务商，再在账号对话框点击“通过 Antigravity 登录”。
授权回调成功后点击“保存到凭证池”。程序向 Google userinfo 获取可信账号身份，
调用 `loadCodeAssist` 读取真实项目，再通过 `fetchAvailableModels` 导入实际模型目录和路由。
这两个操作不生成内容，也不代表目录中的每个模型都已经具备调用额度。

没有项目时明确返回 `antigravity_project_missing`，请先在产品中正常启用账号。
不会伪造 `aicode-consumers` 或自动代用户接受新条款。
模板初始模型目录为空，尚无真实生成验收时不标蓝。

账号按 Google userinfo 的稳定 ID 去重；重复授权保留既有凭据 ID、禁用状态及其他策略。
access/refresh token 进入既有 native credential secret owner，不导出完整浏览器状态。
导入只修改目标服务商，既有模型路由的禁用状态、优先级及其他服务商保持不变。
保存失败时在会话十分钟有效期内保留内存材料，允许重试保存而不重新登录；
会话取消、超时或成功保存后释放相应临时材料。
已开始的提交跨过授权 TTL 时仍可查询；成功后仅保留五分钟不含 token 的回执，
用于恢复保存响应丢失的结果，不能再凭该回执换取或重新导入凭据。

保活复用 Gateway 通用 token refresh scheduler：启动恢复绝对过期时间，每分钟检查，
提前五分钟刷新，single-flight、generation 防覆盖、退避和轮换 refresh token 持久化。
没有新增每账号浏览器、独立刷新循环或数据库版本迁移。
仅 Antigravity adapter 的刷新解析专用客户端参数（默认内置或显式覆盖），且只对精确的 Google token endpoint
与账号保存的同一 client ID 添加 Client Secret；普通 OAuth 刷新不加载或注入它。
配置缺失、无效或与账号 client ID 不匹配时，在发送 refresh token 前停止，返回可恢复的
配置错误，不把账号标记为永久失效。更换 OAuth client 不能自动迁移旧 refresh token。

## 内置 OAuth 客户端与可选覆盖

默认内置参考实现共同使用的公开 desktop installed-client 参数。它们标识应用，
不是用户的 access token、refresh token、密码或个人 API Key。普通使用只需导入账号，
不需要准备客户端 JSON 文件；换电脑或全新数据目录同样可以直接开始授权。

需要特殊部署时，可以显式覆盖。优先级如下（只在授权与刷新时读取，不在模型生成时读取）：

1. 完整的 `GATEWAY_ANTIGRAVITY_OAUTH_CLIENT_ID` / `GATEWAY_ANTIGRAVITY_OAUTH_CLIENT_SECRET` 环境变量对。
2. `GATEWAY_ANTIGRAVITY_OAUTH_CLIENT_FILE` 指定的绝对 JSON 文件路径。
3. 没有任何覆盖时使用内置公开参数，不访问数据目录中的客户端配置文件。

可选 JSON 仅接受 `client_id`、`client_secret` 两个非空字符串；不要把用户 access/refresh
token 填入这里。文件上限 8 KiB，读取有
三秒 deadline，错误不包含文件内容、路径或 secret。仅配置一半或配置为空会明确失败，
不会静默回退。配置修正后下次授权或刷新即可重新读取，不缓存失败，也不要求重启。

授权创建时固定一份客户端快照，授权 URL、code exchange 和保存的 refresh client ID
使用同一份配置。已导入账号的格式、ID、token 和 Key 不因本次外置而重写。
显式覆盖出错不会静默回退，避免把其他 client 签发的 refresh token 发给默认 client。
没有覆盖不是错误，不影响启动、账号授权或刷新。私有覆盖文件不打入发布包；
不要提交用户凭据或通过 `include_*` / `env!` 把私有覆盖编译进二进制。

Google 将桌面应用视为不能保守秘密的 public clients；这个场景中的 client_secret
不等价于用户账号密钥。仍使用 PKCE、state 校验和固定 HTTPS token endpoint。
GitHub 对这两个已知公开参数的误报应按具体检测放行，不关闭其他秘密保护。
Gitleaks 仅在这两条常量上作行级公开参数例外，不对整文件、账号 token 或所有 Google 参数放行。
参见 [Google OAuth installed applications](https://developers.google.com/identity/protocols/oauth2#installed)。

## 当前调用边界

- 生成时为原生 HTTP，不启动、保持或连接浏览器，不需要 Cookie、CDP 或 Node worker。
- 文本 Chat Completions、Completions、Messages、Responses 经 Canonical 桥接。
- 工具结构及显式工具选择走现有 Gemini 格式转换；默认 agent 工具模式使用 VALIDATED。
  多轮工具、Claude thinking/signature 等模型特有行为仍需分别真实验收，不作全模型兼容承诺。
- SSE 上游采用有界聚合后输出 Canonical SSE，**不是实时逐 token 透传**。
  缺少 terminal candidate 的 SSE 按失败处理，不能以半截输出伪装完成。
- 与本次原生采样一致，生成统一走 `streamGenerateContent?alt=sse`，
  HTTP/1.1 请求体为单个 buffered JSON chunk。下游非流式请求仍返回非流式结果。
- 不接管图片、音乐、视频等独立生成端点；不支持服务器存储的 Responses ID 续接。
- 请求上限 16 MiB、生成响应上限 32 MiB、目录上限 2 MiB；所有网络操作有 deadline。
- 不跟随凭据请求重定向；生成失败不自动跨端点、跨账号或重放，以免重复计费。
- 普通账号检查仅读项目和目录，不把它当成模型生成成功。显式提示测试才提交模型请求。

## 参考来源

2026-11-01 联网只读核对：

- [Antigravity2Api](https://github.com/anshulrules/Antigravity2Api/tree/b373165050b32ce4b390bf3a952d73ececc979d4)
- [agy2api](https://github.com/su-kaka/agy2api/tree/6cc45620f1ecbe12bda7792b2daf8940d8a4ffde)
- [AntigravityManager](https://github.com/Draculabo/AntigravityManager/tree/8b4b5eafb8f59c844ec985595bdec316fa0e9466)

默认 installed-client 参数来自上述公开参考，允许操作者显式覆盖，并非用户的 access/refresh token。
当前 Windows 指纹以本机原版 language server 的实际出站为依据：hub 2.15.0、CL 983516863，
不再使用参考项目的 2.0.3 fallback；不会在每次请求中查询、启动或下载客户端。
参考来源只说明实现依据，不代替 Gateway 的真实 OAuth、刷新或模型生成验收。

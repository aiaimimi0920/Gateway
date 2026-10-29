# ChatGPT OAuth 登录

Accounts 中的 ChatGPT OAuth 登录由 Gateway 保管短期 PKCE 会话。创建会话和
保存凭证仍需要管理会话及 Secret Grant；打开链接不代表已完成授权或保存。

## 本机与远程使用

1. 选择已保存的 ChatGPT pool 和订阅分组，点击“通过 ChatGPT 登录”。
2. 点击“打开授权页面”。Web 在当前设备的新标签页打开；新版桌面在当前设备
   的系统浏览器打开，不要求远程 Gateway 启动服务器上的浏览器。
3. 本机 Gateway 可自动接收回调。连接远程服务时，如果浏览器回调地址无法访问，
   复制地址栏中的完整 `http://localhost:1455/auth/callback?...` 地址，回到面板
   展开“未自动返回？粘贴回调地址”，点击“完成授权”。不要公开或记录该地址。
4. 面板显示可保存后，再点击“保存到凭证池”。网络响应丢失时，现有状态轮询
   仍会恢复已提交的保存结果；不会因为打开浏览器而自动导入凭证。

会话到期后重新创建，不重用旧回调。浏览器无法打开时，可使用“复制授权链接”
手动打开。服务器侧的旧 `open-browser` action 保留兼容，仍仅允许本机请求。

## 导入现有令牌

已保存的标准 `chatgpt` 服务商在“新增账号”中可将“录入方式”改为“现有 Access Token”。
选择账号实际的订阅分组，填写网关内唯一的账号 ID、上游 `ChatGPT Account ID`，并在 API Key 栏粘贴对应的 access token。
保存前会使用该 Access Token 和 Account ID 读取官方 Codex 模型目录，读取失败则保留弹窗且不保存。
目录客户端版本与原生 Codex fallback 共用 `codex_client::VERSION`，当前为 `0.155.1`。
普通目录模型自动加入；上游标记隐藏或 `supported_in_api=false` 的条目不自动加入普通模型列表。
也可保留手工声明的模型；目录未返回某个声明模型，不等于已经证明该模型不可调用。
分组写入 `credential_identity_category_id`；声明的模型加法式注册到 provider 和模型路由，
保留其他服务商、现有路由优先级及开关，不在导入时发起模型生成调用。
存量 ChatGPT 账号在“编辑账号”中点击“刷新模型目录”走专用目录刷新，只提交 `credentialId`，不透露原 token；
刷新后点击保存，读取失败不会覆盖模型列表。
`POST /v1/internal/gateway/console/chatgpt-model-catalog` 只接受新 token/account 或存量 credentialId 二选一，
固定访问官方地址、禁止重定向、要求管理认证及 Secret Grant，并在网络后重新检查授权。
结果带原 revision，前端校验登录、grant、草稿与 revision 后才通过正常配置保存链提交。
目录含可见性元信息，但不生成 `verified_models`，也不表示所有模型已经完成实际生成验证。

导入仍使用现有 Secret Grant 和 `secretPatches` 保存链，token 不进入前端普通路由草稿；
切换服务商、切回 OAuth 或关闭弹窗会清空表单中的 token。此入口只导入 access token 和账号头，
不导入 refresh token，也不承诺自动续期。需要长期登录时使用 OAuth 登录流程。
编辑已有 OAuth 账号不删除其原有账号头、refresh token 和其他未修改字段。

## OAuth 自动续期

通过 OAuth 登录保存且带有 refresh token 的启用账号，由后台凭据维护任务自动续期。
桌面本地 SQLite 模式和服务端模式都会启动该任务：启动后先检查一次，之后每 60 秒检查，
在 access token 距离实际 `expires_at` 不足 5 分钟时刷新。重启不会把旧 token 的有效期
重新从零计算；缺失或无法解析绝对到期时间时，按需要刷新处理。

刷新只访问 OAuth token endpoint，不调用模型，不属于模型自动测试，也不会产生模型推理调用。
本地模式通过现有配置 revision、写锁和事务恢复链保存新的 access token、到期时间，
以及上游可能轮换的 refresh token；未返回新 refresh token 时保留原值。
该过程保留账号 ID、订阅分组、账号头、模型配置和网关调用 Key，不升级存储格式。

同一授权的并发刷新互斥；调整模型、名称或其他非授权配置不会丢失正在刷新的状态。
用户重新授权或删除账号后，旧刷新响应不能覆盖新凭据。
暂时性失败按 1、2、4、8 分钟等指数退避，最长间隔 1 小时；上游明确返回
`invalid_grant` 等不可重试授权错误时停止自动重试，并记录需要重新授权的错误类别。
本地写入暂时失败时，保留新凭据在内存中，后续只重试保存，不重复使用已轮换的旧 refresh token。
正常退出会等待正在进行的刷新与保存，并额外尝试一次待保存材料；永久磁盘故障或强制结束
不能保证待保存材料落盘，失败仅记录账号标识和错误类别，不记录 token 或上游响应正文。

此逻辑需要 Gateway 正在运行且可访问授权服务。refresh token 被撤销、账号授权失效等情况
仍需重新登录，自动刷新不代表授权永久有效。仅手工导入 access token 的账号不自动续期。

## 桌面信任边界

服务器 WebView 没有新增 Tauri IPC 权限。外部新窗口只接受长度受限的
`https://auth.openai.com/oauth/authorize` 地址；拒绝其他 origin、端口、路径、
用户信息和 fragment，不在授权 WebView 中加载第三方内容。

系统浏览器启动使用独立 argv，不拼接 shell。启动器最多一个并发，每秒最多
一次，超时十秒；应用退出取消并等待自己的启动器，不关闭用户的系统浏览器。
授权 URL 和启动器输出不写入日志。原窗口同源导航限制保持不变。

以上是客户端功能合同，不代表任何真实账号、上游模型、Linux desktop 或
正式 release 已通过验收。当前验证范围见
[2026-10-02 检查点](status/2026-10-02-remote-oauth-client-flow.md)。

# FlowMusic Web 音乐接入

`flowmusic-web` 是独立的桌面服务商模板，复用既有 `producer` preset 与
`producer_compatible` HTTP 音乐执行器。它不是官方 API key 服务，也不是第三方中转。
旧 Producer 配置、模型名与目录分类保持兼容。

## 范围与调用

- 官网：<https://www.flowmusic.app/>。
- 本次只开放 `producer_music`，公共入口为 `POST /v1/music/generations`。
- 默认模型路由名为 `producer:standard`；这沿用上游 wire 名，不是抓取出的模型目录。
- FlowMusic 的默认 `client_context.selected_model` 为 `Lyria 3.5`；显式请求的
  `client_context` 保留原样，不改写歌词、提示词或自动追加创作指令。
- 新的 `flowmusic_web` special adapter 固定 Direct HTTP，并将路由能力收窄到音乐；
  不能继承 Producer 的浏览器视频分支。未开放图片、视频、文本聊天或 SSE 音乐输出。
- 账号测试默认不自动运行；音乐验收不以 `Reply with only OK.` 或 IQ 分数代替。

```http
POST /v1/music/generations
Authorization: Bearer <Gateway access key>
Content-Type: application/json

{
  "model": "producer:standard",
  "prompt": "Generate a short original instrumental music track with gentle piano, no vocals."
}
```

沿用 conversation → job SSE → clip 轮询的音乐 owner。成功结果包含 `data[].url`、
clip 元数据及完成标志；新模板的结果 `provider` 为 `flowmusic.app`。
上游受账号、额度、地区与当前网页协议影响；模板存在不等于当前账号真实调用通过。

## 账号录入

账号对话框提供独立的 **FlowMusic 网页登录并导入** 与 **FlowMusic 会话 JSON**。

网页登录沿用已解决 Google 登录兼容问题的两阶段普通 Chromium 登录流程：
先打开独占临时 profile，登录阶段没有调试端口或自动化驱动；用户登录并关闭本次
专用窗口后，才短暂重开并导入 FlowMusic 域的 Supabase 会话。不发送生成请求，
不读取日常浏览器 profile，不导入 Google Cookie，不导出完整 storageState。

手工 JSON 是 FlowMusic 专属格式，不接受其他 Web 服务商 JSON：

```json
{
  "schemaVersion": 1,
  "provider": "flowmusic-web",
  "accessToken": "<FlowMusic Supabase access token>",
  "refreshToken": "<optional refresh token>"
}
```

上限 24 KiB；未知字段、完整浏览器状态、控制字符和错误 provider 会被拒绝。
JSON 导入要求正常管理认证、敏感信息授权、配置 revision 与服务商绑定。
材料只进入 Gateway 凭据对象存储；前端草稿和会话回执只返回对象键，不返回 token。

音乐调用时不启动浏览器或 JS 引擎。运行时按对象键读取会话；接近过期时通过
Supabase HTTP refresh 更新 access token 和轮转后的 refresh token，再持久化后调用。
进程内使用固定数量的分片锁串行刷新，避免同账号并发重复轮转；这不声称跨进程 CAS
或长期在线稳定性已获真实验收。刷新失败明确要求重新登录，不静默换号。

## 本地依赖与验收边界

调用仅依赖 Gateway 的 HTTP 运行时；网页登录导入需要宿主 Node、`playwright-core`
和普通 Chrome/Edge。`FLOWMUSIC_CAPTURE_BROWSER_EXECUTABLE_PATH` 可指定浏览器，
`FLOWMUSIC_CAPTURE_TEMP_ROOT` 可指定隔离临时根。远程宿主导入未接入本入口。

定向测试覆盖模板、音乐能力限制、会话解析、敏感信息授权、对象存储引用与浏览器关闭。
真实测试 `tests/flowmusic_live.rs` 默认 ignored，必须明确设置 `GATEWAY_FLOWMUSIC_LIVE=1`、
`FLOWMUSIC_LIVE_ALLOW_GENERATION=1` 与绝对的 `FLOWMUSIC_LIVE_ROOT`。执行前必须核对旧请求，
不能将超时或进程异常当作上游未接受。测试创建不可重复执行的 submission guard；失败后
不自动重放。它支持通过 `FLOWMUSIC_CREDENTIAL_ROOT` 与 `FLOWMUSIC_LIVE_CREDENTIAL_OBJECT_KEY`
复用正常导入的对象引用，不重复登录，也不接受在命令行传入 token。

2026-11-01 的单账号真实验收已通过：公共音乐 API 返回 HTTP 200，52,266 ms 生成
2 个音频结果，两个 URL 均返回 HTTP 206、`audio/mp4` 和非空字节。因此已为新模板登记
蓝标。证据及此前超时、测试栈溢出的处理过程见
[`2026-11-01-flowmusic-web.md`](progress/2026-11-01-flowmusic-web.md)。这使用进程内公共 handler
和真实 HTTP 上游，不等于已发布 EXE 的完整桌面验收，也不代表视频或长期刷新已通过。

协议比对参考：<https://github.com/genz27/FlowMusic2API>，固定查看提交
`34296d89e830e42b1b9582215404c7c5fae2ed27`。未引入、执行或复制其代码；该快照未提供
可确认的许可证文件。本实现继续使用 Gateway 自有协议、凭据和运行时 owner。

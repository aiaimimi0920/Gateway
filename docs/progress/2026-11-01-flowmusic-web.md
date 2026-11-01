# FlowMusic Web 模板与音乐 HTTP 接入

## 目标和非目标

按用户要求新增 `flowmusic-web`，复用已存在的 Producer 音乐实现；本轮仅音乐，不扩展
图片、视频、文本聊天、第三方转发或每次调用浏览器。保留此前所有未提交 Web 改动，
未提交、推送、替换运行实例或覆盖旧包。

当前官网已联网核实，展示 Lyria 3.5 音乐产品。另只读核对
`genz27/FlowMusic2API@34296d89e830e42b1b9582215404c7c5fae2ed27`；未执行、复制或引入其代码。

## 改动

- 独立 `flowmusic-web` 模板，复用 `producer` preset、旧实现线和目录兼容身份。
- 新 `flowmusic_web` 调用流程：会话指纹、仅音乐路由、固定 HTTP，不继承浏览器视频。
- 默认 `producer:standard` 路由名、Lyria 3.5 client context；用户显式上下文和提示词不改写。
- 独立 FlowMusic 账号控件：普通浏览器登录、专用 JSON。拒绝 Google JSON 和完整浏览器状态。
- 管理认证、secret grant、revision 和服务商绑定继续使用既有管理链。
- 最小 Supabase 材料进入凭据对象存储，UI 与回执只持有对象键；不回显 token。
- HTTP 调用前读取会话；刷新体限制 32 KiB，刷新后保存 access token 和轮转 refresh token。
  固定 32 个分片锁限制进程内并发刷新，未声称跨进程 CAS 已实现。
- 音乐请求不运行 JS 引擎或浏览器；浏览器仅用于用户主动登录导入，导入后关闭。
- 单账号真实音乐验收完成后登记新模板蓝标；旧 Producer 的历史 401 不当作当前平台结论。

## 已执行验证与修复过程

1. 新模板测试先以“模板缺失”失败，导入测试先以“模块缺失”失败，随后实现到绿。
2. 最终前端相关 7 个文件 54 项通过，含导入目标变更保护。
3. FlowMusic Node 解析与生命周期、复用的普通浏览器登录 helper 最终共 7 项通过；新增
   `--disable-background-mode` 选项的合同先失败后通过。只在 FlowMusic 导入启用，其他用户保持默认。
4. TypeScript typecheck 通过；checker 自测 31 项和 effective-line ratchet 通过。
5. 第一轮 Rust 定向运行 4 项通过、1 项失败：配置候选仍暴露旧 Producer 的图片/视频
   families。已在配置候选 owner 和请求准备入口增加音乐限制；最终 3 项管理 HTTP/编译
   合同全部通过，其中明确重验了音乐能力收窄和 Direct HTTP，拒绝与成功导入分支均通过。
6. `git diff --check` 通过；已有两个文件换行提示保留，不进行无关格式改写。
7. 收尾的前端模板、专用导入、目录蓝标及导入目标保护共 4 文件 20 项通过；TypeScript
   typecheck、Node 7 项、checker 31 项、effective-line ratchet 和 Rust formatter 复核通过。
   前端测试计数是本次聚焦重验，不与首轮 54 项简单相加。
8. 最终 Rust 管理导入合同 3 项、合成公共音乐 ingress 2 项、托管会话无效材料的公共入口
   拒绝合同 1 项全部通过。后两组使用普通 libtest 线程栈，不提高生产栈，也不调用真实上游；
   无效材料明确以 `flowmusic_session_invalid` 失败，不泄漏材料、不提交生成。

## 真实验收及失败对账

隔离证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-flowmusic-20261101`。
真实凭据目录限制为当前 Windows 用户与 SYSTEM 访问，不进入仓库或发布包。

1. `live-r1`：普通登录导入等待超时；安全回执为 `stage=login`、`generated=false`。
   没有提交音乐生成。不能仅凭用户回复时间推断关闭窗口的真实时刻或超时根因。
2. `live-r2`：普通 Chrome 登录并导入成功，专用浏览器关闭，凭据已保存到隔离对象存储。
   公共 handler 的 debug 测试随后出现 `stack overflow`，无成功结果回执；没有直接重放。
   只读 SQLite 对账看到一条 `running` 审计、无 upstream status；随后通过该账号 HTTP
   GET 查询官方曲库与会话列表，均为 200 且为空。没有查到已接受任务或作品。
3. 新增合成 loopback 公共音乐合同，成功与接受后 SSE 失败两种场景均通过，且 conversation
   POST 始终恰好 1 次，失败不重放。live harness 参照已有完整 ingress 测试预留 16 MiB
   线程栈、box 请求 future，并增加独立生成开关与 `create_new` submission guard。
   这是验收程序修正；未据此声称已定位所有生产栈问题，也没有提高每个生产请求的线程栈。
4. `live-r3`：复用 r2 的凭据对象，未再启动浏览器。单账号通过正常路由选择、鉴权及公共
   `POST /v1/music/generations`，调用真实 FlowMusic conversation、SSE 与 clip 轮询。
   **HTTP 200，耗时 52,266 ms，`completed=true`、`media_completed=true`，2 个结果。**
5. 首个结果在 Rust canary 中 Range GET 通过；随后只读检查全部 2 个 URL，均返回
   **HTTP 206、`Content-Type: audio/mp4`，各读取 1,024 非空字节**。没有为媒体检查再次生成。
   r3 SQLite 只读对账为 1 条 `completed` 音乐审计，精确服务商 `flowmusic-web`、上游状态 200。

安全结果文件：

| 文件 | SHA-256 |
| --- | --- |
| `live-r3/live.safe.json` | `A297C92CCBE1DC2CBF7E11785BF649D50343C39160A14141DC99C0E0D53AAD16` |
| `live-r3/media-all.safe.json` | `BB909695F69C25B909F687EAD6FAE582F698057BF12A8B8EE9C934B86C7BE6C0` |

结果文件无 access/refresh token；`music-results.json` 仅保留生成媒体结果，不保留原始 SSE
或整个用户曲库。模板蓝标时间取全音频读取检查完成时间 `2026-11-01T04:22:35.3306341Z`。

## 代码规模

新增 `flowmusic_auth.rs` 234、`flowmusic_session.rs` 123；live opt-in 测试为 161、公共 ingress
合同为 92 有效行。此次续接中 live 测试从 127 增至 161，仍小于 250。
`gemini_auth_sessions.rs` 446 → 458、`providerCatalog.ts` 423 → 429；均未跨 500/700。
无新增超限源码，不重建或降低基线。

## 未验收边界

真实 FlowMusic 登录、公共音乐 API、两个音频 URL 读取已通过；不需要再次登录。
它使用进程内公共 handler 和真实 HTTP 上游，不等于桌面 EXE 模板操作的完整手测。
登录账号目前保存在隔离验收对象存储，没有擅自导入或覆盖用户正在运行的 Gateway 实例。
长期会话续期、跨进程并发刷新、远程浏览器导入及新 EXE 包装不在已有通过项中。

详细使用合同见 `docs/flowmusic-web.md`。

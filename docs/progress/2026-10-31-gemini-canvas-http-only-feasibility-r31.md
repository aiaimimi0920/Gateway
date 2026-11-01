# Canvas 调用阶段纯 HTTP 可行性验收（2026-10-31，r31）

## 结论

**真正 Canvas 文字生成的调用阶段可以不运行浏览器，本轮单次真实调用已通过。**
这不是普通 Gemini Web `StreamGenerate`、官方 API Key 或其他账号替代。
它是隔离 Python 原型对 Google Canvas 宿主 RPC `q4uTj` 的直接 HTTPS 调用。

当前 EXE、服务商模板和生产调用分支没有切换；它们仍使用既有 browser relay。
本记录证明当前账号/模型下的协议可行性，不等于已经交付通用 HTTP-only provider。

## 先提交推送

在试验前，Gemini Web / Canvas 多模态、模板及必要接线已经提交并推送：

- 分支：`main`。
- commit：`fb44c708ad6842937990539fd58456b090824a5c`。
- message：`feat(gemini): complete web and Canvas multimodal flows and templates`。
- `git ls-remote origin refs/heads/main` 已核实远端与本地 SHA 一致。
- 不包含并行 ChatGPT 重试/模型探测改动、缓存和运行数据；这些现场继续保留。
- staged `git diff --check` 通过，六类常见新增 secret pattern 扫描为零命中。
  这不是完整安全审计。push 提示的 13 项仓库依赖漏洞尚未处理。

本轮 HTTP 原型和此记录晚于上述 commit，未自动再次提交或推送。

## 公开协议与认证诊断

只读检查现有 Canvas 页已加载的 Google 公开静态模块 `ponblb`，确认：

- RPC：`q4uTj` / `BardFrontendService.RouteLlmRequest`。
- endpoint：`https://gemini.google.com/_/BardChatUi/data/batchexecute`。
- request field 2 为 JSON 请求正文，field 3 为操作枚举。
- 文字枚举为 `1`；当前默认文字模型为 `gemini-3-flash-preview`。
- field 4 的 share ID 仅在宿主路径为 `/share/<id>` 时设置，并非所有调用必需。

浏览器检查仅用于读取公开协议定义。没有用浏览器执行生成、刷新页面、读取隐藏
会话字段或导出新的登录状态；调试订阅已 `Debugger.disable`，用户 Canvas 页保持不变。

HTTP 引导 GET 可取得 build/session ID，但没有 `SNlM0e`。最初把缺少该字段当作
认证前置阻塞是过强判断；后续真实请求证明本轮接口接受了基于已导入 Cookie 的
`SAPISIDHASH` 签名认证，不要求这次表单具有 `at` 字段。不能再据此判账号失效。
这也不是绕过登录：请求依然依赖当前有效的 Google 账号凭据和服务端授权。

## 单次真实验收

证据目录：
`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-canvas-browserless-20261031-r31/`。

执行 `probe_canvas_rpc.py`，使用程序此前正常导入并保存的 Canvas account material，
只在内存构造 Cookie/签名，未新导出状态、修改原凭据或持久化敏感请求/响应。
使用新随机验证串，并在生成 POST 前排他创建一次性 guard；失败不重发。

| 检查 | 真实结果 |
| --- | --- |
| 引导请求 | 1 次 GET，HTTP 200 |
| 生成请求 | 1 次 POST，真正 `q4uTj`，操作 `1` |
| 生成外层 HTTP | 200 |
| 对应 RPC response frames | 1 |
| RPC error codes | 空 |
| 可解析的生成结果 | 1 |
| 随机验证串精确匹配 | 通过 |
| POST 耗时 | 1,936 ms |
| GET 耗时 | 1,686 ms |
| 浏览器启动/调用/fallback | 无 |
| 官方 API Key、普通 Web fallback、跨账号 fallback | 无 |

生成结果来自解析后的 `candidates[].content.parts[].text`，不是仅搜索外层响应是否
含有 prompt，也不是仅凭 HTTP 200。安全收据为 `canvas-rpc-safe.json`；
`canvas-rpc-once.guard` 已消费，不能为复核而再次执行该实时试验。

## 原型验证与安全边界

`python -m unittest test_probe_canvas_rpc -v`：**7/7 通过**，全部使用合成凭据和
隔离临时目录，不调用 Google：

- 真正 Canvas RPC 和字段合同，签名不含原始 Cookie。
- 只有精确生成答案才算通过；外层 200/内部错误仍失败。
- guard 已消费时不发送 POST。
- 失败仅一次 POST、不重试、无原始秘密产物。
- HTTPS、目标 host、userinfo/port、请求体大小限制。
- 有界 gzip 解压和拒绝重定向。
- 调用原型无 browser/Playwright/subprocess imports。

传输具有 60 秒 socket 超时、64 KiB 请求体上限和 8 MiB 响应上限。
该小型脚本不是生产实现：固定了本次账号索引和观测到的 source path，未覆盖
Gateway 鉴权/路由/审计/计费、取消、流式、并发隔离、会话续期和错误分类。

## 内存问题与后续建议

当前 browser pool 并非每个请求新启动浏览器，而是按 `runtimeStateObjectKey` 复用
browser/context/page。同 key 初始化也会合并；但普通 storage-state entry 会独立
`chromium.launch()`，不是所有账号共享一个 Chromium。默认最多 4 entries，空闲
30 分钟到期、每 5 分钟回收，繁忙项不被回收；容量满且都忙时返回 429。
它没有总 RSS 字节硬上限，因此用户对多账号浏览器内存成本的担心成立。

建议下一阶段以独立 HTTP transport 接入已证的 Canvas 文字 RPC，复用现有凭据
和 HTTP client，并让 HTTP-only 明确禁止浏览器隐式 fallback；浏览器保留给初次
登录/重新认证，以及尚未验收的模态，而不是让每个 HTTP 请求携带一个浏览器。

以下仍未验证，不能扩大通过范围：图片、TTS、视频、音乐、SSE/流式、多账号、
长时间凭据有效性、Gateway 公共 API 整链和真实 RSS/并发压测。未给出内存节省
百分比，也没有修改运行实例、账号、数据库、模板，或重编译/替换 EXE。

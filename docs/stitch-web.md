# Stitch Web

`stitch-web` 是独立的 Google Stitch 网页服务商，官方入口为
<https://stitch.withgoogle.com/>。它生成界面设计及 HTML 产物，不是普通聊天模型，
也不是 Gemini Web、Canvas 或 Flow 的协议别名。

## 调用与认证边界

- preset：`stitch-web`；adapter：`stitch_web_compatible`；special adapter：`stitch_web`。
- 运行时固定为原生 Rust HTTPS；浏览器执行模式覆盖不能启用浏览器回退。
- 登录导入使用普通浏览器专用窗口。登录期间没有自动化或调试连接；用户关闭该窗口后，
  导入 owner 短暂重开以读取必要的本站 Cookie，等待目标标签页完成导航，再关闭浏览器。
- 仅导入能够正常发送给 Stitch 的 `withgoogle.com` Cookie，保留原 domain/path/flags/expiry。
  不把 `google.com` Cookie 伪装为 `withgoogle.com` Cookie，不导出完整浏览器状态。
- 正常导入完成前，用 HTTP 首页参数及只读项目列表验证登录态。旧用户信息 RPC 不适合作为
  唯一认证依据：本轮它返回拒绝，但相同会话的项目列表、创建和生成均成功。
- JSON 导入入口只校验格式与作用域并准备账号草稿，不声称联网认证成功。
  当前只支持 `authUser: "0"`，格式见 `docs/google-web-json-import.md`。

## 请求与结果

操作别名 `stitch-design` 只代表 Gateway 的“界面设计”操作，不代表 Google 公开模型型号。
导入账号携带这一手工声明，不伪装成动态获取的模型目录。模板本身不预先创建无账号路由。

```json
{
  "model": "stitch-design",
  "messages": [{"role": "user", "content": "设计一个蓝色按钮的单页阅读清单"}]
}
```

可通过 `/v1/chat/completions`、`/v1/responses`、`/v1/messages`、`/v1/completions`
的文本入口适配。每次请求创建一个项目、提交一次设计，有限轮询后返回包含
`provider`、`project`、`project_url`、`screens` 的 JSON 清单作为文本内容。
每个 screen 包含 HTML 地址、可选缩略图、尺寸及名称。Gateway 不执行返回的 HTML。

目前不支持附件、工具调用、`previous_response_id` 或真正的逐 token 输出。
流式入口是生成完成后的 buffered SSE。额外入口的真实上游验收状态以进展凭据为准，
不能由公共 Chat 测试推导其他入口已实测。

生成与轮询使用本站 Nemo batchexecute RPC，不启动浏览器、JS 引擎、打码服务或 proof provider。
仅只读项目列表在服务器明确下发新的 XSRF 参数时允许正常更新并重发一次。
创建、生成及其他操作绝不自动重试；错误阻止切号或协议回退，避免响应丢失后重复扣额。
超时或断连可能留下远端项目，程序不会自动删除用户远端产物。

并发最多 2，单次最长 180 秒，提示最多 32 条消息／32 KiB；Cookie、响应与结构遍历有上限。
HTML 链接只接受官方 `contribution.usercontent.google.com/download`，不向产物地址附带登录 Cookie。

## 实测与参考

2026-11-01 已在浏览器关闭后完成一次独立 HTTP 协议闭环：项目创建、设计提交、完成状态 `3`、
1 个 screen 和 12,575 字节的真实 `text/html` 下载。随后，经用户另行批准，通过正常管理
导入与配置保存后，公共 `/v1/chat/completions` 返回 HTTP 200，另生成 1 个界面并下载
13,720 字节 HTML。模板据此登记为蓝色；没有扩展为附件、工具或全部入口的真实验收。
详细边界与测试凭据见 `docs/progress/2026-11-01-stitch-web-http.md`。

研究参考 MIT 项目 [CLI-Anything-WEB](https://github.com/ItamarZand88/CLI-Anything-WEB)，
固定 commit `931e201783cd63993209193165e19bab8729f325`。仅参考公开协议线索；
凭据作用域、原生执行器、请求约束、管理入口、错误和资源生命周期由 Gateway 独立实现。

默认测试不联网；`tests/stitch_web_live.rs` 为显式忽略的真实单次 canary，
须有用户的额外生成批准、隔离临时根目录及内存凭据桥接，不能加入默认 CI 自动运行。
live canary 自行使用单独的 64 MiB debug 测试线程栈，不再依赖调用者设置 `RUST_MIN_STACK`；
这属于测试线程配置，不代表生产运行需要为每次 HTTP 调用创建浏览器或分配同等内存。

# 2026-10-02：远程 OAuth 客户端打开流程

## 已确认的问题与实现

`ChatgptOAuthPanel` 原先始终调用服务端 `open-browser`，而
`internal_console/chatgpt_oauth.rs` 明确拒绝非 loopback 的该 action。
连接远程 Gateway 时，按钮不能在操作者设备打开授权页面。

本轮将其改为有严格 URL 校验的客户端链接。Web 使用新标签页及
`noopener noreferrer`；桌面拦截新窗口请求，仅将固定的 ChatGPT 官方授权
endpoint 交给系统浏览器。保留回调粘贴、秘密确认、显式导入、轮询恢复和旧
服务端接口。使用说明和信任边界见 [ChatGPT OAuth](../chatgpt-oauth.md)。

桌面启动器独立管理单线程和子进程，限制并发、频率和十秒执行预算。退出时
取消并 join；错误事件通过 UI 队列投递，worker 不等待 UI 线程，避免退出
路径相互等待。不新增依赖，不修改凭证/数据库格式，不开放通用 native IPC。

## 当前验证

- 修复前新增的客户端链接回归失败，原八项 OAuth 测试仍通过。
- 最终前端聚焦三文件：25/25；TypeScript typecheck 退出 0。
- 桌面 `cargo test --offline --locked --release --lib`：23/23，含六项新增
  URL、argv、并发、启动器结果、超时回收和应用退出取消测试。
- 真实 Chromium 加载实际组件及产品样式，后端使用明确标注的合成会话。
  授权站点请求由测试路由原地响应，没有连接真实供应商。
- 新标签页地址正确，`window.opener === null`，服务端 `open-browser` 请求为 0。
  粘贴合成回调、完成、显式保存后 `saved=true`；写请求仅为 create/complete/import。
- 1280×900 和 390×844 截图人工检查；窄屏 `scrollWidth=390`，无横向溢出。
  本次临时 browser session 和 18746 预览服务已关闭。
- checker 测试 31/31、最终 ratchet 退出 0；根与桌面 Rust formatter、两仓
  `git diff --check` 通过。独立子代理两次因 503 未执行，不计为独立审查通过。

所有新改源码为 UTF-8 无 BOM，均不超过 250 有效行，无大小例外：

| 文件 | 修改前 → 修改后 |
| --- | ---: |
| `connection/authorization_browser.rs` | 新增 149 |
| `connection/authorization_browser_tests.rs` | 新增 107 |
| `connection/window.rs` | 108 → 115 |
| `connection.rs` / `lib.rs` | 78 → 79 / 78 → 81 |
| `ChatgptAuthorizationLink.tsx` / 测试 | 新增 30 / 34 |
| `ChatgptOAuthPanel.tsx` / 测试 | 107 → 109 / 101 → 110 |

证据目录：
`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-remote-oauth-20261002`。
包括 `changed-files.json`、`browser-final.log`、Playwright snapshots 和
`oauth-wide.png` / `oauth-narrow.png`。构建与测试缓存同样放在 linshi；
真实用户配置、现有 release、服务和其他仓库未改动。

## 原生 EXE 集成验证补充

当前源码已构建并实际启动 Windows EXE，使用 `tauri/custom-protocol` 嵌入
官方 Rsbuild 配置生成的连接设置页。WebView 的设置页为 `http://tauri.localhost/`，
existing 模式连接隔离的 `http://127.0.0.1:18747/ui/` 合成会话页面；没有依赖
1425 开发服务器，也没有启动 gateway 后端或创建真实凭证数据库。

本次是开发验证，不是正式 release。验证脚本初版缺少 custom-protocol，随后
Windows 绝对 frontendDist 又被 Tauri 的 URL 分支解释为 file URL；修正为
相对资源目录后重新构建并验证，不把前两版构建成功当成原生验收通过。
业务源码没有因此改动，十个源码文件的 SHA-256 仍匹配此前测试证据。

浏览器启动端使用放在隔离目录的无网络替身；先以同样的 Rust Command 解析
探针确认替身路径，再点击真实 WebView 链接。它不改变系统默认浏览器或注册表。

- 合法授权链接完整传入 `url.dll,FileProtocolHandler` 后的独立 argv。
- 越界 host、HTTP、非 443 端口、userinfo、fragment、错误路径六种链接，
  即使绕过前端 href 校验也被原生边界拒绝；无新增启动回执或子 WebView。
- 启动器非零退出显示手动复制提示；模拟挂起时 10,071 ms 显示超时提示，
  对应子进程已回收。恢复正常启动后提示清除。
- 合成回调 → complete → 显式 import 后 `saved=true`；写操作只有
  create/complete/import，服务端 `open-browser` 调用数为 0。
- 启动器仍挂起时关闭原生窗口，79 ms 内 EXE 退出且子进程回收。
- 最终自建 EXE、WebView、启动器、CLI session 和 fixture 进程均已退出；
  18747 及两个隔离 CDP 监听全部关闭。WebView 缓存之外只有连接设置和存储租约文件。

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-native-oauth-20261002`。
关键回执为 `embedded-build-receipt.json`、`source-fingerprint-verification.json`、
`launcher-success-and-denials.json`、`native-ui-result.json`、`native-exit-result.json`、
`native-cleanup-result.json`，以及 `native-oauth-waiting.png` / `native-oauth-saved.png`。
最终验证 EXE 的 SHA-256：
`D125B9409B89F24C032E7E6606B8A24977470B5CBE4556528A808DC9544ABF90`。
本次独立审查代理仍因 503 未执行，不计为独立审查通过。

## 同轮发现的 Linux CI 失败

联网查询 `d4a602a8b156924ddb9645df558078418859ed55` 的
[CI 36953794399](https://github.com/aiaimimi0920/Gateway/actions/runs/36953794399)
确认 Linux job `110672205110` 已完成，并在 Rust 单元测试失败：
3271 passed、1 failed、36 ignored；不是构建仍在运行，也不是全部 Rust 测试通过。

唯一失败为 `body_read_errors_keep_the_original_provider_classification`。
实际 message 没有 URI，oracle 的全量 `.text()` 则附加了合成 response URI。
本轮读取 `wreq 0.16.1` 的实现确认：`bytes()` 为错误附加 URI，`bytes_stream()`
直接返回 body stream。只把测试 oracle 的 collector-only URI 去掉，继续完整
比较 message、code、provider、HTTP status 和 retryable；不改生产错误分类，
不删除断言或跳过测试。该文件仍为 212 有效行。

本地 `cargo test --offline --locked --lib protocol::upstream_body:: -- --test-threads=1`
完成，12/12 通过，包含该失败项和 charset/BOM、有界读取、deadline、取消清理。
重编译用时 6 分 44 秒，仍有既有 18 条编译 warnings；原始最终输出见证据目录
`body-tests-final.log`。这是 Windows 本地回归，不冒充 Linux 全套复验；
未提交推送，也未运行新提交的 GitHub CI。

## 未完成的交付

没有真实账号授权或付费 provider 调用；原生集成的浏览器启动端是隔离替身，
尚未验证默认系统浏览器和真实 OAuth 往返。没有构建、打包或替换正式 release，
临时验证 EXE 不可冒充正式发布包。本轮没有提交或推送。

GTK 两条安全公告的限定例外未获明确批准，安全门禁仍保持 fail closed。
Linux/Docker release 及原 19 项公开 API 验收仍未完成。Windows CI 的
job `110672205579` 本轮最近一次成功查询仍在运行线路矩阵；收尾刷新遇到
`API rate limit exceeded`，无法确认其后状态，不能当成成功或擅自重启。

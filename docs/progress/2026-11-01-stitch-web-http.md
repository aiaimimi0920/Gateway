# Stitch Web 独立模板与 HTTP 生成验收

## 结论与范围

已实现独立 `stitch-web` 模板、正常网页登录／JSON 专用导入、本站 Cookie owner、原生
Nemo HTTP 执行器和公共文本 API 接线。不是 Gemini 或 Flow 协议的别名。
单账号公共 Chat API 到真实 HTML 产物下载已通过，模板登记蓝色。
Flow Web 保持未完成、已暂停；本轮没有继续其生成开发。

操作别名 `stitch-design` 表示 UI 设计，不宣称为 Google 模型型号。
账号草稿仅声明该手工路由提示，`model_selection.fetched` 仍为空；模板不提前创建无账号路由。

## 真实 HTTP 证据

本轮隔离根目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-stitch-web-20261101`。
没有导出完整浏览器状态、向日志输出 Cookie，或修改现有运行实例／账号。

1. `live-01` 在导入重开时过早检查标签页，误判 `stitch_login_page_missing`。
   已修复为等待本站标签页导航与 DOM 就绪，并增加失败清理及导航时序测试。
2. `live-02` 已取到本站 Cookie，HTTP 首页参数有效；旧用户信息 RPC `o30O0e` 拒绝。
   没有创建项目或提交生成，不能据此认定登录态失效或纯 HTTP 不可行。
3. `live-03` 浏览器关闭后，`A7f2qf` 项目列表成功，随后一次 `cabgj` 创建、
   一次 `IEkn6e` 提交、`uYEY6` 完成状态 `3`、`ErneX` 取回 1 个 screen。
   官方产物地址 HTTP 200，`text/html`，12,575 字节。
4. 用户另行批准最多再生成一次后，经内存回环桥接送入正常管理 JSON 导入接口、
   隔离 revision 配置保存及真实公共 `/v1/chat/completions` 入口：HTTP 200，
   `model=stitch-design`，`finish_reason=stop`，1 个 screen，HTML 下载 13,720 字节。
   实际 HTML 含 `<title>Reading List</title>` 和 `Add book`，不是登录页或只有 HTTP 200。
   上游调用和 HTML 下载均由原生 Rust HTTP 执行，没有运行时浏览器或 JS 引擎。

可核对的脱敏证据：

- `live-03/receipt.safe.json`：导入、独立协议闭环及内存清理。
- `live-03/public-api.safe.json`：公共 API 状态、产物字节数及私有凭据对象删除。
- `live-03/generated-screen.html`：独立协议生成的 HTML，仅保存，不自动执行。
- `live-03/public-api-screen.html`：公共 API 结果下载的 HTML，仅保存，不自动执行。

远端生成项目保留，不自动删除：

- <https://stitch.withgoogle.com/projects/2634165041143960813>
- <https://stitch.withgoogle.com/projects/3326991908576339518>

第一次公共 API 测试进程在 debug 测试线程中 `stack overflow`，没有记为成功。
只读项目列表连续核对仍只有独立协议的旧项目，没有新增项目；保留旧 attempted guard 后，
以 `RUST_MIN_STACK=67108864` 修正测试线程配置，再执行批准的唯一公共 API 生成。
不是对不确定的上游生成自动重放。失败进程留下的隔离私有 Cookie 对象已精确清理；
成功测试沿正常 object-storage owner 删除其对象。导入 Node 进程已退出并清除会话引用；
Windows 崩溃测试残留的限制另见下文，不声称已清理该残留进程。

## 本地验证

- `cargo check --locked -j 1 --lib`：通过，已有无关 warning 保留；后续最终定向测试另记。
- 最终 `cargo test --locked -j 1 --lib stitch_web -- --nocapture`：9/9 通过，覆盖独立 framing、
  只读 XSRF 更新／禁止生成重放、Cookie 作用域／轮换、真实格式产物与输入字节边界。
  XSRF 新回归曾按预期先红（尚未实现 `LIST`／`read_only_refresh`），实现后通过。
- 最终蓝色登记、专用导入和普通配置保存的前端 7 文件 52 项定向回归：通过。
  两处目录 receipt／实际蓝色渲染名单已随 Stitch 登记更新；最终 TypeScript typecheck 通过。
- Stitch／共享普通登录／Flow 导入的 Node 测试：10/10 通过；均为离线测试，未恢复 Flow 开发。
- 真实公共 API opt-in canary：1/1 通过；普通 CI 中仍为 ignored，不自动消耗上游额度。
- 本地公共入口与 DirectHttp 合同：2/2 通过。
- 最终 `cargo test --locked -j 1 --test stitch_web_contract --test console_google_web_json_contract`：
  两套共 6/6 通过，包含五个 Web 类型的管理 JSON 导入与正常账号草稿。
- live canary 后续改为自行创建 64 MiB 专用测试线程栈；
  `cargo check --locked -j 1 --test stitch_web_live`：通过。没有为该测试宿主改动再次生成。
- 有效行 checker 自测：31/31 通过；ratchet 无 governed source 超过 700。

包括可选 live test 的一次最终重链接没有通过：`LNK1104` 无法打开
`target/debug/deps/stitch_web_live-76c49cd732d1df5f.exe`。本机保留了早先栈溢出的
同名测试进程 PID `45168`，查询工作集为 12 KiB；正常终止请求返回 `拒绝访问`，
`CloseMainWindow` 返回 false。没有提权、重启电脑或结束其他进程。
常规 6 项集成测试与 live test 的编译检查已独立通过，但不把这个可选测试的重链登记为通过。

权威 lexer 的本轮 before/after 测量保存在隔离根目录 `source-before.json` 与
`source-after.json`。核对 51 个相关文本均为 UTF-8 无 BOM，受影响源码最多 479 有效行；
新增模块／测试最多 222 有效行，扩展的 JSON 合同文件为 216 行，不需要新增尺寸豁免。
主要共享接线：非流式 dispatcher 413→423、stream dispatcher 321→332、认证 manager
469→479、JSON owner 206→219；没有为本功能增长任何超过 700 行的债务文件。
`cargo fmt --all -- --check`、有效行 ratchet 及 `git diff --check` 已通过。
整链成功后仅补充提示总字节边界和账号模型声明等本地正确性变更；这些变化由最终离线
定向回归覆盖，没有再次消耗上游生成额度，也没有把测试二进制当作新的桌面包交付。

## 明确未验收与安全边界

本轮真实公共验收只覆盖非流式 Chat；Responses／Messages／Completions 与 buffered SSE
已接线，但未额外消耗额度做真实生成。工具、附件、图像／音视频 API、逐 token 实时输出、
长期会话续期和多账号并发不在已完成范围，不因单次成功扩大能力声明。

生成及创建失败一律不自动重试／切号；正常 XSRF 更新仅适用于服务器明确下发新值后的
只读项目列表一次重发。Cookie 原作用域不扩大，仅向官方 Stitch HTTPS origin 发送。
响应、Cookie、并发、等待和结构遍历有上限，返回 HTML 不执行，也不向其链接发送登录凭据。

未编译或发布新的桌面 EXE，未运行正式发布全量门禁，未提交／推送；保留已有 dirty edits。
协议研究参考的公开 MIT 项目与固定 commit 见 `docs/stitch-web.md`。

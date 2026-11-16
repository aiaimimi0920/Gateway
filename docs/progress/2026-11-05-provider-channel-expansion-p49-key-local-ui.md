# P49 后续：Kiro Key 真实本地管理 HTTP、持久化与 UI 集成

日期：2026-11-05。状态：本子批定向本地验证通过，不是 P50、整卡或真实平台验收。
仓库：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`；分支 `main`，HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`。

## 1. 本批范围

前批[公共调用链](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-05-provider-channel-expansion-p49-key-public-pipeline.md)
已经通过，但其管理 fixture 使用合成 backend，浏览器 fixture 使用保存 mock，不能证明正常 UI 与真实本地持久化连接。
本批新增测试专用 host：生产 `build_app_state` → `RouteConfigRuntime::new_local/recover_startup` → SQLite 与真实 TCP router。
公共 Key 通过正常管理 API 创建和充值；不手工写 DB、不用 FakeRedisBackend、不用 fallback API Key 代替正常 Key。

Kiro 产品地址/Key 绑定照常验证，仅最终传输沿既有 `cfg(test)` task-local 接缝转到不转发的 loopback 上游。
普通 EXE 不新增任意目标覆盖开关；`dev-protocol-override` 不是 URL override，本批不扩大它。
正常完成先 draining、关闭并 await HTTP owner、等待 local finalizers/关闭 SQLite、关闭 FreeBuff owner；显式浏览器 fixture 的成功 receipt 在资源关闭后才写。

真实 UI 使用正常 `main.tsx` 的隔离 Rsbuild 产物与透明 loopback API proxy；不执行仓库原 publishing/删除 hook，不改工作区 dist。
proxy 不制造 grant、不改写 Host/Origin/auth/body。浏览器正常登录后，由现有登录流程取得敏感权限；HTTP 记录包括正常 session verify/confirm-secret-access，
不是人工注入 grant，也不宣称出现了独立的二次确认弹窗。添加模板、录入 Key、metadata Keep 和清空均由正常页面完成，保存落入真实 SecretPatch/CAS。

## 2. 已发生的验证与修正

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-kiro-key-local-ui-20261105`。

- 写前 `client.rs` 471 effective lines；本批仅添加两行 `cfg(test)` 模块接线，473 effective lines。
- v1 定向编译失败：在测试线程外创建 bootstrap future 再要求 Send，触发 `Executor/Send is not general enough`。
  仅改测试 runner，让 future 在独立 current-thread runtime 内构造，不改变 SQLx/生产 bootstrap 语义。原日志与冻结输入保留。
- 只读审查指出：自动重启如果换端口，旧 grant 的 403 也可能来自 Host/Origin 改变；v2 改为同端口重建，保持该边界不变。
- 只读审查还指出浏览器完成 receipt 不能先于关闭写入；已调整顺序，并增加第一次重启 Key 存在、第二次重启 Key 已清空的显式断言。
- `local-tcp-v2`：1 passed / 0 failed / 0 ignored；编译 7m07s，测试 0.48s，27 项验证输入前后无变化。
- `browser-ui`：显式 `--ignored` opt-in 的 fixture 实际运行，1 passed / 0 failed / 0 ignored，运行 539.81s；不是把默认跳过项计作通过。
- 首次公共回归 filter 拼写顺序不符，实际匹配 0 项，不计通过；改为 `kiro_api_key_pipeline_` 后 `public-regression-v2` 的 10 项全通过，运行 4.22s。
- 本批按完整测试名称去重 12 passed / 0 failed / 0 ignored，其中 2 个新增测试（自动 TCP 与显式浏览器 host）。不叠加前批 200 项，也未无故重跑其余 190 项。
- Rustfmt 四个 owner 均通过。checker 31 项通过；ratchet 扫描 4361 文件、14 项 immutable runtime artifacts，`governed source >700=0`。
  首次错误地将绝对输出路径传给只接受仓库内相对路径的 `--json`，exit 2；改用不写报告的官方 ratchet 入口后 exit 0，原失败保留。
  首次 PowerShell 零输出日志捕获另触发 `WriteAllLines` 的 null 参数错误，仅修临时捕获命令，不改 checker 或生产代码。
- 生产通用后台 scheduler 不在该 fixture 中启动，不外推其生命周期验收。

## 3. 正常 UI 与真实服务的实际结果

1. 从正常目录选择 `kiro-api`，保存 `kiro-api-key` 空池，再录入合成 credential `ui-key` 和原样模型 `Exact.Model/Version`。
2. 管理 API 实查已提交 revision，不以弹窗关闭或“正在保存”提示当成功；管理响应不包含合成 Key。
3. 正常管理 API 创建公共 access key、充值 4 条；公共 JSON 返回 `local-answer`，持久余额 4→3。
4. 正常编辑手工模型，增加 `Local.Metadata.Model`；Key 输入保持未修改的 Keep。真实保存后，在同目录、同端口重建 runtime，revision `r3-1668065e8f07` 和 Key 保留。
5. 页面 reload 后保留 credential ID、两个原样模型及调用记录；复用原公共 token 的 SSE 返回 `local-answer` 和 `[DONE]`，余额 3→2。
6. 正常页面通过敏感读取入口载入合成 Key，再明确清空并保存；再次同目录、同端口重建，revision `r4-210aa96bacb6` 保留，后端断言 Key 为空。
7. 清空后的公共请求返回 400，余额仍为 2；上游只收到先前 JSON/SSE 两次调用，不因失败新增调用。
8. fixture 对两次上游请求逐项断言 bearer、`TokenType: API_KEY`、AWS target、精确模型、`KIRO_CLI` origin 和无 `profileArn`。

自动 TCP 测试另覆盖 stale revision 的 409、相同 Host/Origin 下旧 grant 在 runtime 重建后失效的 403、显式新 grant 清空和二次重建后空 Key 保留。
正常 UI reload 的现有登录流程重新确认权限，不将其误报为浏览器直接使用 stale grant 的负例。

证据：[最终回执](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-kiro-key-local-ui-20261105/closeout.json)、
[浏览器完成回执](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-kiro-key-local-ui-20261105/browser-runtime/completed.json)、
[JSON 结果](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-kiro-key-local-ui-20261105/before-result.json)、
[SSE 结果](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-kiro-key-local-ui-20261105/after_restart-result.json)、
[清空后结果](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-kiro-key-local-ui-20261105/after_clear-result.json)。

页面截图仅证明页面身份和模型保留；API Key 的圆点是编辑器 placeholder，不单凭截图推断秘密是否存在，清空结论来自后端断言及公共请求结果。

![重建后的正常账号编辑页](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-kiro-key-local-ui-20261105/ui-cleared-after-rebuild.jpg)

## 4. 来源、资源与剩余边界

四个 Rust owner 的 effective lines 为 `client.rs` 473（写前 471）、主测试 200、runtime 146、浏览器 fixture 89；均不需要软上限例外。
27 项 Rust 验证输入与各成功运行的冻结 SHA 相同；这些是定向输入，不冒称整个 dirty worktree 被冻结。
前端本批无源码修改，正常入口构建通过；截图、产物及临时测试脚本都保留在上述 linshi 目录。
最终 `git diff --check`、八个源码/文档 owner 的含未跟踪文件 whitespace 检查、UTF-8 无 BOM 与文档本地链接检查通过。
收尾脚本一度把 exit 0 的十条既有文件 CRLF→LF 提示误当 whitespace 失败；仅修临时回执分类，提示原样保留，未改这些无关文件的换行。
本批逐文件复核了 test-only 网络接缝、管理鉴权/秘密回显、线程/HTTP/SQLite/上游关闭与 opt-in 时间界限；生产文件仅增加两行受 feature 控制的测试模块接线。

Rust fixture 先 await HTTP/SQLite/FreeBuff 和合成上游关闭，再写成功 receipt，原 Cargo session exit 0。
浏览器标签已关闭。Node server 的原非 TTY session 不可写 stdin，因此核对 PID 和完整脚本路径后仅终止本批 Node 进程；其 session exit 1 是清理终止，不是 Web build 或业务测试失败。
已核实本批后端 58876、前端 60914 两个监听及两个 owner PID 均不存在；未停止任何其他服务。

这是同一测试进程内重新 bootstrap AppState/runtime，不是原生 EXE 进程/OS 重启或完整后台任务生命周期。
无真实账号、推理、subscription credits 或当前资格证明。真实 PG→对象→upsert/cache、旧 route/未变 source hash DB 自动升级、当前 scope/profile/订阅/ARN/业务 Region、
跨应用 receipt、Vertex/Luma/Grok/Web/媒体/Qwen 剩余渠道，以及 P50 全范围无凭据集成/devtest、P60 集中录制、P70 逐渠道验收继续保留。
累计新增模板仍 13，不增加整卡 `offline_ready`，不标 R02/W02 或总目标完成。
未读取 `.ng` 或历史凭据、未创建 Redis/容器、未出包、未动在线实例、未提交推送、未删除 linshi 外内容。

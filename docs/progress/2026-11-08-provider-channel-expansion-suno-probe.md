# Suno Web：单账号基本音乐测试与逐次费用确认

日期：2026-11-08。R15 / W11 的限定增量；默认直连基本音乐测试子集已取得下列离线验证。总目标保持 active，不是整卡 `offline_ready` 或 P50/P60/P70 完成，不用此子集替代完整渠道。

## 改了哪些内容

正常账号卡的测试入口进入基本音乐表单，使用现有 Dialog、模型控件和测试 owner，不增加计划、保存或自动调度。显式配置/alias 与既有四个 mapper ID 求交，不注入权益或当前模型目录；每次重新确认生成费用，说明未知扣额、不另执行收费下载/转换及本地关闭不代表远端撤销/退款。

同一管理 provider probe 新增 `media.operation=music`，仅一个精确账号、已配置模型/alias、typed SunoWeb 与空参数；拒绝 scope/plan、未确认或参数覆盖，强制 `waitCompletion=true`。复用普通 sender 与前批 MP3 owner，全部实际音频验证后才能 passed/恢复健康；不另扣普通接入 Key，不伪造 usage/credits。成功 audit 封闭为操作/确认、数量、wire model、`supplierCharge=null`，不保存 prompt、歌词、clip、签名 URL 或 session；结果没有文本答案或 IQ。

核验发现既有浏览器 worker 只填提示词，未把选定模型绑定到 native POST；因此本批 BrowserBacked（含 endpoint override）在前后端发送前明确 unsupported，而不是付费生成错误模型后报通过、或静默强制 DirectHttp。既有公共浏览器音乐链保持原样；精确 browser model binding 仍是后续必做，不用此门禁宣布该缺口完成。

正常 Global→Pilot→Suno 现传入 active revision 和内存管理 Grant，绑定费用确认与结果 generation；只更新其他服务商的 revision 或 Grant 更换也撤销 pending/隔离旧结果，不依赖一个可能保持 true 的“草稿已保存”布尔值。账号/材料/模型/提示词/草稿变化及关闭沿原 owner 隔离。无后台生成、重放、换号或退款承诺。

## 运行证据与真实失败

证据目录：[本批产物](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-suno-probe-owner-20261108)。一次性脚本、SQLite、截图和日志在此目录，不读取真实 `.ng` 或凭据。

- `red-ui` 复现旧入口错误地进入文本/IQ 表单；首轮 UI 11 项失败是夹具把产品固定地址误拼成 `studio-api.prod.suno.com`，只修为仓库已有 `studio-api-prod.suno.com`，不放宽目标校验。
- `red-revision-ui` 两项真实红测：账号内容不变时旧费用确认及迟到结果跨 revision；新增正常 global 接线后通过。`red-grant-ui` 同样复现 Grant 单独变化；只补本批 Suno 身份接线，未改其他产品语义。
- `red-browser-mode-ui` 三项真实红测：browser/provider/credential/endpoint 配置仍可进入费用确认；补显式不支持门禁，不以错误模型生成充当精确测试。
- `rust-probe` 编译 E0603：测试挂在 provider_runtime 下无法访问 upstream 私有资产测试入口；只把测试挂到正确 upstream 私有作用域，不扩大生产 API。
- `rust-probe-r2` 为 9 passed / 2 failed，两个失败均为夹具在管理 Key 请求后对已有借用的 Arc 调用 `get_mut().unwrap()`。改为在 router/审计借用前配置 client，不改变生产共享 Arc 语义。并行 Dim OAuth 输入变化及本批 UI 变化另记来源，不把中间结果相加或冒称当前全工作区通过。
- `disabled-final` 实际失败（exit 101，4 个 E0425）：`cfg!` 只在运行时拒绝，关闭 Suno 后仍解析不存在的协议函数。仅在本批模块改为编译期隔离，关闭入口明确拒绝，成功记录不可达时暴露内部不变量；没有补虚假的默认型号或成功。修复后的关闭配置、重新编译的正常测试与生产库均通过。

### 最终通过范围

- `rust-probe-r4`：11 个新增音乐测试全部通过。冻结同一次编译的测试 EXE 后，`contract-final-r2` 按完整测试名去重 **252 passed / 0 failed / 1 ignored**；已包含上述 11 项，不与 r3、首轮 contract 或历史音频批的 224 项相加。唯一跳过为既有 Luma Agents 的显式浏览器宿主测试，不计为通过。
- `production-final-r2`、`disabled-final-r2`：正常 runtime-defaults 和仅去除 Suno leaf 的生产 lib 检查均 exit 0；代表性关闭只验证编译，不声称完整 feature 矩阵或关闭模式业务测试。
- `ui-final`：8 文件、93 passed / 0 failed；`typecheck-final-r2`、`isolated-ui-build-final` 均 exit 0。隔离构建产物在本批证据目录，不发布或清理共享 dist。
- 真实隔离浏览器已验证打开零生成、每次确认/取消、Escape 恢复焦点、一次合成调用与结果及 390×844 / 1280×800 无文档水平溢出；这是实际组件+合成回调，不是正常浏览器到真实管理 TCP 的联合验收。自建页面、server 和监听已收尾，截图及独立回执保留。
- 12 份 Rust 的官方格式检查（改动模块在修复后补查）、checker 31/31、最终 ratchet 5087 文件/14 immutable artifacts/`governed source >700=0` 通过；21 份本批源码均不超过 500 有效行，最大 `console_model_probe.rs` 为 448→451。策略、baseline、exceptions 未修改；最终编码、输入来源与范围 diff 审计见 `audit.json`。保留当前生成的有效行报告，不用旧报告覆盖并行任务。

`test-binary-r4.json` 记录固定 EXE 的 SHA256 与来源 receipt。r4 编译期间存在外部 Dim OAuth、共享 runtime/state/router 等变更；本批 Rust 输入在该运行前后及收尾保持一致，前端消费输入也与其最终回执一致。正常生产检查及最终 contract 运行的前后 4557 项选定输入稳定，但这不反推 r4 编译读取了整个仓库的单一快照，也不声称最新全仓已联合回归。保留并行修改，不为未改本批消费者的外部变化无限重编译。

## 剩余范围

当前模型/feed wire、Clerk/device/受众/challenge/refresh、精确 browser model binding、完整生成参数/其他音频格式/真实播放与权益、正常 UI→管理 TCP 联合、Server/PG/独立进程/EXE 整包仍需接力；OAuth/MCP 仍为 discovery-only，未增加模板或客户端/工具实现。不改旧图片/视频分支，不用 MP3 测试证明其他格式或未来 URL 不变。

计划仍累计 17 个新模板，无新增整卡 `offline_ready`，P50/P60/P70 与其他渠道未完成。无真实凭据/平台生成/付费下载，未创建 Redis/Docker、修改既有实例、出包、提交或推送；P34 历史授权不复用。

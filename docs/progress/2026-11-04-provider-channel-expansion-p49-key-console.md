# P49 后续：Kiro CLI Key 正常模板、材料暂存与管理保存

日期：2026-11-04。工作区：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`。
本批完成正常 Console/SecretPatch 子链的定向离线验证，不是总计划 P50、R02/W02 整卡或真实账号验收。

## 1. 改了哪些内容

- 新建模板 ID 和后端 preset 为 `kiro-api-key`，默认 provider ID 与中英文展示名为 `kiro-api`。
  既有 provider ID 不迁移；API 分类在共享 `kiro` profile/`kiro_compatible` executor 之前按精确 preset 判断。
- 模板创建空账号、空模型、空路由的显式凭据池，固定 `https://runtime.us-east-1.kiro.dev`，不挂 OAuth 登录、
  重新授权或自动目录。自动测试关闭；`modelSelection: all` 仅指操作员显式声明的模型，不是所有上游模型。
  不用 `selected + 空 models` 绕过现有公共测试策略校验，也不猜默认模型、IDE 别名或模型权益。
- 复用真实 `ConsoleGlobalDialogs → CredentialDialog → useCredentialDialogEditor` 与原 secret builder。
  新增/替换/清空要求 Secret Grant；普通编辑默认 Keep，不读取已存 Key，稳定账号 ID、metadata、enabled 和模型保留。
  Key 明文只在 secret edits，不进入普通 route document。
- 新材料绑定稳定 provider/credential ID、Key 产品、固定 runtime 和 auth mode，最终提交再次核验。
  暂存后更改产品/目标时严格保存拒绝，非严格 preview 省略不安全 edit；metadata-only Keep 保留待 autosave 的
  原 replacement 及绑定，clear 则覆盖它，不重放旧 Key。
- 后端 Keep 继续复用原完整 destination projection。新增 Key owner 识别 preset、有效账号/provider auth mode
  及 runtime host，禁止 Keep 时抹去/进入产品、改变目的地、增加共享 Key 消费者或清 override 借用父级 Key。
  独立新材料、明确 replace/clear、稳定 ID 重排与正常 metadata/模型编辑不受损。
- 正常管理 HTTP 的 validate/PUT 沿原 router、SecretPatch、expected revision/CAS；跨产品 Keep 在编译激活前拒绝，
  grant/Origin/revision 约束保留，响应脱敏。未另建保存或认证机制。

主要 owner：
[模板](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/apps/desktop/src/features/console/providerCatalog.ts)、
[暂存绑定](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/apps/desktop/src/features/console/kiroKeyMaterialBinding.ts)、
[暂存生命周期](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/apps/desktop/src/features/console/useCredentialDialogEditor.ts)、
[最终 secret builder](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/apps/desktop/src/features/console/credentialDocument.ts)、
[Keep owner](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/console/secrets/keep_bindings.rs)。

官方/固定许可参考与后端 wire 沿用
[上一后端子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-04-provider-channel-expansion-p49-key-backend.md)。
Key 在同用户 subscription credits 维度共享，不宣称与 OAuth 独立余额；固定逆向兼容 HTTP 不冒充官方通用模型 API。

## 2. 验证、失败与来源

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-kiro-key-console-20261104`。
Cargo 沿用 `--locked --jobs 4 --no-default-features --features runtime-defaults`，测试栈 16 MiB、TEMP/TMP 在本批目录；
不触发默认 management-ui 发布/清理 hook。

| 检查 | 实际结果 |
| --- | --- |
| Keep 行为红测 | `red-secret.log`：3 passed / 4 failed；复现产品身份擦除、进入 Key、有效 credential 与继承消费者缺口 |
| UI 行为红测 | `red-ui.log`：2 passed / 4 failed；复现模板不存在、共享 adapter 误归 OAuth |
| 暂存行为红测 | `red-staging.log`：1 passed / 4 failed；复现缺绑定、metadata Keep 丢 Key、改目标与无绑定替换缺口 |
| 定向前端 | `green-ui-v2.log`：4 文件 25 passed；含15项新增显式行为测试，不叠加到扩大结果 |
| 新增 Rust 合同 | `green-rust-v1.log`：9 passed（7项 SecretPatch、2项正常管理 HTTP） |
| 相邻 Rust | `related-rust.log`：115 passed / 2 ignored；与新增组按 suite/name 去重共124 passed / 2 ignored |
| 扩大前端 | `regression-ui.log/json`：38 文件 277 passed / 3 failed / 1 skipped；不是全仓绿 |
| 写前源码对照 | `before-catalog-control.log`：78 passed / 同3 failed；当前两文件新增模板自然多3个参数化 case |
| 类型与构建 | `typecheck-final.log`、`safe-build.log` exit 0；真实 main.tsx 和合成夹具一起隔离构建，不发布到工作区 dist |
| 静态 | Rustfmt、ratchet 通过；checker31项复用未变输入的前批 `checker-tests.log`；最终 diff/UTF-8/hash 结果见 `closeout-exits.json` |

扩大前端三项失败均在本批写前 `providerCatalog.ts` 的 Vite overlay 对照中复现：
`providerCatalogHealthDefaults` 的 unmatched-products、`providerCatalogNames` 的 qwen-api 使用空 base 触发真实地域/workspace
校验；gemini-canvas 的旧默认名不满足统一后缀断言。不回滚工作区、不修改测试掩盖，不放宽 Qwen 生产验证。

当前13项源码逐项匹配 `source-green-input-v1.json`；`source-audit.json`、`scoped-source.diff` 和 `effective-lines.json`
保留范围及写前对照。源码最大488有效行，新文件最大190，没有 >700 增长或 baseline/exception 修改。
只读独立审查核同一13项 SHA，未发现本批新增可达缺陷；它不代表另行执行测试。
最终扩大文档链接检查另发现总计划历史§10.3的两条linshi日志已不存在；写前副本同样引用这两条路径。
保留该失败与原历史记录，不伪造日志；本批新增链接、UTF-8无BOM和diff检查通过。

## 3. 真实浏览器的合成隔离检查

专用 Edge 标签加载真实 Catalog/GlobalDialogs/CredentialDialog/editor/secret builder；仅保存明确模拟，未连接管理或上游服务。
验证空池→添加 Key/精确模型→再次 metadata Keep→合成 replacement、撤销敏感权限、稳定 ID、无 OAuth/目录入口、clear
覆盖 replacement 与 Escape 焦点恢复。普通 document 无 Key 明文，consoleErrors 为空。

实测 viewport 是 **1280×900 / 390×844 CSS 像素**，不是初始请求但受浏览器缩放影响的尺寸；页面及 dialog 无横向溢出。
截图为 `wide-key-add.jpg`、`narrow-key-edit.jpg`，观测为 `visual-observations.json`。
viewport 已 reset、专用标签已关闭；核完整脚本路径后仅停止本批 Node PID39776，`server-cleanup.json` 的 remainingListeners=0。
该 session 因主动停止 exit1，不当作构建失败；一次 stale AX节点刷新后恢复，不是应用缺陷。

## 4. 未完成与接力

1. Key folder 专用分类/归一化和错误池拒绝尚未实现。旧 shared adapter 会归入 `kiro-compatible` OAuth normalizer，
   不能据此声称 Key import/export 已安全支持；正常模板本身没有新增 folder sync 调用。
2. 公共完整 pipeline 的路由、Chat/Messages/Responses JSON/SSE，以及正常 UI→真实管理服务→保存→公共调用仍须集成验证。
   本批真实管理 router 使用合成存储 backend，不是 Redis、OS重启、原生 EXE 或端到端 UI 服务验收。
3. 不增加目录/更多 Region/资格假设；真实 Key、模型、额度和最终 wire 留 P60/P70。
4. Kiro旧route/未变source hash DB自动升级、当前最小scope/profile/订阅/ARN/业务Region、跨应用receipt，以及
   Vertex OAuth/Express、Luma Agents、Grok余项、其他Web/媒体与Qwen OAuth待决继续保留。

累计新增前端模板由12到13，**不新增整卡 offline_ready**；总目标、P50/P60/P70继续执行中。
未读取 `.ng`/历史凭据、登录、真实推理、创建 Redis、出包、替换在线实例、提交或推送；未删除 linshi 外内容。

# Vertex Express 正常 Console、Key 暂存与精确账号测试子批

日期：2026-11-05。仓库：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`。
状态：本子批定向离线验证完成；不是 R11/W07、P50 或总计划完成。
承接 [后端身份与最终发送子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-05-provider-channel-expansion-vertex-express.md)，
继续遵守 [Vertex 分渠道合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/plan/provider-vertex-channels-contract.md)。
HEAD 仍为 `7969f12c85efceacf2a5e384e143b5c39a550981`；保留原有大量 dirty edits，不提交、推送、reset 或清理。

## 1. 实际改动

1. 在正常目录加入独立 `vertex-express-api` 模板，保留原首项 Bedrock；固定 global publisher-model base，
   空账号池、空模型和路由，默认不自动测试。Provider ID 可改，不从该 ID 推断产品或材料身份。
2. 复用普通账号表单、secret edits 和 SecretPatch builder 实现新增/替换/metadata Keep/显式清空。
   Key 不写普通路由草稿；metadata Keep 不丢尚未自动保存的 replacement，Clear 覆盖 pending replacement。
3. 暂存绑定稳定 provider/credential ID、原产品、精确 base 和 auth mode；最终 builder 再验证目标、profile 与调用流程。
   重排按 ID rebasing；换产品/受众或缺 binding 时严格保存拒绝，非严格预览省略危险材料，不偷偷改投其他渠道。
4. Express 只显示手工模型，不显示无用目录、登录、刷新协议按钮或空目录状态占位；单栏编辑器在宽/窄屏均复用原样式。
   模型是 1–2048 字节 ASCII 单资源段；拒绝完整 resource、斜杠、百分号编码、query/fragment、wildcard 与方法后缀。
   模型映射、账号编辑和测试计划沿产品 policy；不扩大 generic/Bedrock 旧语法，不借 AI Studio 目录或猜默认模型。
5. 正常管理保存合同覆盖 Grant、Origin、原 revision/CAS、稳定 ID、模型、Keep/clear、响应脱敏及换产品 Keep 拒绝。
   此处使用真实 router 和合成 storage backend，不是生产 SQLite/Redis 或真实 TCP 持久化验收。
6. 精确账号 probe 复用现有 Gemini canonical dispatcher 和最终 Express plan/header owner；选择的 alias 解析到该账号的精确模型，
   仅携带选中 Key，不做账号 fallback。无显式请求不发目录或生成；disabled/错误模型/错误 Origin/无 Grant 不触达 transport。
   通用 HTTP probe 在网络前拒绝 Express 材料。probePoint 显示 Vertex Express，而非笼统 Gemini API。
7. Express 文本 proof 拒绝工具和超过 8192 字符的回复，不能截断后报成功；保留原有非 Express Gemini/Web 行为及秘密脱敏。
   没有修改 S06 共享 Gemini 编码器、parser 或 translator 内部，也没有新造存储、OAuth 或转发服务。

主要 owner：

- [前端产品/暂存合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/apps/desktop/src/features/console/vertexExpress.ts)
- [正常模板](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/apps/desktop/src/features/console/providerCatalog.ts)
- [secret builder](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/apps/desktop/src/features/console/credentialDocument.ts)
- [产品模型准入](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/provider_model_id.rs)
- [精确生成 probe](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/provider_runtime/gemini_model_probe.rs)
- [管理保存合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/tests/vertex_express_console_contract.rs)
- [精确 probe 合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/provider_runtime/vertex_express_probe_tests.rs)

## 2. 验证与失败记录

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-vertex-express-console-20261105`。
Rust 固定 `--locked --jobs 4 --no-default-features --features runtime-defaults`，TEMP/TMP 在本批 `tmp`；
显式清除 `RUST_MIN_STACK`，不增测试/生产线程栈，不启默认 `management-ui` 的 Web 发布/清理 hook。

### 2.1 红测与工具问题

- `red-ui.log`：13 failed / 1 passed，模板、模型及暂存路径的真实行为红测；第一组实现后 14 项通过。
- `global-ui-v1.log`：8 passed / 2 failed，发现 disabled 拉取按钮仍显示，以及无权限新增先报空 Key 而未请求 Grant；
  修正为 manual-only 控件和先检查敏感权限。浏览器视觉检查另发现无目录模式残留 48px 空状态，已去除并加强同一测试。
- `red-rust.log` 是 Cargo filter 参数错误；`red-rust-v2.log` 是 test-only seam 私有导致 E0603，都不是行为红测。
- `red-rust-v3.log`：长模型 ID 真实失败，但 generic probe 夹具强制解析缺失 JSON body，发生 EOF panic。
  改用接受任意 body 的极简 loopback server，先停止/join 再断言，未放宽生产限制。
- `red-rust-v4.log`：1 passed / 4 failed，干净复现长模型被误拒、generic sender 返回成功、产品名错误和超长回复截断后成功。
  工具回复当时已被共享完成规则拒绝；本批明确文本-only 条件，不虚构该项先前也误成功。
- 首次扩大 Rust 脚本因 Windows PowerShell `ErrorActionPreference=Stop` 将 Cargo warning 包为 NativeCommandError 提前退出；
  修 runner 按实际退出码判断后完整执行，未改生产或压制 warning。三份 PowerShell UTF-16LE 日志仅转码为 UTF-8 无 BOM，
  保留解码后内容，见 `encoding-normalization.json`。
- `affected-ui-final.json` 的 wildcard 参数没有被 Vitest 当作文件 glob，exit 0 但只运行 2 项，不算扩大回归；
  最终从原 JSON 回执读取精确 33 文件重跑，见 `affected-ui-final-v2.json`。
- 首次快速 viewport 截图存在旧画面；视觉复核后丢弃该证据，使用稳定状态的 `getScreenshot` 重新保存，未用错图证明布局。

### 2.2 最终结果

| 验证 | 最终结果 | 证据 |
| --- | --- | --- |
| Express 库测试，含本批 5 项 probe | 18 passed / 0 failed | `green-rust.log` |
| 扩大库回归，包含上述 18 项 | 176 passed / 0 failed | `affected-lib.log` |
| 9 个正常管理/秘密/相邻 probe 合同目标 | 105 passed / 0 failed / 1 ignored | `affected-contracts.log` |
| 主配置按 target/test name 去重 | **281 passed / 0 failed / 1 ignored**；其中本批新增 7 项 | `test-summary.json` |
| 仅关闭 Google Agent Platform 执行线的代表性配置 | 6 passed / 0 failed | `feature-off.log` |
| 前端本批四文件 | 24 passed，已包含于下一行 | `affected-ui-final-v2.json` |
| 前端受影响精确 33 文件 | **166 passed / 0 failed / 1 skipped** | `affected-ui-final-v2.json` |
| TypeScript、隔离 Rsbuild | exit 0 / exit 0 | `typecheck-final-v2.log`、`safe-build-v2.log` |
| 有效行 checker | 31 passed / 0 failed | `checker-tests.log` |
| scoped Rustfmt、ratchet、Git diff | 通过 | `rustfmt-final.log`、`ratchet-final.log`、`diff-check-final.log` |

Rust 唯一 ignored 是 `session_material::session_material_frontend_to_secret_resolver_roundtrip`，
需要显式导出的合成前端夹具。UI 唯一 skipped 是接受显式 private worker sample 的 opt-in 测试；均未计入通过。
关闭配置和 checker 单列，不叠加到主配置；已有非零 warnings 保留，不宣称全 feature 矩阵、全仓回归或 warning-free。
三个最终 Cargo 阶段退出码均 0，见 `affected-exits.json`；本批 Cargo 已退出。

### 2.3 UI、来源与资源

真实 Edge 运行隔离 fixture，使用真实目录/GlobalDialogs/账号编辑器/SecretPatch builder，**最终保存明确模拟**，
所有管理、授权、目录网络调用均由 fixture 拒绝，不连接真实 Gateway 或平台。实际观察空池→添加 Key/手工模型→
未自动保存的 metadata Keep→replacement patch→替换/clear、敏感权限撤销拦截、非法模型拦截、Escape 与按钮焦点恢复。
`synthetic-key-console` 的 replace 观察中 `hasRawSecretInDocument=false`；不以其他未匹配 fixture 哨兵的首次观察代替该断言。

最终实测 390×780 与 1365×900 **CSS 像素**，document scrollWidth 等于 innerWidth；弹窗不越界，目录空状态为 0。
浏览器原有缩放未更改，物理 override 与 CSS 像素不同，回执记录实测值而非猜测尺寸。
截图：[窄屏](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-vertex-express-console-20261105/ui-narrow.png)、
[宽屏](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-vertex-express-console-20261105/ui-wide.png)。
两个专用标签已关闭，viewport 已 reset；仅停止核对命令路径的新 helper PID 37156、43324，56601/56070 listener 均 0。
详见 `browser-proof.json`、`browser-cleanup.json`，不碰用户旧标签或其他 Node/在线服务。

`source-final-audit.json` 复核 **69 项选定源码/依赖输入**与测试前冻结 SHA 一致，不是全 worktree、EXE 或包来源凭据。
27 个选定源码文件均低于 500 effective lines，其中 `CredentialDialog.tsx` 保持未改；增量 diff 见 `scoped-diff.patch`。
`credentialDocument.ts` 488→494，`providerCatalog.ts` 465→476，editor 349→355，
产品 helper 50，新 probe 测试 248，正常管理测试 195；无行数例外或基线重建。
ratchet 扫描 4,376 文件，14 个 hash-bound immutable runtime artifacts 单列，governed source >700 为 0。
使用同一官方 ratchet 的无报告写入入口，避免覆盖原有 dirty artifact；源码、文档及日志均 UTF-8 无 BOM。
安全/资源复核聚焦固定受众、秘密不入普通草稿/错误、原 Grant/Origin/revision、有限模型/Key/答案、原超时与清理；
未增加后台 frontend 任务、重试、并行秘密存储或真实外部副作用。

## 3. 状态与接力边界

- 本计划累计新增模板 **13→14**，新增的是 Express Key 正常入口，不是 `vertex-oauth` 已完成或新增真实验收/蓝色标记。
- 下一子批先补 Express 公共 router→route→sender→finalize 的 JSON/聚合 SSE、真实 usage/audit、取消/退款和错误准入；
  完整 decoder 终态、工具/reasoning、阻断、多 choice 与实际 stream 合同仍不能由共享 parser 名称或简单合成 STOP 证明。
- 正常 main UI→管理 HTTP/TCP→SQLite 保存→公共调用及 runtime 重建尚未执行；本批合成管理 backend 和 UI 保存 mock 不替代它。
  Server PG/Redis/Hosted、真正多进程/OS 重启、出包和原生 EXE 继续单列，不把 Kiro 的成果自动套给 Express。
- Full Vertex OAuth 独立 client/scopes/redirect、project/location、typed context、Bearer 和持久化 refresh 仍未实现；
  不借 Antigravity token/client，不把 Express 固定 global 外推到 full Vertex。
- 不读取真实 `.ng`、历史凭据或账号；没有真实 Google 调用、目录、刷新、模型/额度验证，没有新 Redis/Docker，
  不复用旧容器授权，没有删除现有数据、替换在线实例、运行正式发布或交付 EXE。
- 其余原定渠道、P50/P60/P70 与集中录制/逐渠道真实验收全部保留，总目标继续 active；现在不要求用户提前录制凭据。

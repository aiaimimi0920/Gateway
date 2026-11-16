# FreeBuff 正常登录与稳定账号重授权 UI

日期：2026-11-06。仓库：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`，`main`，HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`。工作区包含大量既有未提交修改；HEAD 不是本批源码或交付凭据。

本批承接 [正常管理后端](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-06-provider-channel-expansion-freebuff-management.md)，只完成前端登录、重授权交互和原生批准链接准入的定向离线闭环。**不是 FreeBuff 整卡、真实管理 TCP/SQLite、真实平台或 P50 完成**。新增正常模板累计仍为 16，R06/W04 与总目标继续执行中。

证据根目录：[gateway-freebuff-ui-20261106](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-ui-20261106)。

后续 [真实本地管理子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-06-provider-channel-expansion-freebuff-local-ui.md) 已补正常 main→真实管理 TCP/Local 持久保存与两次同目录 runtime 重建；不追溯改变本文原组件/合成 API 证据的范围，也不表示跨进程、公共业务或真实平台完成。

## 1. 改了哪些内容

- [FreeBuff API](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/apps/desktop/src/api/console/freebuff-auth.ts) 使用独立公开 schema、实际状态与期限边界，复用既有有界管理传输。create 带原 provider/revision/可选稳定 credential ID；poll/import/cancel 的 body 只有原 revision，GET 不带 Grant 或秘密。回包继续检查 provider/revision/session/目标账号和成功 receipt。
- [专用 hook](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/apps/desktop/src/features/console/useFreebuffLogin.ts) 仅复用有界会话所有权；策略要求显式 poll、禁止 callback completion。内部共用模块名含 OAuth 不改变 FreeBuff 的产品语义：这是 browser-assisted session，不是标准 OAuth 或平台推理 API Key。
- [登录面板](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/apps/desktop/src/features/console/FreebuffLoginPanel.tsx) 提供“通过 FreeBuff 登录 → 打开/复制批准链接 → 检查授权 → 明确保存到凭证池”。定时器只 GET 管理状态，不自动批准轮询或保存；前端不接收 authToken/fingerprint receipt，不生成 agentId 或业务资格。
- [正常全局弹窗](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/apps/desktop/src/features/console/ConsoleGlobalDialogs.tsx) 按精确 `preset: freebuff` 接入，改名 provider 仍可使用，其他产品不被名称误识别。新增与稳定 ID 重授权共用正常管理 Grant 和保存后 refresh，不把结果喂回普通手工保存表单。
- [账号弹窗](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/apps/desktop/src/features/console/useCredentialDialog.ts) 只对显式提供录入切换的 session 产品开放批准面板；普通 Grok/session 手工导入行为不变。“现有会话材料”入口保留 token+agentId 的产品专用导入；普通 metadata/Keep 编辑不强迫重新授权，未保存表单修改会阻止直接重授权。
- [批准链接组件](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/apps/desktop/src/features/console/ChatgptAuthorizationLink.tsx) 与 [原生 launcher](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/apps/desktop/src-tauri/src/connection/authorization_browser.rs) 都只接受精确 `https://freebuff.com`、443、无 userinfo/fragment、长度有界的链接。允许服务端返回的批准路径，与后端合同一致；不开放 Codebuff、子域或任意外部 URL。原生继续单 argv、单受跟踪 worker、10 秒预算、退出取消/join，不实际启动平台登录。

复用现有 `@neuro/ui`、主题及弹窗控件，没有新 CSS、主题重做、依赖升级或平行会话框架。

## 2. 保存未确认与生命周期

每个管理身份/产品拥有独立 operations Map；同一个 Console 内关闭重开可续查已经接纳或响应不明的保存。管理 token/client 变化隔离记录，不将旧 owner 的 receipt 借给新 owner。

- Ready 仅代表服务端已取得材料，仍需明确保存；没有自动导入或普通表单代存。
- 保存响应丢失、`save_unconfirmed` 都锁定重新创建、重复保存和录入模式；关闭允许，已接纳保存不发送 cancel 冒充回滚。
- 新 Grant 只能续接捕获的原动作/原 revision，revision 变化撤销待执行动作；已接纳保存仍可 GET 原会话，不自动改绑新版本。
- 未保存活动会话关闭会取消；取消、过期和失败有终态显示。状态 GET 连续失败五次后暂停，用户只恢复 GET，不重发保存。
- 会话保存前消失允许重新授权；保存接纳后回执消失不证明提交失败，保留防重复提交栅栏。
- 浏览器实查发现未知保存仍显示旧“尚未保存”并重复提示，已通过真实红测修补：只显示一条准确的未确认状态，不猜成功或明确失败。

以上重开状态只在同一个存活 Console 组件/管理身份内；不是浏览器刷新、跨标签、跨应用或 Gateway 进程重启后的完整 reconciliation。

## 3. 实际验证

| 检查 | 结果与范围 |
| --- | --- |
| 新增前端定向测试 | 最终 **82/82 passed**：API 29、面板 10、生命周期 16、链接 17、正常全局弹窗 10；[最终回执](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-ui-20261106/focused-final-receipt.json) |
| 相邻前端组合 | **335/335 passed**，覆盖 Accio/Grok/Kiro/Vertex/ChatGPT/Antigravity、产品材料与普通账号编辑；含上述全局 FreeBuff 10 项，不累加冒充不同用例。[回执](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-ui-20261106/adjacent-v1-receipt.json) |
| 联合去重口径 | 以文件+完整测试名去重为 **385 个不同前端测试名**；既有参数测试存在同名，原始执行实例不等同于不同测试名。原始 JSON 完整保留 |
| 原生 launcher | **8/8 passed**，其中 1 项新增 FreeBuff origin/单 argv 回归；原有 endpoint/重入/shutdown/子进程失败/超时 kill+reap 全保留。[回执](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-ui-20261106/native-v1-receipt.json) |
| 类型与构建 | 最终 TypeScript `tsc --noEmit`、真实 `src/main.tsx` 与组件夹具的隔离 Rsbuild 构建通过；不调用仓库 live dist publisher/清理 hook。[类型](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-ui-20261106/typecheck-final-receipt.json)、[构建](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-ui-20261106/web-build-final-receipt.json) |
| 浏览器交互 | 最终构建的 1280×720 与 390×844 CSS 像素检查通过；初始焦点、无横向溢出、批准/显式检查、未知保存关闭重开/Escape、成功 receipt 才 refresh、稳定 ID 重授权、本地修改门禁、取消与手工入口；无 error/warn。[记录](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-ui-20261106/browser-final.json) |
| 静态门禁 | Rustfmt、checker **31/31**、ratchet **4570** 输入通过；治理源码没有 >700 的违规，无 baseline/exception 改动。初次 wrapper 备份错报告路径，最终审计捕获并按紧邻前批已验证副本恢复，限制见 3.1。[门禁](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-ui-20261106/gates.json)、[实际 ratchet 输出](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-ui-20261106/ratchet-report-actual.json) |
| 来源/大小/编码 | 各测试、类型及构建阶段分别冻结 939 项选定输入且无阶段内漂移；15 项受影响源码，8 项新增，最大 369 有效行，新增最大 104。UTF-8 无 BOM、文档本地链接与 scoped diff 检查见最终 closeout |

原生结果之后只改变前端状态文案与相应测试，原生输入未变化；相邻回归后仅修 FreeBuff 自己的未知保存提示，最终 82 项已复跑覆盖真实全局入口，不无故重跑未变的相邻实现。构建不等同于 EXE 出包；浏览器使用真实产品组件和合成 GatewayApiClient，**管理响应及保存确认是模拟的，没有真实管理 TCP/SQLite**。浏览器观察中一次 import、确认后一次 refresh、普通手工保存零次；定时 GET 不产生自动 POST。

### 3.1 保留的失败，不报全仓绿

- `red-entry`：原正常全局弹窗没有“通过 FreeBuff 登录”按钮，真实行为红测，后续转绿。
- `globals-v1`：9 passed / 1 failed，测试错误使用“手动模型补充”作为 label；实查控件为“手动模型列表”，仅修夹具后最终全通过，未改生产契约迎合测试。
- `red-unknown-copy`：9 passed / 1 failed，真实复现浏览器发现的误导状态，修补后最终面板 10/10。
- 一次临时计数脚本以 `C:/...` 直接做 Node ESM import，返回 `ERR_UNSUPPORTED_ESM_URL_SCHEME`；修为合法 `file:///C:/...` 后计数通过。它不是源码编译失败，三个官方静态门禁此前已分别 exit 0；不抹除外层失败。
- 初次静态 wrapper 误将 `scripts/artifacts/effective-code-lines.json` 当成 checker 的实际输出；最终审计因计数不一致失败，实查 resolver 的 root 后确认真正输出为仓库根 `artifacts/effective-code-lines.json`。实际 4570/无违规报告已另行归档，写回前精确核对本次生成报告 hash，再恢复紧邻 FreeBuff 后端子批已验证的报告字节。**本批没有在运行前直接快照实际 root 报告，不能声称有这种证据**；恢复来源、hash 和该限制见 [report-recovery.json](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-ui-20261106/report-recovery.json)。未影响源码、凭据或测试结果，也不通过改 baseline 凑绿。
- 前序协议子批的扩大 Python scanner 既存失败未重跑/宣称清除；本批也未执行全仓/全 feature 矩阵或付费上游回归。

## 4. 安全、资源与范围审查

UI 只发送产品所需公开绑定和显式管理 mutation；设备秘密仍由服务端拥有。链接在 UI 和原生两个出站边界校验；未知错误只呈现共享 hook 的固定文本，不显示上游秘密或原始报错。复用 AbortController、30 秒请求预算、五次失败暂停、16 项未知保存上限和 cleanup，不新增每账号无限 task、重试或缓存。操作状态保留只含脱敏 receipt，不写 localStorage。

差异与最终哈希见 [closeout.json](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-ui-20261106/closeout.json)。原有 7 个文件的 before 字节、15 文件行数与逐项原始差异均保留。本批没有修改共享 `useManagedOAuth` 的实现，没有借此重构其他认证产品。

## 5. 下一批的真实未完链

1. 正常 UI → 新鲜真实管理 TCP → SecretPatch/CAS → SQLite 持久保存 → runtime 重建，使用合成 FreeBuff upstream，不重复实现已验协议/后端。
2. typed receipt 与旧业务链的实际兼容、完整 agent map、session/run lease、取消/退出、精确账号 probe 和公共推理/费用/权益。
3. FreeBuff-specific Server/Worker/PG/Redis、跨页面/进程未知保存恢复及真实进程退出/重启；同组件 Map 不是其替代。
4. 其余 R01–R16、OAuth 兼容恢复、Kiro 新模板及 Qwen 受限决策仍保留。P50 全范围无凭据集成/devtest、P60 集中录制、P70 真实逐渠道验收尚未完成。

无真实 `.ng`/历史凭据读取，无真实 FreeBuff/付费请求，无容器操作、Git 提交推送、在线替换或新 EXE。仅新建回环静态夹具服务，结束时按 PID/命令行身份停止并关闭临时浏览器页；在线 Gateway PID 23980、原启动时间不变。P34 单容器许可已消费，历史回放未再次使用。

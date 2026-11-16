# FreeBuff：本地注册状态、显式重新注册与正常编辑 UI

日期：2026-11-06。工作区 `C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`，`main` / HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`。保留 inherited dirty edits，HEAD 不代表本批来源。承接[本地设备持久 owner](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-06-provider-channel-expansion-freebuff-device-runtime.md)，不表示 R06/W04、整卡 offline_ready、P50/P60/P70 或总目标完成。

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-device-ui-20261106`。最终以该目录 `closeout.json`、`audit.json`、冻结输入表与失败日志为准。本批没有真实平台认证、生成、付费请求或真实凭据，也不使用 Redis。

## 1. 从本地状态到显式动作

新增正常管理 `POST /v1/internal/gateway/console/freebuff-device-registration/status`，复用目录五字段及正常管理身份、Secret Grant/Origin、产品受众和前后 revision fence。只读本地 exact account/origin/client/token scope；不获取 discovery slot、不 GET/POST、不生成安装 key。absence、合法 stored 与 unconfirmed 都可在上游目录不可用时观察；坏 key/receipt 继续 fail closed。

stored 响应新增非秘密本地 `recordId`，不是上游 keyId。原注册 API 新增与 `retryUnconfirmedAttemptId` 互斥的 `replaceStoredRecordId`，确认必须精确匹配当前状态；网络前和安装锁/SQLite writer transaction 内分别核验。普通重复注册遇到 stored/unconfirmed 都直接回本地状态；陈旧确认在 GET 前拒绝，不再让 pending reconciliation 依赖目录可用性。bootstrap GET 失败不创建 replacement marker、不消费确认。

## 2. 保留已知 receipt，不将它伪装成可用回退

additive schema-1 表 `local_freebuff_registration_replacements` 为每个已存在 binding 保留最多一个待确认 attempt，仍受4096绑定上限和固定安装 OS lock 约束。显式 replace 不清除或改写主表旧 sealed receipt；准备发送前提交新的 attempt，未知重投先轮换其精确 ID。原安装 key、账号 token 和 AAD 不变，不发明 key rotation 或服务端幂等合同。

成功后先严格验证新 capability，再在同一 SQLite writer transaction 更新主记录和删除 exact overlay；更新、删除或 commit 任一步失败不报告成功，旧 receipt 与 marker 保留。replacement 未确认时，正常目录读取返回 `freebuff_device_registration_unconfirmed`，不自动使用旧签名，也不 unsigned fallback。初次注册的 NULL marker 与旧数据库读取保持兼容。启动 sealed-owner 计数包含 overlay，缺 seal 不能换 key。

## 3. 正常账号编辑入口

沿现有 `ConsoleGlobalDialogs → CredentialDialog → FreebuffCatalogPanel`，新增紧凑设备工具，复用同一显式 business origin 与 CLI/Desktop 选择，初始都为空。打开不检查或注册；先点“检查设备”，再按真实本地状态显示注册、再次注册或重新注册。

所有发送经共享 `ConfirmDialog`，确认包含账号、业务站点和 client；未知结果明确提示可能已经成功、不保证幂等。已有记录重注册说明保留旧 receipt 但不自动回退。取消默认焦点、Escape/焦点恢复、窄屏换行与已有弹窗滚动沿 shared Neuro primitives；没有新主题、颜色或解释性面板。

确认快照绑定当前 response 对象与完整 API/管理身份/Grant/账号/revision/disabled/目标 scope。dirty、autosave、身份变化和卸载清结果/确认，abort 并丢弃迟到响应；同步 pending guard 防双击。写请求未取得确认或停止等待后不保留可重复发送的动作，必须再显式检查本地状态。关闭重开不自动 POST。设备等待期间目录与目标控件禁用，注册开始清旧目录展示；不修改手动模型、路由、token 或业务目标默认配置。

## 4. 已执行的验证与边界

| 检查 | 已核结果 |
| --- | --- |
| 设备定向 Rust | 19/19，包含9项新增状态/替换/管理用例 |
| 正常 FreeBuff 管理路由 | 26/26，production router oneshot + 真实 LocalRuntime/SQLite/seal + 随机回环上游 |
| 全部 LocalRuntime | 48/48，包含旧 access-key 与已有 schema/seal 行为 |
| 相邻 catalog/device 协议 | 24/24 |
| 生产 lib / Rustfmt | `cargo check --locked --no-default-features --features runtime-defaults --jobs 4 --lib` 和官方全 workspace 格式检查通过 |
| 代表性 FreeBuff 关闭组合 | 154/154，包括原注册和新status入口的 compiled-out 拒绝；不以此冒称全 feature 矩阵 |
| 前端相关 | 4文件68/68，其中24项新设备组件测试及1项正常编辑宿主测试；类型检查通过 |
| 前端构建 | 隔离正常 main + 实际编辑 Dialog 宿主构建通过，无 live dist publisher/cleanup |
| 浏览器 | 1280/390 CSS像素，实际产品 `ConsoleGlobalDialogs` + 明示合成 API；首次未知→关闭重开→显式检查→确认重投→stored→确认替换，1次目录/2次状态/3次注册、0次配置保存；20行目录内部滚动、dirty门禁、取消焦点/Escape恢复、宽窄屏无横向溢出 |

Rust 七个最终阶段各3244项选定输入无漂移；前端测试/类型/构建各949项选定输入无漂移。Rust按完整名去重240 passed/0 ignored，其中9新增，含各关闭模块复用的support用例；不是240个新增产品功能，也不累加阶段重复项。清单见 `rust-passed-tests.json`。20项源码（13修改/7新增）最大417有效行，新增最大230；checker31/ratchet4613（14 immutable runtime、governed source >700=0）与报告直接备份/精确恢复见 `gates.json`，未改 baseline/exception。来源/UTF8无BOM、全仓及新文件 whitespace、实际 scoped diff 与168个本地链接见最终 audit。

这不是浏览器→真实管理入站 TCP 的联合验收，不是独立进程/OS重启或跨进程锁竞争、Server、原生 EXE/整包，也不证明真实平台设备、模型、额度或权益有效。库检查不冒充出包。已有两实例同目录恢复与 OS lock 用例也不冒充跨进程测试。

## 5. 失败账本与停止边界

- 入口行为红测2/2失败，实际404分别对照预期403/200；新UI红测因缺“检查设备”失败。
- 首轮 UI 联跑20/24失败：旧只读组件 fixture 没有 `NeuroProvider`，新增真实 ConfirmDialog 需要该宿主；只补测试 wrapper，未放宽生产要求。新设备测试本身通过。
- 首轮 Rust 编译3处 E0277：本仓库 UUID 未启用 serde，夹具改显式 `to_string()`，未升级依赖。第一次类型检查发现 Testing Library ByRole 不支持 `exact` 参数，移除无效测试选项。
- Rust四测试线程18/19，一项既有 post-revision 用例返回409但无预期 marker；同源码按仓库串行测试约定19/19，再由管理26/26覆盖。生产共享 discovery 仅2个槽，不能靠扩大并发或改生产语义“修绿”；未保留当时响应 body，故不把具体竞争原因冒称已证。失败日志完整保留。
- CUA辅助变量 `browser` 尚未绑定导致一次只读脚本中止，绑定当前 IAB 后继续。临时 static helper 的 stdin已关闭，常规 stop不可用；核实本批精确PID/command line后仅停止该 helper，端口无监听。不是正常 join；未操作在线实例。
- 一次性audit误把 `git diff --no-index --check` 的1当作whitespace失败；该模式隐含exit-code，实际stdout/stderr都为空。修正为区分正常内容差异与诊断后再核通过，没有修改源码来消除差异。旧最小ChatGPT-only的17个Gemini关线编译错误未在本批重跑，不动S06，也不报全feature矩阵绿。

本批后继续持久默认业务 origin/client、catalog 发布、session/completion 最终签名、显式 admission/reuse/费用确认、typed agent/lease/probe/公共推理与取消；Server/跨页面进程/原生包和其余原始渠道另验。模板仍16、无新增整卡 offline_ready，P50/P60/P70与总目标保持未完。未提交/推送、部署或出包；P34 Redis授权已消费且不能重用。

# 服务商扩展：Full Vertex 真实本地管理与 UI 闭环

日期：2026-11-05。状态：R11/W07 的本地合成子链定向通过，不是整卡、P50 或总计划完成。接续 [专用 UI/模板子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-05-provider-channel-expansion-vertex-oauth-ui.md)。

## 本批闭合范围

正常 `src/main.tsx` Web 入口连接生产 `build_app_state`、真实 loopback TCP router 与 Local SQLite；不再以模拟管理保存证明本子链。
合成 OAuth issuer 和模型 server 仅替换最终 transport，不转发外网；固定 Google 产品、target、Bearer/quota、PKCE、Secret Grant、SecretPatch 与原 revision/CAS 均经过真实生产路径。
独立 Node helper 只提供隔离静态资源和透明转发，保留浏览器 Host/Origin/auth/body，不制造管理响应、grant 或保存结果。不是已打包 EXE 的静态托管/原生桌面验收。

1. 正常选择 `vertex-oauth` 模板，显式 `project-a/global` 创建无凭据、无模型的空池。
2. 专用面板输入合成 client/可选 secret/`quota-a`，创建真实会话；不点击 Google 链接，以手动合成 callback 经生产 complete 交换，Ready 后明确保存。
3. 正常账号编辑补 `gemini-contract` 和 `gemini-metadata`，保留原 access/refresh/client secret。
4. 正常管理创建普通 message-prepaid Key，公共 Chat JSON 成功，SQLite 余额 `4→3`。
5. 由测试控制启动原生产 refresh loop，保留原 owner/join/cadence，仅将合成长期材料的 due margin 调至7200秒；实际执行一次 refresh、持久发布与 rotation，不新增 scheduler。
6. 同目录、同端口停止并重建 AppState/runtime，保留 revision、完整账号和余额；公共 Chat 聚合 SSE 成功且有 `[DONE]`，余额 `3→2`。
7. 正常面板按同一稳定 credential ID 重授权，改用不带 secret 的合成 public client，固定原 project/location/quota，保留手工模型。
8. 再次同目录/端口 runtime 重建，旧 client secret 确认清除；公共 Chat JSON 使用新 access 成功，余额 `2→1`。

真实浏览器完成上述模板、管理登录、授权/手动回调、明确 import、模型编辑和稳定 ID 重授权；普通 Key 与公共请求由独立 loopback control client 执行，未冒称点击了尚无的 UI 控件。
浏览器最终显示1个账号、3次调用、`3/3`成功，重建前后 ID 一致。
这些是同测试进程 runtime 重建，不是进程/OS 重启、真实 Google 登录/IAM/模型资格或真实账单。

## 实际发现并修复

### Full Vertex SSE 默认栈溢出

`local-tcp-v3.log` 保留 `STATUS_STACK_OVERFLOW`。只读失败现场确认刷新已经发布，第二条公共请求仍在途；未重放原 transaction 或清理失败数据库。
Full Vertex 与 Express 均先聚合响应，但前者进入第二层大型多服务商 streaming dispatcher。
在 [streaming owner](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/client/streaming.rs)让 typed Full Vertex 复用现有同任务 buffered→SSE 分支。
不增大 `RUST_MIN_STACK`、不另起 task/thread、不改取消所有权，也不接管共享 Gemini 内部；`local-tcp-v4` 同一测试默认栈通过。

### 显式 OAuth 身份在展示投影中丢失

真实浏览器发现卡片及正常账号下拉将 `vertex-oauth` 显示为 `vertex-api`，OAuth-only 筛选会误排除。
[access classifier](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/apps/desktop/src/features/console/providerAccessKind.ts)现在复用 typed Full Vertex 判断，要求合法 preset alias 与 `auth_mode=vertex_user_oauth`，不按可改 ID/label 猜身份。
卡片 section、两条已有 document catalog、表格 row 和账号下拉透传公开 auth mode。
正常 active-summary 不携带该字段，故仅在原有 revision 匹配门之后从同版本 active document 补投影；不同版本 summary 不借草稿推断，不改源 document、凭据或 ID。
旧 generic Vertex 保持原分类，Express 仍为 API。

相关通用名称测试另暴露既有 `accio`、`freebuff` 分类遗漏和 Canvas 被追加 `-web`；按已有 OAuth/session preset 补分类，保留 `gemini-canvas` 精确产品名，不迁移存储或改写自定义池名。
新增红测覆盖四个 Full Vertex aliases、改名/误命名、卡片/表格/搜索、正常下拉、同版本与旧版本 summary；原通用模板测试夹具补真实 typed Vertex 元数据，不放宽生产身份校验。

## 运行证据

全部临时脚本、合成材料、SQLite、构建和日志位于：
`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-vertex-oauth-local-ui-20261105/`。

| 检查 | 实际结果与边界 |
| --- | --- |
| 定向本地 TCP | `local-tcp-v4.json/.log`：1 passed / 1 ignored；默认栈，2983项选定 Rust 输入无漂移 |
| 受影响 Rust | `vertex-affected.json/.log`：170 passed / 0 failed / 2 ignored；`runtime-defaults`，不累计本地测试重跑，不报全仓或完整feature矩阵绿 |
| 生产非测试编译 | `production-check.json/.log`：`cargo check --locked --no-default-features --features runtime-defaults --lib --jobs 4` exit0，2983项选定输入无漂移 |
| 显式浏览器 host | `browser-host.json/.log`：仅显式执行 ignored fixture，1 passed；完成时2983项输入无漂移，和上述普通忽略项分列 |
| 实际业务 | `before-result.json`、`after_refresh-result.json`、`after_reauth-result.json`：3次200，余额4→3→2→1；管理/公共响应不暴露合成access/refresh/secret |
| 停止后SQLite只读核对 | `completed-state-inspection.json`：3条audit全部`completed`、inflight=0、消息余额remaining=1；不清理或重写数据库 |
| 持久重建与最终 wire | `browser-runtime/completed.json`：两次重建同一ID/target/revision、3次精确模型请求、2次code grant+1次refresh；第二次无client secret，最终assert核Bearer/quota/路径/body |
| 前端最终组合 | `affected-v5.json`：18文件187 passed / 0 failed / 0 skipped；含本批10项新增测试，不累计中间绿色重跑 |
| 类型/最终Web构建 | `typecheck-v5-receipt.json`、`build-web-v5-receipt.json`：exit0；正常main入口、`cleanDistPath=false`、只输出到linshi；900项选定UI输入含installed UI包，最终来源审计无漂移 |
| Rust格式/规模/编码 | `source-audit-final.json`：20个源码/测试文件均严格UTF-8无BOM、scoped whitespace无诊断、全部<500有效行；最大489，无新增例外或baseline改写 |
| checker/ratchet | `checker-final`：31 passed；最终ratchet通过、governed source >700=0；immutable runtime单列，不把其行数当新增违规或篡改baseline |
| 差异 | 本批before副本、`scope.diff`与最终文档回执可复核；最终`git diff --check`通过，不撤销其他工作区改动 |

新增 Rust 普通测试与显式浏览器 fixture 各1个；前端新增10项。历史子批的457项、556项等不与本批计数相加。
前端没有配置 formatter 命令，本批保持既有样式，不安装格式器或伪报执行。Rust相关文件使用官方rustfmt检查。

### 保留的失败与来源限制

- `local-tcp-v1`：测试将 `url::Url` 直接传给不支持该 trait 的 rquest，修夹具为 `callback.as_str()`；不是生产认证缺陷。
- `local-tcp-v2`：测试 deny-proxy 误截获本应直连的合成 issuer，精确endpoint断言失败/超时；只对两个明确 loopback peers设no-proxy，漏掉HTTPS seam仍会被拒绝。
- `local-tcp-v3`：真实默认栈溢出已由上述生产最小分支修复；旧失败SQLite/在途audit和receipt保留，未重复提交。
- UI首轮新测试有一项使用错误Map/telemetry夹具；修正后`access-red-v2.json`六项均为真实分类断言失败。
  `access-console-catalog-red.json`及`access-active-summary-red.json`分别证明两条catalog和active-summary入口的字段丢失，再定向修复。
- `access-green.json`的24 passed/1 failed暴露通用模板中的Accio/FreeBuff/Canvas既有缺口；补独立红测后修classifier，不删业务断言。
- 初版Web输入因外部package/lock路径变动漂移；原回执`source-audit-v4-ui-drift.json`保留，installed UI包22文件比较未变。
  未改依赖、重装、升级或撤销外部工作；后续v5另立900输入快照。并未冻结整个node_modules或验证clean install。
- 真实业务闭环使用v2正常main构建；修正后的卡片/筛选/下拉浏览器证据为v3的`ui-oauth-filter-v3.png`及AX文件。
  v3之后两条catalog/active-summary的最终补齐有v5类型/测试/构建证据，但未再次重走浏览器；不能声称v3截图与最终v5全构建逐文件同hash。
- 浏览器日志中的`MaxListenersExceededWarning`和`ObjectMultiplex`来源均为既有`chrome-extension://.../contentscript.js`，未伪归因Gateway或改扩展。

### 清理

host通过文件finish指令正常join原OAuth manager、refresh task、router、SQLite及合成peers。
两个静态helper的stdin因非TTY不可写；核PID49876/34992及完整专用脚本命令后仅停止这两个Node进程。
`helper-cleanup.json`确认两个helper和host PID30308均不在，49463/59970/56074无listener；专用tab已关闭、未设置viewport override。
未删除测试现场、读取真实`.ng`/ADC/历史凭据、调用真实Google、创建容器、修改在线实例、出包、部署、提交或推送。
P34 Redis授权对应容器此前已消费并清理，本批无需Redis，未复用授权。

## 下一接力点

1. Full Vertex 独立公共 Chat/Messages/Responses JSON/聚合SSE的完整pipeline，Token Key/usage/结算、HTTP/坏流/取消退款；本批只验证正常Chat文本与message-prepaid子链。
2. Full Vertex完整响应终态准入/decoder、工具、签名及各模态；不借Express测试结果或先聚合再桥接的SSE声称真实token流。
3. Vertex-specific Server/Worker实际Redis publication/restart/竞争、PG/Hosted、跨进程/断电/failover receipt、旧数据/部署兼容及原生EXE；如需新测试容器，重新说明并申请一次性授权。
4. 单独官方线的共享Gemini Web15项缺符号旧失败未由本批修复或重跑；完整feature矩阵继续保留。
5. Express剩余decoder、其他原定渠道、Luma Agents/Web/媒体及P50无凭据集成/devtest；P60/P70仍在功能完成后集中录制/真实验收，不要求现在逐项提供凭据。

累计新增模板仍15，不增整卡`offline_ready`，不标R11/W07、P50或总目标完成。

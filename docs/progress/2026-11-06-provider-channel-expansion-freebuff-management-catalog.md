# FreeBuff：正常账号管理的只读模型目录

日期：2026-11-06；仓库 `C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`，`main` / HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`。开始时 tracked dirty 551 项，原改动保留；HEAD 不是这些未提交实现的交付凭据。

本批承接 [catalog/device 独立协议](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-06-provider-channel-expansion-freebuff-catalog-device.md)，只接正常管理 API 和账号编辑 UI 的**显式、只读目录**。不是 R06/W04 整卡、P50 或总目标完成。模板仍 16，不新增整卡 `offline_ready`；P50/P60/P70 与其他渠道仍未完成。

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-management-catalog-20261106`。最终状态见 [closeout.json](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-management-catalog-20261106/closeout.json)；本子链通过不代表整卡可用。

## 1. 实现边界

- 新增正常 `POST /v1/internal/gateway/console/freebuff-model-catalog`，要求精确 provider/credential、expected revision、正常管理 token 与 Secret Grant；不接受调用者 Key、任意目标或额外字段。
- 业务 origin 只允许 `https://codebuff.com` / `https://www.codebuff.com`，client 只允许 CLI/Desktop。没有默认、GUI 推导或登录 host 推导；UI 明确告知此次读取会向所选业务站点发送该账号令牌。选择仅用于本次读取，不反写登录 base、Keep audience 或持久配置。
- 服务端取同一 snapshot 的精确 credential 和最终 compiled payload，执行既有 `freebuff_session::validate_target`；不按可变名称猜产品、不借池级/其他账号秘密，disabled 的显式账号仍可由管理员读取。
- 网络前后核 Grant/revision，复用全局有界 discovery slot。每次独立 `CatalogSession` 只调用 unsigned Bearer GET，无 key 生成、设备注册、admission、自动重试、redirect 或持久 cache；总网络/decoded body 预算十秒，沿用 1 MiB/512 rows/64 badges 限制。
- 返回 no-store display DTO：稳定 key、名称、短文案、warning、badges、access/dataUse/premium/multimodal 与精确账号/目标/revision。无 handle/fetchId、plans URL、token/fingerprint 字段；locked 可见，未来 row 按 server issuedAt 过滤。目录可见不证明实际模型调用或权益。
- UI 接现有正常账号编辑 Dialog 的 stored-account tools；仅 edit、open 且非重新授权页挂载，使用 initialValue 的原始账号 ID。本地表单修改、dirty、autosavePending、锁定和 active revision 不一致均阻断。打开不读，用户显式选择并点击才 POST。
- API client、管理 token/Grant、账号、revision、目标、client 或 disabled 变化以及关闭/unmount 都取消旧请求，scope 和 response binding 双验阻止迟到结果；显式取消不自动重试。不调用普通 discovery/apply，不修改手动模型、`supported_models` 或 `model_routes`。

## 2. 源码与责任

后端：[handler](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/http/routes/internal_console/freebuff_catalog.rs)、[router 测试](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/http/routes/internal_console/freebuff_catalog_tests.rs)、[回环 fixture](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/http/routes/internal_console/freebuff_catalog_fixture.rs)。`internal_console.rs` 和 `management_config.rs` 仅接线；协议 `catalog.rs` 仅增加 cfg(test)+feature 的受限回环 scope，生产无 URL override。

前端：[panel](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/apps/desktop/src/features/console/FreebuffCatalogPanel.tsx)、[请求生命周期](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/apps/desktop/src/features/console/useFreebuffCatalog.ts)、[strict display DTO](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/apps/desktop/src/features/console/freebuffCatalog.ts)。`CredentialDialog`/types 与 `ConsoleGlobalDialogs` 仅增加专属工具接线，不新增账号卡 action 或平行全局 context。样式复用 Neuro primitives，仅加目录内部滚动和长文本折行。

## 3. 验证及失败记录

- 首轮后端行为红测 **0/2**：路由缺失，实际 404 对照预期 403/409；UI 行为红测 **0/1**：缺少“读取目录”按钮。不是用编译错误代替行为红测。
- 后端接线后首轮 **7/1**；唯一失败为测试 fixture 使用不存在的 `openai-compatible` preset，配置编译拒绝。替换为实际 `xai-api` 后 **8/8**，相邻 Qwen **5/5**、生产库检查通过。只修 fixture，不放宽产品校验。
- 交叉只读审查发现“无设备签名”断言误用了 `x-freebuff-device-key-id`；生产本来调用 `refresh(None)`，没有签名。测试改为检查实际三个 `x-freebuff-device-key/ts/sig` header 后，最终再次 **8/8**、相邻 Qwen **5/5**、生产库检查均通过；只移除 FreeBuff、保留其他 runtime features 的关闭组合 **1/1** 验证网络前拒绝。最终去重 **14 个不同 Rust 测试**，其中本批新增 9 个，四阶段各 **3232 项选定输入无漂移**。不重跑或冒称修复前批最小 ChatGPT-only 组合的既有 Gemini media 关线错误，不报全 feature 矩阵或全仓绿。
- UI 最终相关 **77/77，7 个实际测试文件**，含本批新增 33 项；独立类型检查通过。测试覆盖两类只读 DTO 约束、缺权限/dirty/版本、账号/管理身份/Grant/API client 变化、取消/unmount/迟到响应，以及正常全局宿主不提交配置。首轮 23/3 缺 NeuroProvider、随后 25/1 错误标签均为 fixture 问题，修正后失败日志保留。
- 前端最终测试、类型检查、正常 main+真实产品 Dialog fixture 的隔离 Rsbuild 构建各 **945 项选定输入无漂移**。构建直接输出本批 linshi，不运行 live dist publisher/cleanup、不替换运行页面。
- 合成 API 的真实产品全局宿主浏览器在 1280×900、390×844 检查通过：20 行长文本目录内部滚动、文档/弹窗无横向溢出，dirty 修改清除结果并禁用读取，Escape 关闭/焦点回到打开按钮，观察到 reads=1、saves=0。不是正常 main→真实管理 TCP、Local SQLite、原生 EXE 或真实平台端到端。
- Rustfmt、checker **31/31**、最终 ratchet **4597 文件**通过；14 immutable runtime 单列、governed source >700=0。16 项源码（9 新增/7 修改）最大415有效行，新增最大250；所有相关源码/4份文档 UTF-8 无 BOM、来源/链接/scoped差异及 `git diff --check` 通过。实际 root 报告直接备份后归档并精确恢复，SHA256 `dc7e58afefbfaa02f4d257ef099af01e91b8fb61a444b517863b3f816bdb3550`，不改 baseline/exception。

三项真实回环请求覆盖两个显式 origin/client 的成功读取和上游 401；请求只 GET 精确目录 path/账号 Bearer，响应无短效字段，document/revision 保持完全一致。私有 fetch seam 只用于成功后撤销 Grant/修改 revision，以及 busy/caller Future Drop 释放 slot；这些不是管理 HTTP 客户端断开后服务端一定取消的证明。

浏览器夹具第一版漏传关闭其他 Dialog 的控制字段，误显示添加分组；补齐 fixture 后重建，未因此修改产品。静态 helper 的 stdin 已关闭，不能以 `write_stdin` 正常停止；已核实 PID/命令行为本批脚本后仅停止该 helper，端口无监听。不是 graceful helper shutdown；回环 peer 正常 close 则有三秒有界 join。没有停止既有服务。

## 4. 接力与仍未完成

1. 安装级 Ed25519 私钥与 per-account registration 的持久 owner 仍缺；现有 fingerprint 不是设备私钥，不用临时 JSON、`extra_body` 或伪装 access-key 行保存 PKCS#8。Local SQLite/seal 机制可参考，但不是现成通用 vault，也不代表 Server 多实例已支持。
2. 本次只读选择不等于 durable 业务 origin/client 配置或 catalog 发布。公共业务尚不能使用此 DTO 的稳定 key自动选择/发送短效 handle；需要保持 key/handle/fetchId 分离与有明确生命周期的绑定。
3. session/completion 最终签名、显式 admission/reuse/逐次消费确认、完整 typed agent/lease/probe/公共 JSON/SSE/取消、Server/跨页面/进程和原生包仍未闭合。不要把旧自动 POST、默认 agent、零 credits 或目录可见当作这些已完成。
4. 不重复前批已验登录/保存/基本重建；其他原定渠道、P50 无凭据集成/devtest、P60 集中录制、P70 真实逐渠道模型/权益/费用继续。

本批未读真实 `.ng`/历史凭据、未发真实认证/生成/付费请求、未操作容器或在线实例、未出包/提交/推送。公开协议依据复用前批已固定并核实的官方来源，本批没有重新联网复验平台可用性。P34 容器授权已消费并清理，不复用历史问卷许可。总目标保持 active。

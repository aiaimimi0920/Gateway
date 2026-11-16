# 服务商分渠道扩展：P36 Grok 手动 OAuth UI 与 expiry 准入

> 2026-11-03；本批实现和定向验证完成，扩大前端回归另有一项标签期望不匹配失败。R05/W03、P50 与总目标仍执行中，不能以本批代替全部服务商完成。
> 工作区 `C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`，`main`，HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`。保护前序与并行脏改，未提交、推送、出包或替换实例。

## 1. 本批交付

- 新增第十个计划内前端模板 `grok-oauth`，固定 Grok Build base、空模型/空池；API 继续复用 `xai-api` 和原 `xAI-api` 默认 ID，Web 保持独立。没有搬迁材料、合并额度、自动探测/生成路由或适配完成蓝标。
- 正常 `ConsoleGlobalDialogs` 按精确 preset 接线；dirty/locked/非 active revision 草稿禁止开始或继续变更。显式 Client ID/scopes/redirect，consent selection 与 required principal 分开，不借官方 CLI 身份。只提供手动 callback，不宣称有 listener。
- 编辑账号中增加明确的重新授权动作，传原稳定 credential ID；未保存表单修改时禁止切换，原 metadata/模型编辑和 keep 入口保留。
- 独立 API schema 校验 provider/revision/session/目标 ID，成功必须具备 credential ID 与 saved revision。每个 mutation 都传 Secret Grant，GET 仅管理身份；授权链接严格限定 auth.x.ai 的精确 authorize 路径。
- 每请求 30 秒 abort；轮询串行、五次连续失败后暂停，不叠加重试器。管理身份/产品 generation 与 mutation version 分别阻止迟到写回和旧读覆盖新动作。revision 改变不自动 rebase，也不丢弃已接纳保存的只读观察。

完整交互合同见 [Grok 管理登录合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/provider-grok-oauth-login.md)。没有改变后端 P35 的 SecretPatch、原 revision/CAS、受跟踪保存任务或成功判断。

## 2. 保存未知、关闭重开与审查修补

操作记录由正常 Console 的 `client + managementToken` owner 持有，最多 16 条，只存非秘密会话 view、未知/明确拒绝标志，不存 callback/token，不写 localStorage。import 发送前先记录；关闭弹窗或返回编辑不会重新开放未知保存的 create/import，重开继续读取同一会话。成功观察只消费一次记录/刷新一次。

明确接纳前拒绝（`grok_login_save_rejected` 等）不伪装为网络结果未知，Ready receipt 仍可由用户明确再次操作；不是自动重发，也不取得最新 revision 继续。网络丢失、超时/解析失败及 `save_unconfirmed` 则不重新提交。

只读交叉审查发现并实测修复两轮问题：关闭重开会丢弃唯一观察者；旧 import HTTP 回包可在新面板已观察 succeeded/消费记录后将其复活。最终迟到写回/拒绝只条件更新仍属于原 session 的记录；不能重建已消费记录、覆盖新会话或删除后续操作。迟到的明确拒绝也会由重开观察者读取，而不是永久锁在未知状态。

此记录不跨 Console 卸载、页面刷新、management 身份替换或 Gateway 进程重启。未知结果不按年龄自动抛弃；完整跨进程 reconciliation 仍待实现，不能把此内存子链当作完成。

## 3. P35 legacy expiry 失败关闭

P35 的 `legacy_compiler_rejects_extreme_token_expiry_without_panicking` 在本批先实际复跑仍为 exit 101。原因是 unchecked loader 直接进入 compiler，legacy refresh owner 会截断过大 lifetime；strict validation 则拒绝原始不可表示声明。

将既有 `Instant/SystemTime::checked_add` 准入 helper 移至共享 runtime owner，并在 provider 编译边界检查声明，strict validation 复用同一 helper。不改 storage version，不重写持久配置，不削弱合法 lifetime 的现有运行态截断。新增无 refresh 材料时的 None/0/1/100年/u64::MAX 准入一致性和原值保持测试；未修改原失败断言或增加 skip。

首轮扩大命令因新增测试误用 `pub(crate)` document accessor 而编译失败（`legacy-router-v1.log`），不算生产测试失败；测试改用公开 strict validated document 后，v2 实际执行原失败项和整组合同通过。

## 4. 验证证据

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-provider-expansion-20261103-p36`。

| 范围 | 实际结果 | 日志 |
| --- | --- | --- |
| 旧 expiry 红测 | 原合同真实失败；随后 v2 转绿 | `red-legacy-v1.log` / `legacy-router-v2.log` |
| 前端模板红测 | 缺少 grok-oauth，1 failed / 7 passed | `red-grok-template-v1.log` |
| 首版 UI | 8 tests 中2 failed；1为未知提示残留，1为 mock complete 错返 cancelled 的夹具问题 | `grok-ui-v1.log` |
| 重开/明确拒绝红测 | 2 failed，随后修补并转绿 | `red-grok-reopen-v1.log` |
| 迟到记录复活红测 | 1 failed / 7 passed，随后修补并转绿 | `red-grok-late-record-v1.log` |
| API、状态机、重授权、链接、模板 | 54 passed | `grok-ui-v5.log` |
| 正常全局入口、原 revision、preset、管理身份 owner | 7 passed | `global-ui-v1.log` |
| 三渠道空池与既有材料/路由隔离 | 1 passed | `grok-catalog-v1.log` |
| 相邻前端回归 | 90 passed / 1 failed，见下 | `adjacent-ui-v1.log` |
| Console 文档、Grok 管理、正常事务 integration | 66 + 2 + 12 passed，另1既存 opt-in ignored | `legacy-router-v2.log` |
| routing/config lib | 119 passed | `routing-v1.log` |
| 代表性 feature-disabled 编译 | exit 0，不是全矩阵 | `feature-disabled-v1.log` |
| TypeScript、最终隔离 Web build | exit 0 / exit 0 | `typecheck-final.log` / `safe-web-build-final.log` |
| Rustfmt、checker tests、ratchet、diff | exit 0；checker31/31 | `rustfmt-v1.log` / `checker-tests-v1.log` / `ratchet-final.log` / `diff-check-final.log` |

本批不同 Rust 测试共 **199 passed，另1 ignored**；前端共 **152 passed，另1 failed**。没有把重复运行累计成新增通过数，不报告全仓绿色。

扩大前端的真实失败为 `BrowserConsoleApp.shell.test.tsx:210`：期望按钮名 `Managed OpenAI`，当前 `ProviderLedgerCard` 调用既有 `providerAccessLabel` 得到 `Managed OpenAI-api`。该 owner/并行 UI 契约不在本批写入范围，未修改测试去迎合，也未撤销其后缀展示代码；这项失败保留，尚未关闭。

后端统一 `--locked --jobs 4 --no-default-features --features runtime-defaults`；feature-disabled 使用 `line-gemini-web-reverse --lib`。TEMP/TMP/TMPDIR 指向本批目录，16 MiB 栈仅用于测试进程，不改变生产设置。未启用默认 management-ui 的工作区发布/清理 hook。

## 5. 浏览器与来源边界

使用已安装 Rsbuild API，真实 `src/main.tsx` 和生产组件编译至唯一 `linshi/.../safe-web`，显式禁用清理，不加载原 staged publish 插件，不碰 `dist/web`。这是隔离 Web 构建，不是 devtest/正式 EXE 或发布凭据。

浏览器检查使用同构建的明确标识“合成材料 · P36 UI 隔离验证”组件夹具：真实 CredentialDialog/GrokOAuthPanel、模拟管理 API，无真实上游。验证显式参数→手动 callback→Ready→未确认保存、Escape 关闭、重开不重新提交，及 1280宽/390宽的可读性和内部滚动。截图为 `grok-unconfirmed-reopened.png`、`grok-unconfirmed-narrow.png`；未打开真实授权页。它不能代替完整产品页面到真实后端的 E2E；截图使用 build-v1，后续仅生命周期竞态修正后已另做最终类型/测试/构建。

临时 HTTP server 仅绑定回环随机端口62520；已核对脚本路径后只停止本批 PID21324，监听已退出，未停止其他 Node/服务。创建的浏览器页已关闭，临时视口 override 已还原。

本批22项源码以官方 lexer 测量并记录 SHA-256；主要前后值：config202→202、compile179→186、refresh_state178→183、validation409→404、resource_limits204→235、CredentialDialog473→484、GlobalDialogs293→313、catalog445→452。新 hook165，其余新增文件均在250以内；最大484，无501–700例外，未改baseline/exception。`audit-final.json` 对源码/文档做严格 UTF-8无BOM/尾随空白审计；其 `failures` 仅指来源与文本完整性，已知测试失败单列，不是总成功标志。

## 6. 后续未完成

- Grok 自动 listener/device、跨控制台/进程的未知保存恢复、真实第三方客户端与产品使用边界、完整 Responses/业务保真、跨 revision 可信刷新恢复、永久 Sending 可用性、其他 refresh 入口与 durability 继续。
- 其他原定 Accio/Kiro/Vertex OAuth、Perplexity Web、其余 API/媒体及 P50/P60/P70 继续；本批没有新增完整 `offline_ready` 的平台交付卡。
- 扩大 shell 标签合同失败仍需由对应 UI owner 判断真实契约后处理，不能把它隐藏为“本批所有回归通过”。

本批未读取历史凭据、调用真实平台、录制账号、消费付费额度、创建 Redis 容器、出包、替换在线实例、提交或推送。P34 授权早已消费并精确清理，本批没有复用。

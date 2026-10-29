# 三适配器框架接入前最终复核

## 结论与边界

本轮完成框架收尾的定向复核，修复两处前端编辑缺口，相关门禁通过，可以转入逐模板接入开发。
这不等于所有模板已可用或上游验收已完成；`TEMPLATE_ADAPTATION_RECEIPTS` 仍为空，不标蓝。
保留既有未提交的 UI、凭据池、4300 devtest 隔离与前三轮框架修改，不提交、不推送、不出 EXE。

## 本轮生产修复

1. **合法 camelCase 策略编辑失效**：卡片 reader/writer 原来只处理 snake_case，导致
   `callFlow` 的真实指纹显示为 Auto，编辑旧字段后又被 flow 覆盖；旧 camelCase profile 还可能
   与新写入的 snake_case 字段组成重复 alias。现在读写接受单一合法 alias，保留现有名字，
   同步 flow 与旧 profile，Auto 删除对应旧字段，无 flow 时不合成 flow。
2. **会话指纹可被改成 API 指纹**：原来显式 `session` 被显示成 Auto，卡片可以提交 Claude 等
   API 指纹，生成后端正确拒绝的专用执行器配置。现在显式 session 或明确选择 Basispoints、
   Gemini Web、ChatGPT Web、AI Studio Web 时显示“站点会话”，锁定原有 select，保持单行布局。
   writer 先校验再写入；hook 收到拒绝时保留草稿并报错，不仅依赖 disabled 控件。

同层两种 alias 同时存在、非对象 flow、未知显式指纹也在写入前拒绝，不部分改写，不静默修好
非法策略。生产修改仅位于 requestClientProfile、selector、ledger 显示类型和 policy editor；
不改 CSS，不增加解释行，不引入新的 preset 注册表，不修改协议和专用执行器实现。

`special_adapter=auto` 对尚未逐模板开放的产品继续沿后端 preset/transport owner 验证，不能把
本轮显式 session 的显示保护理解为对全部历史产品做过前端能力枚举或真实接入验收。

## 复现与验证

先新增 5 项失败回归。首次两文件运行结果为 6 failed / 8 passed：5 项复现上述生产缺口；另
1 项是旧测试假定卡片背面立即挂载，其 DOM 查询在已存在的首次翻面懒挂载逻辑下失效。
后者单独修正为首次翻面后查询，并保留后续翻面的节点复用、不可见性和禁用断言，没有为
测试恢复立即挂载，也没有删掉覆盖。补充明确专用适配器的 Auto 指纹保护，最终两文件 15 项通过。

| 检查 | 本轮实际结果 |
| --- | --- |
| 指纹 reader/writer、selector、真实 policy hook | 2 文件，15 项通过 |
| 前端相邻回归 | 9 文件，55 项通过，包含上述 15 项，不重复相加 |
| TypeScript | `npm run typecheck` 通过 |
| 后端三适配器 | `cargo test --locked --lib call_flow --jobs 4 --no-default-features --features runtime-defaults`，34 项通过 |
| 格式 | `cargo fmt --all -- --check` 通过；前端未配置独立 formatter，保留现有写法并检查 diff |
| 有效行 checker | 自测 31 项通过，ratchet 通过；未调整基线或豁免 |
| 编码与差异 | 变更文本 UTF-8 无 BOM、`git diff --check`、依赖锁未变检查通过 |

前端相邻回归覆盖 requestClientProfile、usePilotPolicyEditor、providerCallFlow、
ProviderCatalogDialog（原有与 iteration）、providerCatalogCreation、credentialAuditLedger、
PoolWorkspace.render-budget、BrowserConsoleApp。后端 34 项同时重跑之前的合成策略、随机
loopback 发送、网络前拒绝和 CPA 准入保护；不把 CPA 拒绝测试称为成功发送或真实模板验收。
本轮 Rust 生产源码未修改，未重新执行上一轮的全目标 cargo check 或十组相邻后端套件。
Cargo 仍报告已有 unused/dead-code 等 warnings，不为清理告警进入无关 Gemini lane。

独立后端只读交叉复核未找到确定的新阻断：复核 Redis/Hosted 候选过滤与策略准备顺序、
请求级协议选择、模型负证据、发送前准入和 official 三条发送复用；源码复核不代替运行证据。

## 增量有效行与安全复查

| 本轮文件 | 修改前 → 修改后有效行 |
| --- | --- |
| `requestClientProfile.ts` | 20 → 36 |
| `RequestClientProfileSelect.tsx` | 20 → 23 |
| `accountsLedgerTypes.ts` | 120 → 120 |
| `usePilotPolicyEditor.ts` | 324 → 327 |
| `usePilotPolicyEditor.test.ts` | 90 → 119 |
| `requestClientProfile.test.tsx` | 81 → 133 |

已检查相对本轮前快照的 scoped diff。新增工作为同步、常数规模的字段检查和现有控件状态，
不新增 I/O、权限、凭据读取、秘密日志、进程、监听器、异步任务或全局可变状态；只复制一个
flow 对象，保留独立字段和凭据。显示用 session union 不进入可写 API profile 枚举。

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-call-flow-final/`。
包含 before 快照、`red-ui.log`、`green-ui.log`、`regression-ui.log`、`typecheck.log`、
`backend-call-flow.log`、`production-final.diff`、before/after counts、source-validation、
formatter、checker、ratchet、diff 和依赖锁检查记录。

## 未执行

- 未连接或修改 4200/4300，未读取 `.ng`、真实账号或凭据，未调用付费或真实上游。
- 未启动 PostgreSQL/Redis；Hosted/Redis 测试使用生产函数和合成数据，不是外部服务验收。
- 未构建网页/桌面分发产物、产品 EXE 或安装包，未进行真实 WebView 人工视觉验收。
- 未运行全量业务回归、逐模板浏览器会话验收或正式发布审计；没有新增模板蓝色标记。

下一阶段逐模板校验固定地址、三项有效策略、认证与凭据不变性、请求/响应、流式与错误路由，
取得该模板的真实验收证据后再记录 receipt。框架复核与正式接入的完成状态必须分开报告。

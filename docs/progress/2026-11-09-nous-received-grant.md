# Nous Portal：received successor 与可发布结果分离

此文是 OAuth／订阅优先批次的阶段记录，不代表 Nous 完整接入或全看板完成。
参考来源、独立 client 资格与官方/社区错误分类分歧见
[固定来源合同](../providers/nous-contract-2026-11-08.md)。没有借用 `hermes-cli`，
没有读取 Hermes profile、真实 `.ng` 或账号秘密，也没有真实授权、订阅或推理调用。

## 本子批代码范围

- 新增独立 `nous_portal` 协议 owner：显式 client、device code、首次 interval 等待、
  `slow_down +5s`、服务端 TTL/本地 600s 上限、发送前后取消语义。
- Refresh 只有一次 HTTP 尝试；RT 只在 `x-nous-refresh-token`，form 仅 grant type/client。
  Portal/推理/目录目标固定，禁 redirect、环境默认 headers 和网络下发任意 host。
- 成功 token response 先捕获有界 `ReceivedTokens`，即使 access JWT、scope、expiry、
  token type 或 inference target 不可用，仍保留已收到的 successor。缺失 successor 不回用旧 RT。
- JWT 仅做未验签 hint/可用性检查，不当作管理身份或同账号证明。两个未知 `sub` 不相等；
  使用真实 JWT exp/响应 expires_in 的较小值，无默认一小时/六小时 TTL；恢复不重新计时。
- 目录保留含 `/` 的精确模型 ID；成功空目录与错误分开，不猜价格、能力、权益或联通状态。
- 复用现有 local writer lock、atomic write、Redis CAS 与 family head，新增可选 receipt，
  不另建 refresh 调度器、锁、凭据文件或通用配置写入口。

## Durable 状态合同

`Sending + receipt` 仍是 **Sending**，不是 Result，不可 replay 为可用 token，也不能 Published。
带 receipt 的记录只有后验通过且 access/RT/绝对 expiry 与该 receipt 精确一致后，才允许沿原
Result/Published 流程前进。未带 receipt 的旧 generic OAuth actor/历史记录保持原合同，不能仅凭
endpoint 字符串把既有合法记录判坏；正式 Nous typed worker 接线时还须建立“产品发送必先 reserve、
接收必先持久化 receipt”的前置不变量，本子批未声称已经建立该入口不变量。
收到材料之后即使账号已撤销或替换，仍可保存原消费回执，但不能借此覆盖新账号配置。
回执不可变；冲突结果失败，迟到的相同确认不能把 Result/Published 降级回 Sending。

字段为 additive optional，缺省时不写入 JSON；旧记录、新记录原 family/material ID、locator
和永久 Sending fence 不变。不改 storage version、数据库或既有记录，不增加新 phase。
旧版 `deny_unknown_fields` reader 遇到新 receipt 会拒绝读取并阻止 reserve，不会自动把它当作
缺失记录重新发送；不宣称旧二进制能理解/发布新回执。新 reader 继续读取无 receipt 的旧记录。

此次尚未把 Nous typed context 接入 credential/material。该前置工作完成前，receipt 的
Result 校验要求响应显式给出允许的 inference target；缺失目标只能隔离保留，不能猜回
Subscription。协议 helper 已接受旧 typed context 并验证 old access/client 绑定，供后续贯穿。
Welcome 目标省略字段的旋转应继承 Welcome，不改发到 Subscription。

设备成功响应到达本地 TTL 边界时，仍返回 receipt 并显式标记 `challenge_expired`；管理 owner
后续必须保留接收证据，但过期 challenge 不得作为导入许可。

## 验证与当前状态

临时证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-magpie-expansion-20261108`。

- 已运行 `nous-receipt-red.log`：编译成功，预期 **0 passed / 1 failed**，原 reader 拒绝
  含 successor 的非发布回执。不是编译错误。
- `nous-receipt-lib-r3.log`：协议与 receipt 单元合同 **20 passed / 0 failed**。
- `nous-receipt-integration-r3.log`：真实本地 persistence 的 Nous 合同 **7 passed / 0 failed**；
  同一集成 EXE 的 `nous-receipt-full-local-r5.log` 是 **14 passed / 6 ignored**，包含这 7 项，
  不能重复相加。六项均要求显式 disposable Redis URL，未执行，不算通过。
- 同一库测试 EXE 的相邻 `nous-receipt-adjacent-refresh-r3.log` 为 **145 passed / 4 ignored**；
  `nous-receipt-adjacent-runtime-r3.log` 为 **8 passed / 0 failed**。四项真实 Redis 合同未执行。
- `nous-receipt-static-r5.json`：22 份 scoped Rust 文件的 rustfmt、UTF-8 无 BOM、scoped diff check 通过。
- `nous-receipt-checker-r1.log`：checker tests **31 passed**。checker 未再变化，不重复执行。
  `nous-receipt-ratchet-r5.log`：5144 文件、14 个 immutable runtime assets、governed source >700=0。
  `nous-receipt-lines-r5.json`：本批 22 份源码最大 **407** effective lines，无 >500。
- 为避免向原 runtime 契约文件继续堆入 backend forwarding，将完整 Pooled backend impl 按职责移至
  `runtime/pooled_backend.rs`；原 `runtime.rs` 从 487 降至 406 effective lines，原有 forwarding 保留。

两份中间失败日志保留：`nous-receipt-green-r1.log` 的 `E0425` 是提取后 `WriterLockGuard`
相对路径错误；`nous-receipt-green-r2.log` 的 `E0624` 是集成测试误用 crate-private `document()`。
前者修正 owner 路径；后者改用真实 YAML 字节与公开 revision 检查，没有为测试放宽生产可见性。
这两份日志都不是绿测；库 EXE 虽在第二次命令中完成链接，该命令整体仍因集成编译失败退出。
后续实际执行的库与集成结果按上表分别记录。

只读交叉审查指出的 Welcome 目标继承与 device TTL 边界 receipt 丢失窗口均已修正并有定向回归。
没有因未知拒绝清空账号、虚构 TTL、增大测试栈或放宽生产计费来取绿。

共享工作区并非干净 HEAD，其他 owner 持续变更；范围哈希不等于整树来源证明。
最终源码范围为 `nous-receipt-candidate-r5.json`，实际执行的两个 EXE SHA256、结束后的范围
漂移核对见 `nous-receipt-final-r5.json`；明确 `wholeTreeProvenance=false`。
另一 owner 的 Cargo 编译未被停止，未清空 target、删除 EXE 或调整其参数。

## 明确保留的后续工作

本批未接 Nous preset/feature/manifest、credential typed context、SecretPatch/Keep、管理 session、
UI、目录应用、自动 refresh worker 的 received/pending/restart 恢复，以及 DB/cache 精确 CAS。
Local receipt owner 已实际验收；Redis receipt 分支只完成同一 CAS owner 的接线与编译，未取得
真实 Lua/Redis、Nous Server 或跨实例进程崩溃恢复验收，不能由相邻模拟合同替代。
`nous_portal_product_contract` 仍保留此前的 red，不能把模块存在当成产品入口已完成。
后续必须把收到回执、落盘失败保留/重试、后验 admission、Result、配置 publication 与 cache
串成真实路径，再验证三公共 API 的 Chat 转换、SSE、工具、取消、计费和来源绑定。
没有真实 Gateway client 注册与账号时，真实资格、授权/轮转、权益/费用和桌面/Server 验收单列。

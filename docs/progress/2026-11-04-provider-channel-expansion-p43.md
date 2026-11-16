# P43：Accio 完整轮换材料接入共享持久刷新链

日期：2026-11-04（Asia/Shanghai）。状态：本批接线与定向离线合同通过；R01/W01、正常 OAuth 登录/UI 和总目标仍在执行，不新增整卡 `offline_ready`。

## 1. 本批行为

接续 P41 协议与 P42 业务 body 所有权，移除 feature 开启时“生命周期未准备好”的临时 gate，改由独立 `token_refresh/accio.rs` 接入已有 scheduler、single-flight、generation、永久 grant 与配置 authority。没有另建刷新调度器。

- 发送前绑定 Accio adapter、固定根 gateway、固定 refresh endpoint、显式有效 client 和当前 access/refresh；混入 Grok context、外国目的地或伪装 generic adapter 均拒绝。关闭 Accio leaf 时仍在 reservation 前可恢复拒绝。
- 刷新发送复用 P41 的固定 JSON wire；`sgcookie` 不作为 Cookie header 或业务 body 发送。release 不含回环替换入口，实际 scheduler 合成测试仅经 compile-time task-local seam 改到严格校验的 `127.0.0.1` endpoint。
- 新增 credential-only 秘密字段 `accio_sgcookie`，接入 descriptor/redaction、禁止 draft 明文、普通 SecretPatch 的 keep/replace/clear。provider-level 路径不支持；Keep 按旧或新实际产品识别保护完整 resolved destination，不依赖必须已有 SG。
- 有新有效 SG 则轮换，省略/null 保留旧 SG；durable Result 保存最终有效值，replay 不重新猜前值。SG 同时贯穿 attempt、grant admission、runtime generation、有限配置投影、Published 等价比较、正常 cache 写回与重启 patch。SG-only 重授权使旧 generation/pending 失效，但不豁免迟到成功的 Result 存储义务。
- 成功材料仍按 **durable Result → runtime apply → 配置 authority → Published → cache** 的原顺序提交；不是将多个存储伪装为一个跨库原子事务。每一代 runtime 材料仍由同一锁更新，缓存仍 CAS 完整 raw。
- 原绝对 expiry 随 Result 保存、重放和恢复，不通过剩余 TTL 重建。已过期但成功轮换的材料也必须落盘；正 lifetime 元数据最低 1 秒只是旧框架兼容字段，dispatch/deadline/缓存中的绝对 expiry 不延长。

`ProviderAccountPayload`、业务 headers、extra body 均不新增 SG。cache 的 `account_payload` 是存储 `Value`，前后 SG 来自完整 durable record，而不是业务 payload；因此不需要向业务消费者开放该秘密。没有新增 credential/document 的 Debug 日志，也不宣称现有 credential 派生 Debug 已全局脱敏。

## 2. 旧格式与持久兼容

credential 与 GrantMaterial 的新字段均 `default + skip_serializing_if(None)`。没有 SG 时：

- 旧 JSON/YAML 不增加字段，不变更存储版本，不重写旧凭据或归档。
- material ID 保持原 7 项数组，不能尾加 null；新增 SG 时才追加明确 tagged 元素。
- refresh family ID 始终只由 endpoint/client/refresh 定义，修改 SG 不能绕过同 family 的 Sending/Result head。
- Sending record 的旧序列化字节、identity digest 和旧 locator 保持原样。
- 不因新字段顺带收紧无 SG 的旧 Accio record 的 access/client 校验；严格材料校验属于专用发送准入，不是旧格式读取迁移。

新增固定合成旧 record 与独立计算的三个 hash 常量、旧 locator 匹配；旧 revision 通过删除新字段后独立重算 JSON/YAML digest 验证读取及篡改拒绝。合成 Local archive 两次 startup 均保持 revision ID、credential ID、秘密、归档字节和 routes YAML 不变。

Redis Lua 未修改：正常 cache 写回仍 raw CAS，重启 repair 仍使用实际读到的 active ID、immutable revision raw、grant raw、Activated transaction raw 与 cache raw；没有以重序列化 JSON 取代 expected raw。

## 3. 实际验证

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-provider-expansion-20261104-p43`。

生产未改时 `red-material.log` 实际运行 **0 passed / 2 failed，exit 101**：旧 schema 丢失 SG，正确 Accio 材料仍被旧 gate 拒绝。不是编译报错冒充红测。

最终成功检查：

| 日志 | 实际结果 |
| --- | --- |
| `green-accio-refresh.log` | 17 passed，含本批 14 个新增库测试 |
| `regression-refresh.log` | 111 passed / 4 opt-in ignored |
| `regression-accio.log` | 95 passed |
| `regression-routing.log` | 119 passed |
| `management-and-legacy-v2.log` | 3 项真实管理 router 合同 + 2 项旧 revision/Local 重启合同通过 |
| `regression-grant-contract.log` | 7 passed / 6 disposable-Redis opt-in ignored |
| `regression-secret-contracts.log` | Console document、Bedrock、Qwen secret/consumer 四个 target 共 98 passed / 1 synthetic frontend fixture opt-in ignored |
| `check-focused-v2.log` | library 与两个新增 integration target 类型检查 exit 0 |
| `feature-disabled-check.log` | 仅关闭 Accio leaf、保留其余 runtime-defaults leaf，`cargo check --lib` exit 0 |
| `checker.log` | 31/31 passed |
| `rustfmt-check.log`、`ratchet.log` | 官方 scoped formatter 检查、无报告写入 ratchet 均 exit 0；仅管理测试状态断言随后修改并重新格式化，最终再次检查见 `rustfmt-final.log` |

成功 run 共 **452 次通过，按完整测试名去重为 418 个不同 Rust 测试，其中 19 个本批新增**；不累计红测、失败 run 中的局部通过或 ignored。命令、退出值、时长及新增测试名见 `verify-exits.json`、`management-v2-exits.json`、`followup-exits.json` 和 `test-summary.json`。

新增覆盖包含：实际 scheduler 的 SG 缺省/保留/首次/轮换，Local 持久与重启，不重复发送的磁盘重试，迟到成功与 SG-only 撤销，真实 Local Sending head 下 SG-only 编辑不能重新 reserve，原绝对 expiry 的 fresh/replay 过期拒绝，generic JSON 不能伪造 dedicated 字段，以及 cache 原 SG/错误类型/矛盾 alias 的拒绝和生产 CAS RESP wire。

管理测试实际验证 Secret Grant、Origin、原 revision/CAS、replace/clear、响应脱敏、provider-level 路径拒绝、没有 SG 的旧 Accio 与双向产品转换的 Keep 保护；不是正常 OAuth attempt/callback/receipt 登录流程。cache RESP witness 不是 Redis Lua 执行，本批未新建 Redis 容器，也未复用历史单容器授权。

### 3.1 过程失败与修正

1. 首次扩大 `cargo check --tests --lib` 被无关 `gemini_canvas_host_ingress_contract` 的 `queries overflow the depth limit!` 阻断；未修改它的 recursion limit，也不报告全仓检查通过。
2. 新增 integration 夹具初版调用了私有 `snapshot.document()`，触发 E0624；已改用现有公开 revision archive 接口，不扩大生产可见性。
3. PowerShell 5 的 native stderr 在 `ErrorActionPreference=Stop` 下把普通 Cargo warning 当作终止错误；修正证据 wrapper 的 native 调用段，保留日志，不改变生产构建语义。
4. `compile-targets.log` 的初次多目标 `--jobs 4` 因 `E0786` / `os error 1455` 失败，诊断为 Windows 页面文件/commit 资源不足，不是 rlib 内容损坏。当时物理内存负载 96%、可用 commit 约 6.4 GB。等待仍在运行的库测试编译完成，随后利用 Cargo 指纹缓存分目标以 `--jobs 1` 运行；没有删除缓存、改系统页面文件或干预其他进程。该资源例外不改变项目默认并发策略。
5. 管理初轮 **2 passed / 1 failed**：embedded-secret 的夹具误期待 400，现有 `internal_console.rs::secret_patch_to_gateway_error` 对 `secret_value_must_use_patch` 明确返回 422。v2 仅将该断言改为 422，并增加精确错误码断言；生产和库测试源码 hash 完全不变，见 `freeze-delta.json`。v2 实际全部通过。

P41 明列的四项既有 feature-disabled 测试失败未修复、未重跑；本批关闭模式仅代表性编译，不声称全 feature 矩阵绿。

## 4. 变更与边界

开始时保存完整 `git-status-before.txt`、HEAD 和 28 项既有 Rust 源码及 3 份文档的逐字节 before。本批新增 8 个 Rust 文件；最终覆盖 36 个 Rust 文件和 4 份文档，不用全 dirty 工作树 diff 冒充本批增量。

完整状态快照另观察到 5 项本批范围外的 desktop 删除/清理对话框状态变化，见 `git-status-delta.json`；本批未修改或回退它们，不归入上述 40 项，也不将工作树状态计数差全部归因于本批。

生产与库测试全部保持第一次成功测试时的冻结 hash；第二次冻结只改变新增管理测试的状态/错误码断言。最终 `source-final.json`、`scoped-diff.patch`、UTF-8 无 BOM/尾空白检查和 `git-diff-check.log` 负责范围核对。既有修改文件最大 492 有效行；新增最大 276，是同一管理入口合同职责，其余新增均不超过 150。没有新增 501+ 文件、超限增长、baseline/exception 或有效行报告覆盖。

未读取 `.ng`/历史凭据，未调用真实平台、录制账号、启用 management-ui/发布 hook、修改运行实例、出包、提交或推送。定向离线证据不证明真实 OAuth/付费业务、跨 OS 进程、断电/failover 或正式交付验收。

## 5. 后续接力

1. Accio 正常管理登录：provider/preset/revision/Secret Grant 绑定、显式 client、attempt/callback/receipt 生命周期与取消恢复，复用已存在框架。
2. 正常保存与模板/UI：稳定 credential ID 重授权、utdid 录入/保留、SG 与 access/refresh/原绝对 expiry 经普通 SecretPatch/CAS 完整保存；旧导入副本归一化按实际 owner 补齐，不把 SG 塞入业务 body。
3. 精确业务/probe、真实客户端使用范围和真实额度录制；仍按用户要求先完成无凭据开发，再集中录制，不逐平台催登录。
4. 其他原定渠道及 P50/P60/P70 继续。沿用 P42 的 11 个新增模板记录，本批没有新增模板或完整 `offline_ready` 卡；产品/额度 `unknown` 不合池、不制造余额。

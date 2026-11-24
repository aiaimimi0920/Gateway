# Luma Agents 成功媒体的晚到扣费补查

日期：2026-11-17。本轮只推进 R14 Agents REST 的成功任务费用读取；定向运行回归通过，不代表 Luma 整体完成。

## 行为与来源

本轮联网核实 [Luma 官方 Usage 指南](https://docs.agents.lumalabs.ai/guides/usage/)：任务完成与用量结算可能不同步；`credits_charged: null` 不能当作免费，而 `usage: null` 表示没有用量报告。官方没有承诺结算完成时限。

此前生成状态到达 completed 后只下载并返回媒体，费用字段不再更新。现在媒体下载并验证成功后，若用量对象仍待结算，则继续 GET 同一 UUID。最多 3 次、间隔 1/2/4 秒、最多额外等待 8 秒；同时服从原总 deadline，并给返回预留 250ms。已有金额（含零）或没有报告时不额外查询。

补查复用原固定目的地、精确 Bearer、无重定向 transport、单响应及全流程元数据预算。每个回复重新验证原 ID、模型、类型、completed 状态及费用数值；只接受实际 credits，不替换或下载补查回复的新资产。GET 出错、回复不可信或截止时保留已验证媒体，费用保持未知；受控诊断不含 Key、任务 ID、私人正文或资产 URL。

公共结果 `gateway_luma_generation.usage` 与 SQLite 审计 `lumaAgentsGeneration.usage` 使用同一事实，显式媒体 probe 也复用该 sender。原普通 Key 次数结算一次，probe 不扣普通 Key 余额；供应商 credits 不转换成 Token 或 USD。本轮没有新增 API、配置、数据库迁移或后台任务。

## 验证

验证日志目录：`../evidence/luma/gateway-luma-settlement-20261117`。

最终 `cargo test --locked --offline --lib upstream::luma_agents:: -j 4 -- --test-threads=4`：43 passed / 0 failed / 1 ignored，运行 14.54 秒，记录在 `rust-tests-v5.log`。包含此前栈溢出的完整管理入口，使用默认线程栈。原 browser-host 测试要求显式专用浏览器会话，本轮无 UI 改动，不运行它。

11 个新增用例覆盖晚到零值/小数与公共审计、已知或缺失报告、永久待结算、身份/状态/费用拒绝、HTTP/JSON/大小错误、总时限余量、8 秒上限、取消关闭响应体、共享字节预算，以及媒体验证失败时不补查。合成上游只监听本机，未调用真实服务商。

共享调用链相邻回归：40 passed / 0 failed / 0 ignored，含格式选择、prepared executor、凭据/配置投影、真实 loopback 的 buffered/stream/passthrough/native-shortcut，见 `call-flow-v5.log`。两组按名去重共 83 个 Rust 测试通过。生产 `cargo check --locked --offline --lib -j 4` 通过（5m39s，既有 warnings 保留），见 `cargo-check-v5.log`。

现有测试夹具默认显式使用 `usage: null` 代表没有报告；晚到结算用例独立指定 `usage: { credits_charged: null }`，不把未知值改为零。普通失败/资产/配置路径继续保留原回归。

构建曾因锁定 BoringSSL 临时源码目录缺失而失败，已使用仓库 `prepare-cpa-bssl.py` 和本地固定哈希归档恢复；无依赖升级、重新下载或删除旧目录。检查器 31 项测试通过；有效行数 ratchet 通过，5778 项文件、14 项 immutable runtime artifacts 单独归类，governed source 没有超过 700 行。

受影响源码 rustfmt、`git diff --check` 和 14 项源码/文档的 UTF-8 无 BOM 检查通过。没有执行全仓库发布回归、安全审计、真实上游调用或 EXE 构建。

首轮 Rust 编译通过后，正常管理媒体 probe 触发 `STATUS_STACK_OVERFLOW`。装箱 generation 和管理测试 future 后仍复现，不能把这两次尝试记作修复。通过 Windows Debug API/StackWalk64 在本次合成测试子进程上取到实际栈：共享 `execute_json_passthrough` 的 poll 帧约 589648 字节，加上媒体、console 和 HTTP probe 后累计耗尽默认栈，在首次 POST 建连时溢出，尚未到补查阶段。

最终修正将共享 JSON dispatcher 的非 Luma/CommandCode 分支构造移至同步函数返回 boxed future，保留前置 admission、单次 call_flow prepare 及原 executor 投影。原分支主体经忽略空白及 executor 参数更名的精确对比一致；没有绕过普通 sender、加大线程栈或派生生产后台任务。此文件从 443 到 472 有效行，其他改动源码最大 217 行。完整管理入口仍用默认栈验收，取消仍由原请求拥有。

首次测试二进制单独执行此前未运行的 11 个新增补查用例全部通过（8.38 秒），随后排除已知崩溃入口的诊断运行有 42 项通过；这两次仅用于定位、不重复累计，最终以含原管理入口的 v5 完整定向结果为准。

## 尚未覆盖

本轮不提供失败生成的费用审计、超过补查截止时间后的后台追账、cash/USD、其他 Luma 操作或 MCP 创作业务。真实账号晚到扣费、真实视频解码/播放、Server/PG 与完整产品验收仍待完成。

没有读取真实凭据、生成付费内容、修改 4200/4400 的运行库或重打 EXE。当前已交付的 4400 包不包含本轮源码；本轮未提交或推送 Git，也不减少按整项去重的剩余数量。

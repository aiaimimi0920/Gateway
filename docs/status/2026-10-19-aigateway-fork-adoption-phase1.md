# AIGateway fork 能力重实现：第一批

本记录对应用户授权“将值得加入的内容使用我们的新框架重新实现后加入 Gateway”。
来源能力清单见 [fork 审计](../plan/2026-10-19-aigateway-fork-adoption.md)。
这不是“全部候选能力已完成”的声明，也不是发布或上线回执。

## 已接入的运行路径

### A03：精准的模型容量错误分类

新增独立 `ModelCapacity`，只接受限定 status 下的明确 code 或错误消息前缀。
普通请求字段、被引用的 prompt、认证/验证等冲突 code 不获得此例外。
保留上游原始 status/code；不会把容量 400 改写成虚假的 HTTP 429。

共用重试 owner 仅在 policy 允许 503 时允许容量 400 重试；仍使用既有
attempt、deadline、等待预算、准入和副作用重放限制。健康分类为非永久的
`ProviderTransient / CredentialModel`，不误当作永久凭据失效或普通额度耗尽。
具体合同见 [等待与重试策略](../retry-wait-policy.md#模型容量错误)。

实现 owner：`src/error/model_capacity.rs`、`src/error/classification.rs`、
`src/retry.rs`、`src/provider_failure.rs`。

### D03：结构化输出格式保真

Chat 的 `response_format` 与 Responses 的 `text.format` 双向转换，支持
`text`、`json_object`、`json_schema`。原样保留 schema、name、description，
区分 `strict: false`、null 和未提供；不自动开启 strict，不改写 schema。
转换后不把另一个端点的字段名一并发送。原生 Responses 的 text 配置继续保留。
`text.verbosity` 与 Chat 顶层 `verbosity` 显式互转；Responses-only 的 `include`
不会混入 Chat 请求。实际 `pack_responses_bridge` 也保留转换后的 `text`，不是只验证
局部 pack helper。字段布局已对照 OpenAI 的 [Responses](https://developers.openai.com/api/reference/resources/responses/methods/create)
和 [Chat Completions](https://developers.openai.com/api/reference/resources/chat/subresources/completions/methods/create) 合同核实。

实现 owner：`src/protocol/output_format.rs`，接入现有 normalize/pack 链。

### D02 子集：自定义工具、工具选择与流式身份

- 自定义工具定义、指定工具选择、`custom_tool_call.input` 和对应 output
  在现有 canonical 请求/响应及双向 SSE 翻译路径接通。
- 自定义输入是原始字符串，不 JSON 标准化，不 HTML entity 解码。
  普通函数工具继续使用原有函数参数兼容处理。
- 保留函数定义的 strict 与扩展配置；`allowed_tools` 转换其选择器布局，
  不擅自扩展为整个工具集合，也不删除原有工具目录。
- 只对同一 OpenAI 协议族保留 raw 工具扩展；Anthropic 等协议的 `input_schema`、
  `cache_control` 不会泄漏进 Chat 的 function 对象。
- Responses 原生请求和 buffered 响应保留 item id、call id、namespace 的区别。
  `previous_response_id` 下只有 output 的续接也保留其 custom/function 类型。
- SSE 以 item id 关联增量和终态，避免把 call id 当成另一个 item，造成第二次
  工具 announcement 或重复输入。终态只补充未发送的尾部；内容冲突明确失败。
- SSE 翻译器最多保留 128 个工具、累计 8 MiB 输入，身份字段限制为 512 UTF-8
  字节。超限返回协议错误，不生成成功终态。这里是翻译器状态上限，未更改原生
  passthrough 的 API 合同。未创建后台任务、定时 probe 或额外重试循环。
- 名称延迟到达时保留原始 delta 边界，使用字符串边界索引而不是再存一份输入；
  未 announcement 的 pending delta 元数据总量另限 8192 项，防止大量极小分片累积。

实现 owner：`src/protocol/openai_tool_wire.rs`、OpenAI/Responses 的既有
normalize/pack/response 模块，以及两侧 `tool_calls.rs` 流式状态 owner。

### 完整 HTTP 回归暴露的栈边界

全量串行 gate 在未修改断言的 `access_key_groups_contract` 首个真实 loopback relay
复现 Windows 默认测试线程栈溢出。诊断定位到共用 buffered attempt 的 legacy HTTP
future 进入多层 transport poll；仅给测试层的两个 router future 装箱没有解决问题。

最终在 `src/pipeline/stage_send/buffered.rs` 将既有 attempt future 装箱，避免将大型
多供应商 future 内联带入 retry/gate/调用者帧。仍在同一个任务中 poll，取消/Drop、
准入 lease、deadline 和发送次数的 owner 不变，没有新增任务或改变供应商实现。
原样恢复测试文件后，三个 account-group 合同在默认栈上全部通过；没有保留诊断打印、
扩大 `RUST_MIN_STACK` 或削弱断言。更大栈只用于一次问题诊断，不计入验收结果。

扩大回归还发现原生透传测试把 Chat 的 `[DONE]` 夹具用于 Responses，与 HEAD 已有的
响应级完成证明不一致。仅在 `tests/account_protocol_routing_contract.rs` 为 Responses
补充合法 `response.completed`，保留 comment/id/vendor event 和逐字节相等断言；未改
生产完成校验。两个原生路由合同和八个流完整性合同均通过，截断/失败流仍明确失败。

`stream_observation_limits` 的旧夹具也把 `message_start` usage 当作可立即发布的最终
快照，与 HEAD 的现金计费保护冲突。只补一条后续 `message_delta` output usage；保留
原来的字节指针、延迟发布、prompt/output 计数与 event 空白处理断言，未改 usage 或
计费实现，不能把 provisional usage 当最终账单。

本批未新增存储格式、数据库迁移或并行状态 owner。工具状态按项数、字节和标识符长度
设限；容量分类只看明确错误字段，不扫描 prompt。新增装箱每次 attempt 有一个受任务
生命周期约束的分配，不新增发送、后台任务或锁，不更改 credential pin 和授权边界。

## 验证记录

新增四个集成测试文件，共 19 项专项用例。主要缺口与交叉核验问题均先观察真实失败，
再接线实现；另增加资源上限保护用例。最终聚焦回归包含这 19 项、27 项预算/流完整性
及 15 项本地存储/账号组/原生路由/观测合同，合计 61 项通过，不重复计入全量总数。

| 最终本地检查 | 实际结果 |
| --- | --- |
| Rust 全库 `cargo test --locked --offline --no-fail-fast -- --test-threads=1` | 4035 passed、0 failed、104 ignored；95 个 suite 结果，退出码 0。 |
| Rust 全目标编译 `cargo check --locked --offline --all-targets` | 通过，退出码 0；已有 unused 等 warning 未作为本批无关清理。 |
| `cargo fmt --all -- --check` | 通过。 |
| Node 脚本测试 | 2122 passed、0 failed、2 skipped。 |
| Python 测试 | 362 项中 358 passed、4 skipped；使用匹配 requirements 的已有隔离环境。 |
| 桌面 TypeScript typecheck | 通过。 |
| implementation-line manifest 校验 | 44 checked、0 errors。 |
| 有效行数 checker 测试及 ratchet | 31 项 checker 测试通过，ratchet 通过；未重置 baseline 或新增例外。 |
| UTF-8/BOM、源码尺寸与 diff 检查 | 本批文本均为 UTF-8 无 BOM；源码/测试最大 443 有效行；`buffered.rs` 为 360→360；`git diff --check` 通过。 |

最终 Rust gate 明确清除 `RUST_MIN_STACK`，以默认线程栈通过；单线程测试顺序沿用仓库
既有验证方式。早期默认并行 lib 曾竞争 ChatGPT Web solver 的两个 slot，最终没有通过
扩大生产上限或删除断言来消除此问题。默认忽略的 104 项没有强行启用，不宣称真实供应商、
外部数据库、打包运行或人工业务验收已覆盖。

最终源码检查结果和过程失败记录在本次本地证据目录：

`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-fork-adoption-20261019/`

- `verified-closure/verification.json`：上述最终 Rust 全库、61 项聚焦合同、全目标编译、
  formatter、checker、ratchet、diff 的退出码及起止时间，各命令有独立 UTF-8 日志。
- `after-stack-fix/verification.json`：最终生产源码对应的 Node、Python、typecheck、
  manifest 校验。其后只修正两个测试夹具并更新本记录，没有再修改生产源码。
- `verification-summary.json`：对最终日志的逐 suite 计数及命令回执汇总。
- `verification.json`、`final/`、`wide/`、`serial/`、`complete/`、`final-complete/`、
  `verified/` 保留过程证据，包括协议 red/green、默认栈溢出、旧夹具失败；一次测试
  二进制正在运行时发生的重链接 `LNK1104` 也保留，最终已无重叠构建并串行重跑通过。
- `affected-after.json`：本批文件、初始 clean 文件对应 HEAD 的有效行数、最终行数、
  UTF-8 BOM 检查及 SHA-256；不把已有用户修改计入本批。
- `preservation-check.json`：16 项继承现场与 fork 219 项源码/HEAD/branch/index 的复核。

本批运行路径已接入并完成以上本地源码验证；这不是整个 fork 采纳计划完成，也不是
gateway-productization 的打包/隔离运行阶段完成。原始审计报告继续作为只读历史基线，
不能把其中“尚未实施”的旧时点描述或本批专项结果当作剩余候选已交付的证明。

## 明确保留的后续能力

| 审计项 | 当前状态与边界 |
| --- | --- |
| C01 模型证据观测 | 尚未接入。必须在原始供应商解析边界区分自报 model、别名和合成回填，不能从打包后的 model 冒充真实证据。 |
| D01 reasoning 往返 | 尚未实现正文/历史和 SSE 的完整保真；没有把明文 Base64 包装成 encrypted_content。 |
| D02 namespace 跨协议与历史合法性 | Responses 的已有 namespace 保留，但没有为 Chat 不可表示的命名空间伪造无碰撞映射，也未加入静默删除 orphan/重复 output 的策略。 |
| A04 指定账号/OAuth 优先 | 尚未新增；现有严格 credential pin、账号授权和付费 fallback 边界不变。 |
| B01/B02 账号组共享预算、单请求 half-open | 尚未新增；不能拿现有软冷却排序冒充跨实例原子许可。 |
| B03/B04/B05 外部 advisory 健康源 | 尚未新增；需要独立的新鲜度、同源/SSRF、认证脱敏及生命周期合同。 |
| E01/E02 维护预览与定向恢复 | 尚未新增；原 archive/CAS owner 不变，不做基于文案的永久删除。 |

本批没有修改 AIGateway，未运行其 live OAuth 验证脚本，未请求真实供应商；
没有修改运行配置、重启服务、制作发布包、提交、推送或部署。

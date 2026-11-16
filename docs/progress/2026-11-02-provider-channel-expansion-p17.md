# 服务商分渠道扩展：第 17 批，普通 Qwen 公共 buffered 推理与响应准入

> 2026-11-02；R09 / W05 / P01 / P10 的无凭据切片。buffered 响应准入及本批定向离线验证完成；不是 W05 或总目标完成。
> `main`，HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`；保留 P01–P16 脏改，没有提交、推送、打包或重启实例。

## 1. 产品与范围

仍是普通 `qwen-api` / `qwen-official-api` workspace API Key 产品。没有改成 Coding Plan、OAuth 或 Web，也不合并它们的凭据、受众和额度。Web 继续作为独立有价值的候选渠道，不是认证兜底；额度共享关系没有证据时保持隔离、标 `unknown`。
本批未新增外部产品事实、依赖或第三方源码；沿用已联网冻结的 [API 合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/plan/provider-api-contracts.md)。没有新的联网调查、历史凭据读取或真实上游调用。

本批只处理普通产品公共 Chat / Responses 的 **buffered** 路径。P16 是管理 probe 的正式发送接线；本批新增真实公共 router → auth/filter/route/send/finalize → 产品响应 → 客户端协议 → 独立 SQLite audit/usage 的证据，不把两种测试混为一项。

## 2. 生产 owner 与边界

- [request_plan_part_2.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/client/request_plan_part_2.rs) 在正常 planner/header/sender 和成功 HTTP status 之后，按 workspace 产品 ownership 分派到新的 [workspace_response.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/qwen/workspace_response.rs)。不是按入站 endpoint 选上游 decoder，也不再回退 Responses/Accio 解码器。
- 新 reader 复用现有 `upstream_body` **64 MiB decoded-body 累计预算**和 Content-Length 预检查；这是 Gateway 本地准入策略，不是官方服务上限，也不是 P16 短 probe 的 64 KiB 上限。累计超限或 allocation 准入失败不自动重放同次生成。
- JSON/UTF-8、非空 choices、每个 choice 的 assistant message、已知终态、可由既有 canonical parser 消费的 function tools/legacy function_call 先校验，再复用 OpenAI Chat canonical decoder。顶层非 null error、缺终态、畸形工具不被当成成功。
- 不套用 probe 的“只允许 stop + 非空文本”：正常 `length`、`content_filter`、工具调用及有意空字符串保留其真实语义。usage 可缺省；存在时验证整数、checked total 和有效 cached_tokens，primary/alias 顺序与 canonical parser 一致，不编造使用量。
- 畸形响应使用固定 `invalid_qwen_workspace_response`，不回显任意 body 或 JSON parser 原文；这类错误不作同候选自动重试。不改变用户原来显式配置的跨 provider fallback。
- full-route 终态矩阵另揭示共享 OpenAI builder 把 `content_filter` 归一为 `stop`。在 [response_builders.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/openai/response_builders.rs) 的实际 wire owner 最小增加正确分支及保护测试；其他 finish mapping 不变，没有在 Qwen 路由层补丁覆盖公共输出。

这不是 reasoning/refusal 全字段保真或多 choice 全部输出的证明；现有 canonical 只返回首 choice，相关剩余边界见第 6 节。

## 3. 离线安全和实际路径

新测试使用合法 workspace/provider/preset、正常三适配器、两个合成账号和模型映射；只有目标账号有被测模型。正式 planner 与最终 header 处理完成后，**仅 `cfg(test)`** 的 task-local seam 改 transport URL，原 payload/模型/认证不变。生产没有可由配置、环境变量或 HTTP 启用的改写入口。
本地随机端口 server 同时是显式非转发 proxy：非 POST（含 HTTPS CONNECT）拒绝；POST 只接收精确 loopback authority，支持实际 origin-form 与 proxy absolute-form。专门的无 seam HTTPS 测试要求本地 proxy 实际观察并拒绝 CONNECT。`managed=false` 防止绕到独立 wait client。
公开 `/v1/chat/completions` 和 `/v1/responses` 经真实 `build_router(...).oneshot`，不是 stage_send-only 或单独 planner 测试；Windows 使用独立 16 MiB 栈和 current-thread Tokio。wire 检查 workspace-b、精确 Key、映射模型、Codex 指纹、Chat body、输出预算和唯一发送；audit 分别检查 provider、`route_trace.realCredentialRef=exact`、模型和实际 usage/cache，未用子串匹配冒充精确账号归属。
所有 TEMP/TMP/TMPDIR 均指向 [P17 证据目录](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-provider-expansion-20261102-p17) 下的 runtime-temp。独立合成 SQLite 关闭后留在该目录；Console fixture 只清理该 TEMP 内自己创建的 UUID 目录。没有读取或修改真实 `.ng`、数据库或凭据，没有创建临时网络盘符。
Cargo 在本任务内串行、`--locked --jobs 4 --no-default-features --features runtime-defaults`；未启用 `management-ui` 或调用 Web 发布/清理。本轮曾观察到独立 Otter 构建并存，没有停止或修改它；编译耗时不当作稳定性能数据。

## 4. 验证记录

### 4.1 红测与中间失败（保留，不覆盖）

- `red-v1.log`：0 pass / 6 fail，全部先撞到 fixture 要求 URI 带 scheme 的错误假设；实际为 origin-form。**不是产品红测证据。**随后的 fixture 同时接受合法两种形式，仍严格核对 loopback authority。
- `red-v2.log`：**3 pass / 5 fail**。Chat 基线、无 seam CONNECT trap、无效公共 Key 不触网通过；Responses 输出被错 decoder 吃成空字符串，以及 oversized/error envelope/invalid message/缺 finish 被当成功，五项均实际复现。此轮同时已修正 audit selector：provider 身份不是 credential ID，精确凭据应检查 route_trace。
- 首轮 `green-pipeline.log`：**9 pass / 1 fail**。原五项产品红测已绿，但新增终态矩阵实际发现 Chat `content_filter` 变 `stop`。它是产品输出缺口，不通过放宽断言处理；随后在共享 builder owner 修复。
- 只读交叉审查发现 cache alias 可绕过新校验；已补 primary/alias 有效值同序验证及仅 alias 越界、空 primary + alias 越界、合法 alias、primary 优先的回归。这个问题来自源码审查，没有声称另有独立执行过的红测日志。

### 4.2 最终验证

全部 Cargo 命令由 `run-green-v2.ps1` 串行执行并实际 exit 0，没有 0 tests 冒充通过；日志保存在第 3 节证据目录。

| 日志 | 实际执行 | 结果 |
| --- | ---: | --- |
| `green-pipeline-v2.log` | 10 | 全部 pass；包括 Chat/Responses 输出、精确 wire/audit/模型健康、错误、64 MiB 累计超限、公共鉴权和 CONNECT trap；终态矩阵的六次请求仍只计一个 test |
| `green-response-v2.log` | 6 | 全部 pass；结构、终态、工具、usage/cache primary/alias 验证 |
| `probe-regression-v2.log` | 26 | 全部 pass；包含 P16 七项，不重复累加 |
| `sender-regression-v2.log` | 3 | 全部 pass；普通 workspace/compact/最终 sender 合同 |
| `bounded-regression-v2.log` | 12 | 全部 pass；共享 collector 的字节边界、deadline、drop、charset/BOM 和错误分类 |
| `openai-regression-v2.log` | 62 | 全部 pass；包括本批新增的公共 builder content_filter 保护，不另重复计算 |
| `integration-regression-v2.log` | 10 | Cohere 真 router 四项、Qwen workspace runtime 六项全部 pass；生产 library 不含测试 seam，Cohere 的流式通过不是 Qwen stream 证据 |

默认 runtime 定向合计 **129 pass**。另 `api-off-runtime-contract-v2.log` 使用 `line-qwen-official-core,family-openai-compatible-official-api,line-gemini-web-reverse` 组合，**5 pass**，确认 API-off 不回落通用 OpenAI，并编译新响应 owner。它不是失败的纯 core-only 组合，也不是 API-off 管理 probe 或完整公共 Qwen SSE 验收。
本轮没有全仓回归、真实上游或费用测试；真实额度/现金结算未以这些合成 usage/audit 结果代替。

`checker-final-v2.log` **31/31**；`ratchet-final-v2.log` 扫描 **4008 文件、14 immutable、governed source >700=0**。官方 Rustfmt、全工作树 `git diff --check`、五个新增源码和四份未跟踪文档的独立空白检查均通过；ratchet 沿无报告入口，没有覆盖既有脏的有效行报告。
早期辅助脚本分别遇到空 formatter 输出被传为 null、把 `git diff --no-index` 对干净新增文件的 exit 1 误判为失败；不是 Rust/checker 失败。`run-static-v2.ps1` 已正确处理空数组和无诊断的新增差异，实际 exit 0；收尾对最后文档再次做来源/UTF-8/空白检查，不覆盖旧日志。

## 5. 来源、有效行与兼容

`source-before.json` 记录四个初始 owner 的脏树起点及五个新增源码的 0 起点；新增 shared builder 写入前另有 `source-additional-before-v1.json`。`source-frozen-v1.json` 记录本批最终 10 个源码 hash；最终源码复核匹配。

| 源码 | 前 → 后有效行 |
| --- | ---: |
| `src/upstream/client.rs` | 444 → 450 |
| `src/upstream/client/streaming.rs` | 350 → 354 |
| `src/upstream/client/request_plan_part_2.rs` | 429 → 432 |
| `src/protocol/qwen/mod.rs` | 11 → 12 |
| `src/protocol/openai/response_builders.rs` | 183 → 210 |
| 新 `src/protocol/qwen/workspace_response.rs` | 148 |
| 新 `src/protocol/qwen/workspace_response_tests.rs` | 153 |
| 新 `src/upstream/client/qwen_pipeline_test_support.rs` | 150 |
| 新 `src/upstream/client/qwen_pipeline_tests.rs` | 197 |
| 新 `src/upstream/client/qwen_pipeline_response_tests.rs` | 170 |

五个新增源码均在 148–197 有效行，全部 10 个源码小于 500。源码和四份本批文档严格 UTF-8 无 BOM、无尾随空白，文档本地引用和代码围栏检查通过。
不新增 baseline/例外、依赖升级、锁文件变化、存储版本或数据库迁移。仅对普通 workspace 产品增加响应分派；其他产品保持原 decoder owner，公共 builder 只修正明确的 content_filter 终态。
P11 的 `protocol/qwen/mod.rs` 和 `upstream/client/streaming.rs` 因本批模块/测试接线有意变化；不能伪称它们仍等于 P11 hash。其他 P11 记录与 P12/P14/P15/P16 各自最后 frozen source 逐项匹配，审计脚本对任何未拥有的变化仍报错。所有本批 exec/Cargo sessions 均已实际结束。

## 6. 未完成项与接力

1. **下一最小任务：普通 Qwen stream 的产品原始 Chat SSE 终态验证，在 `stream_translation::translate` 之前接入。** 当前通用 Responses converter 在 delta→EOF 或 delta→`[DONE]`、没有合法 finish_reason 时仍可能制造 completed；必须先用真 router/body 建红测，不能只检查转换后的 completed 自证成功。
2. stream 的 Chat/Responses、工具、usage/cache、错误、取消、连接/permit 和 audit/finalizer 闭环仍未验收；未来复用 task-local seam 时，应把惰性 body 消费/drop 也包在 scope 内，不只包 `oneshot`。
3. Chat downstream usage builder 仍只输出 prompt/completion/total 三字段，尚不输出 cached details。本批不能声称 Chat 客户端收到 cache details；Responses 和 audit 的 cache 证据分别以实测为准。reasoning/refusal 字段和多 choice 保真也未闭合。
4. P16 共享 probe recording 的外层 future drop audit 缺少 Drop guard，仍可能留未完成记录；普通 buffered 外层取消 audit 也没有本批闭合证据。不以连接停止代替 audit finalize。
5. 精简 `line-qwen-official-core` 的 Gemini Web 编译失败、P13 极端 expiry 和 P15 已有前端四项失败仍保留，未越过 S06 lane 修 Gemini，也未声称本轮重跑这些旧失败。
6. Bedrock 普通参数/reasoning/signature/取消收尾，其余 R01–R16 的 API/OAuth/Web/媒体，P50 devtest 包、集中录制和逐渠道真实验收仍保留。未真实登录、刷新、推理、额度核验、浏览器手测、出包或启动 EXE，不新增蓝色验收标记；W05 和总 goal 继续 `in_progress` / active。

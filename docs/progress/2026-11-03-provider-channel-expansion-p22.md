# 服务商分渠道扩展：第 22 批，Bedrock 基础生成参数桥接

> 2026-11-03（Asia/Shanghai）；W06 / P10 的基础请求参数切片，不是 Bedrock、OAuth 或总计划整体完成。
> 本批实现与最终冻结源码定向验证已收尾：92 pass / 0 fail；不是 W06 或总计划完成。
> `main`，HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`；不提交、推送、打包、改凭据或接管在线实例。

## 1. 先核实真实 owner，纠正探索遗漏

P21 后续只读探索最初漏读已有 `src/protocol/generation_parameters.rs`。主代理亲读 HEAD 后确认 OpenAI Chat、Anthropic、直接 Responses packer 都已经调用它，能从 Bedrock inferenceConfig 提取已列字段；“合法 maxTokens=17 普遍退回 Anthropic 默认4096”的推断错误，已撤回。不能重复实现既有反向转换，也不将已有通过行为算作本批修复。

真正的目标侧缺口是：Bedrock packer 将通用别名原样写到 AWS JSON 顶层，未形成 inferenceConfig；provider defaults 可因此整体注入另一上限。现有 native 往返合同并不证明跨协议参数有效。反向共享 helper 对未知 nested 字段、非法类型和 Responses bridge allowlist 的额外边界继续保留，不借本批四参数映射冒称全部保真。

## 2. 本轮联网核实的官方合同

2026-11-03 无凭据只读读取 AWS [InferenceConfiguration](https://docs.aws.amazon.com/bedrock/latest/APIReference/API_runtime_InferenceConfiguration.html) 与 [Converse](https://docs.aws.amazon.com/bedrock/latest/APIReference/API_runtime_Converse.html)，两者明确 Converse / ConverseStream 共用基础参数：

| 字段 | 官方基础合同 |
| --- | --- |
| `inferenceConfig.maxTokens` | integer，最小 1；本轮不臆造模型专属最大值 |
| `inferenceConfig.temperature` / `topP` | float，0..1 闭区间；零值不能丢失 |
| `inferenceConfig.stopSequences` | 字符串数组，0..2500 项；每字符串至少 1 字符 |

专属模型参数使用 `additionalModelRequestFields`，不把 top_k、penalties、seed 等自动塞入基础 inferenceConfig。API 形状不证明任意模型同时支持所有采样组合或具体额度/资格；Prompt management 等附加限制也不由本批完成全 schema 验证。

## 3. 实际 red 和实现

证据目录：[P22](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-provider-expansion-20261103-p22)。修改生产前，`red-generation-v1.log` 实际 **1 pass / 5 fail / 0 ignore**，exit 101：

- Chat/Responses/Messages 通用字段没有形成 inferenceConfig，实际 maxTokens 为 Null 而非17。
- provider 的 inferenceConfig.maxTokens=99 取代了未转换的 caller max_tokens=17。
- 冲突 aliases 未拒绝；非法原生参数未校验；相等 aliases/零值未生成正确对象。
- 原生未知字段往返与已有 reverse bridge 的用例已通过，不能把它报为红测修复。

实现新增 Bedrock 专属 generation / validation owner；四类基础字段有固定映射，stop 字符串规范为数组。checked pack 在 source projection 前后都检查，防止原始冲突被隐藏。原生字段与 aliases 不一致即拒绝，相等值接受；采样整数0与浮点0.0按数值相等处理。未知 native config 字段保留，仅校验已声明四类。

公共 `pack_bedrock_converse` 保留 Value structural API，不假称直接调用都做了 admission；真实 planner 改用 fallible `try_pack_bedrock_converse`。native normalize 校验原生输入，公共 request planner 在 provider defaults/profile 处理后再校验最终 body，拒绝重新注入顶层通用别名或非法 config。错误 code 固定 `bedrock_generation_parameters`，只含固定字段名，不回显值。

沿用框架原有顶层 defaults merge：caller 已有任何 inferenceConfig 时，provider 的整个同名 defaults 不注入；没有该顶层字段时才整体注入并验证。**没有新增递归逐字段 defaults 合并。**未改认证、签名、路由、计费、schema、依赖、存储版本或 S06/Gemini owner。

## 4. 验证进度与边界

新参数合同覆盖三来源的 buffered/stream plan、冲突/非法值/不回显、原生扩展字段和既有 reverse bridge、零值/等值、stop 2500/2501 与空数组、浅层 defaults；新 wire 合同使用真实 execute/execute_stream、随机 loopback 和合成 legacy auth 捕获最终请求 JSON，并检查非法 caller/defaults 在发送前拒绝。

wire 合同不命名 crate-private 的 streaming enum；取得 streaming response 后 Drop，仅证明实际请求参数，不替代响应 EOF/完整 EventStream 处理。buffered execute 则完整消费合成 EventStream；后者完整路径另由受影响既有生产合同回归证明。loopback 不冒称官方 Bearer/SigV4 认证或真实 AWS 可用。

`green-generation-v1.log` 实际 exit 101：原 Converse 合同6项、参数合同6项、负向wire1项通过，正向wire1项失败，原因是测试误以为 buffered execute 请求 Converse JSON 并应返回 OK；真实 owner `request_plan_part_2.rs` 既有行为是强制 ConverseStream 再聚合，合成结果为“你好”。按真实调用点修订测试，同时增加 execute_json_passthrough 的实际 Converse JSON 发送；不修改生产 forced-streaming 行为，也不把该测试预期修正当产品 bug。

初始 green 编译期间补强了测试（移除 private 类型 import，加入数值等值和浅 defaults 断言）；随后性能复读调整了本批生产 view：普通 Chat/Responses/Anthropic/Bedrock 借用 canonical request，不为四字段映射额外 clone 完整 prompt/raw_body，仅 Gemini nested source 委托既有 helper，没有修改 S06 文件。早期构建不作为最终来源。最终在 `source-frozen-v3.json` 九文件来源上重新执行，v1/v2 快照与失败日志保留。

### 4.1 最终实际结果

`production-regressions-v3.log` 编译 8m16s，12 个 suite：参数6、真实发送2、Converse6、原生输出18、原生pipeline6、AWS入站20、AWS入站pipeline3、旧EventStream13、精确probe7、Qwen运行6、Cohere发送4，合计 **91 pass / 0 fail**。材料贯通1项是显式 opt-in，首次为1 ignored，没有把它当通过。随后明确设置 P07 的合成 fixture 路径重跑 `material-roundtrip-v3.log`，**1 pass / 0 fail / 0 ignore**；合计92个实际执行通过的测试，不累计初版 green 重复用例。

`fmt-v2.log`、`checker-v2.log`、`ratchet-v2.log`、`diff-v2.log` 对应静态检查已通过：官方 Rustfmt、checker 31/31、ratchet 4031 文件 / 14 immutable / governed source >700=0、全工作树 whitespace。源码 hash/UTF-8/文档链接与围栏、最终 tracked/untracked diff 在 `source-verify-final.log` 和 `untracked-diff-final.log` 留存。P19/P20/P21 的冻结 owner 未改。

有效行前→后：`bedrock_converse.rs` 101→104、`packing.rs` 166→187、`normalization.rs` 206→207、upstream common 154→154、planner part1 146→151；新 generation46、validation88、参数合同218、wire合同226。没有新增500行以上源码或有效行例外。

## 5. 继续保留的范围

API Key/OAuth/Web 按实际产品和额度隔离，unknown 不合池，Web 不是兜底；R01–R16 全范围不变。本批不使用历史凭据或付费上游，不生成 EXE、部署或重启实例。TEMP/TMP/TMPDIR 指向本批 runtime-temp，Cargo 串行使用 locked、jobs4、no-default-features + runtime-defaults，不触发默认 management-ui 发布/清理流程。

Bedrock reasoning/signature、其余模型专属参数/能力、专属完整取消 audit、Qwen 完整保真与 PG principal/source reserve 等边界、OAuth 生命周期、其余 Web/媒体、P50 包/隔离启动、集中录制及逐渠道真实验收仍开放。以前 feature/前端/expiry 未通过项不消失，总 goal 继续 active。下一步转向 R05 Grok OAuth 的独立固定受众 grant/refresh 与设备授权基础，不以本批替代横向渠道开发。

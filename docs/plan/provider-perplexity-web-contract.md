# Perplexity Web：独立消费端渠道的公开协议参考

> 2026-11-03（Asia/Shanghai）；R12 / W08 / P00 / P30。公开只读调查，不是已实现或已验收声明。
> 关联：[总计划](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/plan/2026-11-02-provider-template-expansion.md)、[渠道身份表](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/plan/provider-channel-identity.md)。
> P37 后续已新增独立单轮 request/parser/transport，当前实现与保守边界见[实现合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/provider-perplexity-web.md)；下文“本次”指原公开调查，不代表后续源码仍不存在。

## 1. 范围与证据

消费端 Web 独立于 Sonar/Search/API Key；不能用平台 API 成功冒充 Web 或默认共享凭据/额度。本次只读取公开仓库和官方文档，未登录、发送业务请求、读取账号或历史材料、安装/执行参考项目，也未复制代码进 Gateway。

公开文件保存在 [参考证据目录](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-provider-expansion-perplexity-reference-20261103)，含 `provenance.json`、`reference-index.json` 和文件快照。固定版本：

| 来源 | commit / 许可 | 可用边界 |
| --- | --- | --- |
| [edusu/perplexity-cli](https://github.com/edusu/perplexity-cli/tree/9b62d77e7dacb6905f81d577f824b83626e9db16) | `9b62d77e7dacb6905f81d577f824b83626e9db16`；pyproject/README声明MIT，但tree没有LICENSE，GitHub license为null | 可以定位行为线索；未取得完整版权/许可文本，不直接复制或补造声明 |
| [helallao/perplexity-ai](https://github.com/helallao/perplexity-ai/tree/05e1b65971ddbe0ac4eb0ab13296a8e78f710934) | `05e1b65971ddbe0ac4eb0ab13296a8e78f710934`；完整MIT，Copyright (c) 2023-2026 helallao | 可登记行为参考；采用受保护代码时保留许可/版权，不引入仓库无关能力 |

官方 [API FAQ](https://docs.perplexity.ai/docs/resources/faq.md) 说明 API/UI 的配置、工具、模型及结果可能不同，API credits 在 API Console 购买。它没有提供消费端 `/rest/sse/perplexity_ask` 的稳定协议、Web session刷新或机器可读剩余额度合同。Help Center本次返回403，没有绕过；开源代码许可不等于厂商自动化接口授权。

## 2. 可定位的 wire

| 层面 | 固定公开证据 | 实施约束 |
| --- | --- | --- |
| 请求 | 两项目均指向 `POST https://www.perplexity.ai/rest/sse/perplexity_ask`，JSON body | 固定HTTPS origin/path，无携密重定向，不转API endpoint |
| 材料 | Cookie，浏览器User-Agent/Referer，Accept text/event-stream | 专用Web材料，不以Bearer或cf_clearance单独证明用户已登录 |
| 版本 | 两参考观察值 `2.18` | 非稳定官方版本承诺；失败不能自动拼接猜测字段重试收费请求 |
| 输入 | `query_str`；`params`含model_preference/mode/sources/frontend UUID/context UUID/attachments/language/version | 先冻结实际支持的单轮文本surface；不以字符串拼接冒充多轮、工具或附件支持 |
| 输出 | SSE JSON；blocks/markdown/diff或旧嵌套text JSON | 专用解析owner，答案、引用及终态分别处理，不把任意JSON文本当答案 |
| 终止 | helallao client要求 `event: end_of_stream`，require_complete下须有可用最终答案；提前EOF报错 | 不能照搬edusu的EOF直接返回积累答案；实际final_sse_message与end_of_stream组合仍待录制核实 |
| 目录 | edusu `_models.py`观察 `GET /rest/models/config?config_schema=v1&version=2.18&source=default` | 目录不证明账号可用模型或剩余额度，不自动注册全部模型可调用 |

精确来源文件均属于上表 pinned commit：

- edusu `src/perplexity/_core.py:21-25,90-137`：origin/version/JSON POST/Cookie/SSE；`:137`会日志输出原始片段，不照搬。
- edusu `src/perplexity/_chat.py:60-109`：请求body；`:112-181,218-253`：markdown/chunks/diff、web_results和continuation材料。
- edusu `src/perplexity/auth.py:21-38,74-96,148-151`：storage state Cookie提取；其session-token或cf_clearance任一存在的宽松条件，不作为Gateway认证成功判定。
- helallao `perplexity/config.py:13-20`：消费端origin/path及session endpoint；`client.py:278-315`：请求body；`:338-396`：终态、截断错误与finally关闭response。
- helallao `perplexity/utils.py:273-345`：旧text JSON中的FINAL/content.answer和blocks提取。

两个参考的blocks intended_usage分别出现 `ask_text_0_markdown` 与 `ask_text`，请求字段和解析行为也有差异。它们是易变/兼容性线索，不构成任意合并wire的依据。JSON Patch操作、索引、replace/append与引用归属需按明确格式处理，不能简单累计所有字符串。

## 3. 材料、额度与资源不变量

1. 导入完整Cookie或storage state时，必须核实domain/path/secure/expiry和分片，不能把全profile Cookie扁平化后发给任意origin；Cookie不得进入公开配置、日志和错误。
2. `__Secure-next-auth.session-token`是已观察的登录材料名称；`cf_clearance`只作可能的challenge材料，不单独代表账号身份。`/api/auth/session`的存在不证明已实现refresh。
3. followup的`read_write_token`属于会话访问材料，不按普通公开元数据存储或回显；上下文ID不可用于跨账号复用会话。
4. helallao内部把有Cookie的本地copilot/file_upload计数设为`inf`，不是平台无限额度；不能用于展示、路由准入、调度或授权。Web与API的关系继续unknown隔离。
5. 401/403、HTML/challenge、429和截断分别分类；不把HTTP200、Cookie存在、目录或部分答案判为成功，不降级到Sonar/Search或换账号伪造成功。
6. Gateway自有HTTP/SSE owner应有总deadline、decoded字节/单事件/答案/引用/patch数量上限、取消与response清理；这些是本地安全预算，不冒称官方上限。生成不自动重提。
7. 不引入临时邮箱自动建号、第三方账号服务或整套浏览器/CLI运行时；若真实平台必须依赖额外worker，再准确声明依赖和退出/打包合同。

## 4. 尚未完成与实施入口

2026-11-12 更新：已联网取得另一 MIT 参考的已合并 patch 修复及当前协议源码，明确根 replace/chunks add 样例，同时确认 schematized/workflow 与多种终态并存。旧“没有明确 op/path 参考”停点已解除，但尚未接入生产解析器；不能照搬参考的 EOF 成功兜底。固定 commit、文件 hash、可实施子集和负向条件见 [schematized 协议调查](provider-perplexity-schematized-contract.md)。

本次仅消除“没有可定位公开Web协议参考”的部分缺口，没有新增preset、模板、材料表单、transport、parser或测试绿灯。生产实现应沿三适配器和现有SecretPatch/CAS链，不重起网关、不借用平台API池。

后续先以明确可证的请求/响应profile增加独立request/parser/transport和合成loopback测试，再接正常空池模板、材料保存、精确账号probe和公共pipeline。合法账号录制用于核实session/challenge、终态组合、模型权限、额度及上游漂移，不用于替代可提前完成的资源和错误测试。

多轮、Research、附件、tool、模型选择及最终产品能力必须逐项说明；公开参考未证明的能力不虚构请求凑齐绿灯，也不把本合同或单轮切片算作R12全部完成。集中录制、真实消费端可用性与厂商授权边界仍未验证，总目标保持active。

# 普通 API 渠道实施前合同：Qwen / Bedrock

> 2026-11-02 联网只读核实；没有使用实际 Key、授权、生成或刷新。属于 [总计划 P00/P10](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/plan/2026-11-02-provider-template-expansion.md)。
> Qwen 原生目录及其管理/UI、运行边界、普通 preset/模板、账号目标/Key、父级默认目标编辑和精确 probe 正式发送已分批实施；公共 buffered、原始 stream 终态及 Stream 取消切片已有定向离线证据，probe 本地取消 owner 见 P19，公共 buffered 外层取消与 admission/terminal owner 见 P20；PG 审计 principal、PG source reserve 动态取消和完整保真仍缺，不能据此声明 W05 完成。Bedrock 已实现的认证切片另见专门合同。

## 1. Qwen 普通 API 不等于 Coding/Token Plan

官方 [base URL](https://www.alibabacloud.com/help/en/model-studio/base-url) 和 [regions](https://www.alibabacloud.com/help/en/model-studio/regions) 说明 Key、endpoint、模型表按地域隔离；workspace 专用域名要求同 workspace 的 Key。
普通按量 API、Coding Plan、Token Plan 使用匹配的专用 Key/base，不自动互换、转发或回退。Coding/Token Plan 限交互式编码工具，官方不允许将它作为普通 backend service 的通用替身。
Qwen Web 仍是独立 session 产品；不因普通 API 存在而删除 Web，也不以 Web token 冒充 OAuth。
普通 API 新 preset 方向为 `qwen-official-api`，不复活退役的 `qwen` / `qwen-dashscope-openai`；模板地域/工作空间输入必须走正常 UI，不从可改 Provider ID 推断。

## 2. 地域与精确目的地

六个一致有据的 workspace OpenAI-compatible base：

| region | base |
| --- | --- |
| 北京 | `https://{WorkspaceId}.cn-beijing.maas.aliyuncs.com/compatible-mode/v1` |
| 新加坡 | `https://{WorkspaceId}.ap-southeast-1.maas.aliyuncs.com/compatible-mode/v1` |
| 东京 | `https://{WorkspaceId}.ap-northeast-1.maas.aliyuncs.com/compatible-mode/v1` |
| 法兰克福 | `https://{WorkspaceId}.eu-central-1.maas.aliyuncs.com/compatible-mode/v1` |
| 弗吉尼亚 | `https://{WorkspaceId}.us-east-1.maas.aliyuncs.com/compatible-mode/v1` |
| 香港 | `https://{WorkspaceId}.cn-hongkong.maas.aliyuncs.com/compatible-mode/v1` |

shared legacy 推理入口中，北京 `dashscope.aliyuncs.com`、新加坡 `dashscope-intl.aliyuncs.com`、香港 `cn-hongkong.dashscope.aliyuncs.com` 文档一致。
US shared `dashscope-us.aliyuncs.com` 在 base-url 与 regions 页面存在矛盾，不作为无说明默认入口；US workspace host 的证据一致。
显式选择地域，Key 不跨区发送；目录 source 必须与已保存的地域/workspace endpoint 精确绑定。endpoint region 不是全部实际 inference deployment location 的证明。
不把所有地域模型做一个共享 union，不因目录失败自动向另一地域或 Plan host 重试。

## 3. 官方模型目录是 native 分页，不是通用 OpenAI parser

官方 [List models](https://www.alibabacloud.com/help/en/model-studio/list-models) 页面标示更新于 2026-10-27：

```text
GET /api/v1/models
Authorization: Bearer {API_KEY}
page_no=1
page_size=20
providers=qwen
supports=inference

output.total
output.page_no
output.page_size
output.models[].model
```

必须按分页取齐声明范围，不能读第一页就冒称完整。官方未给本轮可核实的 page_size 最大值，不自造 1000；实现须有总 deadline/总请求数/总字节/模型数/页数预算，不只有每页上限。
文档明确的新加坡目录源为 `https://dashscope-intl.aliyuncs.com/api/v1/models`，香港为 `https://cn-hongkong.dashscope.aliyuncs.com/api/v1/models`；北京、东京、法兰克福、弗吉尼亚列 workspace 对应 host 的 `/api/v1/models`。
不能把“workspace 推理 host 有文档”直接等同于“每个地域/入口成功目录响应已验证”；实施 endpoint descriptor 时必须保留 source 支持边界。

普通目录包含平台多厂商、多模态，不表示所有条目都能走当前 Chat adapter。渠道若声明 Qwen 聊天系列，应有对应 providers/capabilities 策略，不把图像/视频/embedding 全部生成聊天 route。
2026-11-02 再次联网核实的官方文档已明确：重复 `capabilities=TG&capabilities=Reasoning` 是 OR（文本生成或推理）。当前目录 reader 使用该筛选，并要求返回条目明确支持 Text 输入／输出；不宣称包含全部模态。目录也不证明 Key entitlement、免费额度或真实调用成功。

四次无凭据 GET：北京/新加坡的 `compatible-mode/v1/models` 及 `api/v1/models` 均返回 401。它只证明入口要求认证，不证明成功 schema 或 compatible `GET /models` 的正式支持。
本轮未找到官方 compatible `GET /models` 成功合同；不得因 401 错误长得像 OpenAI 就套 `data[].id`。

## 4. 本仓接缝与实施要求

- [preset/qwen.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/preset/qwen.rs)、[registry](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/preset/registry.rs)：P13 新增 `qwen-official-api`，沿 `line-qwen-official-api` gate；保留 Coding Plan 两格式及 Web，不伪造 OAuth。
- [model_catalog.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/provider_discovery/model_catalog.rs)：沿现有 bounded/no-redirect HTTP client、新增 native 分页 parser；不要把所有模型塞进 OpenAI data[] 分支。
- [Qwen 管理边界](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/http/routes/internal_console/qwen_catalog.rs)：management + Secret Grant + slot + 精确 credential；fetch 后再验 grant/revision，返回 no-store 元数据，不直接保存。Stored 请求不接受目标或 Key；Draft 必须显式新 Key 和经 descriptor 验证的 `accountBaseUrl`，不能指定 source 或任意携密 URL。
- [officialModelCatalog.ts](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/apps/desktop/src/features/console/officialModelCatalog.ts)：fixed-source 旧目录保持兼容，普通 Qwen 使用六区动态 descriptor，来源与精确有效账号目标一起验证；固定来源不能通过未知字段剥离接受 Qwen binding。
- [useAccountDiscovery.ts](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/apps/desktop/src/features/console/useAccountDiscovery.ts)：复用 cancellation、grant 和 editor/revision fence；目录不调用生成探测，失败不覆盖旧目录/manual 模型/route 优先级。

六区 workspace descriptor、原生有界分页、额外推理 base 绑定与离线测试见 [P10 回执](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-02-provider-channel-expansion-p10.md)。SG/HK 目录使用 shared origin，仅 source URL 不足以隔离 workspace，因此 Qwen `ModelCatalog` 另要求精确 `accountBaseUrl`；旧 fixed-source 数据不新增序列化字段。
普通产品的发送前保护、固定 Responses→Chat 桥接与 sender 不跟随重定向见 [P11 回执](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-02-provider-channel-expansion-p11.md)。内部 `qwen_workspace_api` auth-mode marker 只保留产品身份，实际仍是 Bearer API Key，不是 OAuth 或新额度渠道；精确 workspace raw payload 也执行同一保护。P11 当时未注册 preset，后续正常添加入口见 P13。
普通模板开放前的 Key 保存受众安全前置见 [P12 回执](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-02-provider-channel-expansion-p12.md)：普通 keep 不授予旧 Key 换产品/workspace/地域/消费者的权限，不能仅靠 runtime 检查代替 old→new 授权。
[P13 回执](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-02-provider-channel-expansion-p13.md) 接通六区创建、账号继承/override、原始 preset 锚定的最终配置检查和普通 Key staged binding；不设置默认地域、模型或账号。只有非执行 parent 可留空目标，每个最终 credential（含 disabled）必须合法；清 Key 可保存但不能发送。
[P14 回执](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-02-provider-channel-expansion-p14.md) 接通原生目录 management、动态来源读写和正常账号拉取/刷新。旧目录不丢失或转成 manual，新材料目录以临时 receipt 绑定当前 Key/身份/目标；目录可见不等于生成成功或额度证据。保存仍走正常 SecretPatch/CAS，撤权、取消和 stale 结果不能覆盖草稿。
[P15 回执](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-02-provider-channel-expansion-p15.md) 接通已有 provider 父级默认目标的正常卡片入口。显式确认后，原继承账号固定为旧有效目标，原 override 不变；不重绑旧 Key，不删目录或将 fetched 洗成 manual。空 parent 且无继承账号可首次设置；不迁移 provider-default 秘密。
[P16 回执](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-02-provider-channel-expansion-p16.md) 将普通产品精确账号及测试计划接到正式 prepare/planner/header/sender，无 request 明确 unsupported、不发 compatible `/models` 或隐式生成。probe 单独进行 64 KiB 有界 Chat JSON 完成判定与脱敏；这不是完整 `execute_*` 或外部 pipeline 验收。Qwen 编辑器仍不调用 generic discovery 或借用其他产品的目录。
[P17 回执](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-02-provider-channel-expansion-p17.md) 处理普通 Qwen 公共 Chat/Responses buffered：复用共享 64 MiB 本地 decoded-body 预算，产品校验后只使用 Chat decoder，不按入站端点 fallback；真实 router 检查输出、精确账号、模型、usage/audit。共享 builder 的 content_filter 不再伪装 stop；最终执行证据由回执记录。
[P18 回执](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-02-provider-channel-expansion-p18.md) 已接入转换前的普通 Qwen 原始 Chat SSE guard：delta 继续实时流动，finish/最终 usage/DONE 有界暂存到严格 HTTP body EOF；补 Chat cache details，真实 router Stream 取消/连接/audit 的定向离线验证已通过（233 项，含生产 integration）。P18 当时留存通用 Responses 两项脱敏失败，后续修复见 P21，不能说完整 suite 全绿。共享 recording 的 probe 本地外层取消 audit 后续见 [P19 回执](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-03-provider-channel-expansion-p19.md)：独立32槽、create task返回guard、finish提交后保留原终态、连续drain及真实管理双case plan取消，最终101项定向通过。
[P20 回执](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-03-provider-channel-expansion-p20.md) 已补公共 buffered 外层取消：完整 Owner 跨 admission task及未领取output、正常terminal提交后等待者Drop不二退、SSE同步handoff；真实公共Chat/Responses中性取消及受影响生产integration等147项定向通过。独立128槽覆盖admission/生成/终态存储，是新的全局backpressure；入场使用当前初始budget，不限制全部排队请求体内存。Cash未发送释放、未知费用保留reservation，不把取消当成功零用量。PG probe因缺principal仍在网络前拒绝；PG auto-route source reserve动态取消、reasoning/refusal/多choice等保真继续开放。不能以目录、probe或当前子链成功把W05标offline_ready。
[P21 回执](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-03-provider-channel-expansion-p21.md) 已实际复现并修复 P18 两项通用 Responses 脱敏失败，完整 Responses 57 项、相邻及生产合同合计 167 项定向通过。只固定 CompletionError/聚合/Responses→Chat 的错误对象出口，精确结构化码保留现有健康分类，ServerError/retry/fallback 不变；native 原始失败 bytes 仍透明传递，stream audit 模式诊断未改。不是 W05/W06 整体完成或任意 wire 全量脱敏。
不能为了减少改动只做一个地域却不说明其余范围，也不能为未知成功 wire 造 mock 后称已完成真实验收。

## 5. Bedrock

认证、region、模型路径、旧配置兼容和明确缺口见 [Bedrock 认证合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/provider-bedrock-auth.md)。
Bearer API Key 与 IAM SigV4 是认证路径，不默认拆两份额度；temporary IAM 是多字段材料子类型，不是 OAuth 或新额度产品。
第 7 批已接正常空池模板、账号级模式、多字段与已有 SecretPatch/YAML/配置编译 owner，见 [P07](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-02-provider-channel-expansion-p07.md)。
精确账号原生 probe、HTTP labels 修补见 [P08](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-02-provider-channel-expansion-p08.md)；长模型身份的编辑/重开/编译/映射/手动及自动测试接线见 [P09](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-02-provider-channel-expansion-p09.md)。共享公共取消owner和原生pipeline交叉回归见P20；普通参数桥接、完整reasoning/signature、Bedrock专属完整取消audit与集成包仍缺，不能把材料或wire子链冒称全部功能闭环。

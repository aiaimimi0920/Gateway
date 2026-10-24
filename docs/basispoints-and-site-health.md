# Basispoints 可选通道与独立站点健康

这两项能力分别属于调用通道和站点控制面。站点监控不测试账号，不发推理请求，
不会写入账号测试结果、账号 breaker、额度或计费记录。

## Codex 凭据的可选通道

在账号编辑窗口的「Codex 调用通道」中选择「继承服务商设置」「原生 Codex」或
「Basispoints」。服务商级 `codex_basispoints_enabled` 默认 `false`；凭据级同名字段
为可选布尔值，显式 `false` 可以覆盖服务商级 `true`。删除凭据级字段即恢复继承。
新建 Codex 服务商时也有 Basispoints 选项。

仅原生 `https://chatgpt.com/backend-api/codex` 凭据可启用；要求包含 OAuth access token
和 `Chatgpt-Account-Id`。新通道仍复用原凭据 ID、账号 ID 和现有刷新/持久化 owner，
不复制账号、不注册 Go 插件、不改原始凭据的 `base_url`。关闭后恢复原生执行器。
同一个模型名按所选通道发送，不要求添加 `-basispoints` 模型别名。

启用后固定请求 `https://bps.openai.com/basispoints/api/responses`，禁止重定向，不发送
原生 Codex 的任意附加认证 header。请求使用产品协议和 Excel profile；客户端
function/custom/namespace 工具目录转为受限说明，`run_officejs` 仅作为传输封装。
Gateway 不执行 OfficeJS。响应必须校验工具目录、工具类型、call ID、tool_choice、
parallel 限制和支持的参数 schema 子集，然后才还原客户端工具调用。

参数校验支持 type、required、properties、additionalProperties、items、enum、const、
本地 `$ref`、allOf/anyOf/oneOf，并限制递归深度和工作量；它不是完整 JSON Schema
引擎，不能声称验证了所有自定义语法或 schema 关键字。历史调用按本次完整目录
无状态重建外层封装，不使用进程全局 call-ID 缓存，也不保证保留旧外层参数的逐字表示。

流与非流都要求有效的 `response.completed`，不能将断流或失败流当成功。原生
Responses 非流输出保留 reasoning、自定义工具和 namespace；Chat 等入口继续走
框架已有的 Responses 转换。流式响应先完整缓冲并校验，再按标准事件回放，
**不是实时低首字延迟流**。成功响应上限 16 MiB，错误响应沿用既有有界错误读取策略，
输出 item/content 总数上限 4096。

### 本版明确边界

- 支持文本以及 function/custom/namespace 工具；图片、文件、音频当前明确报不支持，
  没有静默删图，也没有实现 Basispoints 专用 attachments 上传。
- 不支持 `previous_response_id` 隐式续接和后台任务；需要显式历史。
- 固定 `store:false`、`model_selection:explicit`；保留已支持的生成约束，让上游对
  可能不支持的约束显式报错；空 `context_management` 数组按已知协议省略。
- 是否有模型权限、产品通道是否临时变化，以及实际回答质量，需要真实账号另行验证。
  不能保证切换后更聪明，也不绕过额度或账号权限；本轮没有执行真实模型调用。
- 编译时需要 `line-chatgpt-codex-oauth-official`，配置编译和执行器都检查该开关。

## 独立站点健康与自动模板

2026-10-24 起，主界面按 **New API、sub2api、One API、类型未确认的兼容网关** 选择通用适配器；
具体性能摘要、slot、embed 等作为内部读取器自动协商，不为每个部署站点增加独立变种。
官方服务保留独立适配器。旧配置与认证范围兼容规则见
[网关适配器与内部读取器](site-health-gateway-adapters.md)。下表描述内部读取方式，而非顶层选择项。

官方站点的固定地址、无需认证自动填充、独立读取方式和 ChatGPT Codex API 特例见
[官方站点健康适配器](official-site-health.md)。官方分类状态不伪造模型成功率。

在「凭据池」服务商卡牌背面点击「站点健康」，进入该站点的独立弹窗并启用。新建自定义站点默认开启；
已有站点保持原配置，需单独启用。保存走既有 route document 的校验、revision/CAS
与持久化，不另建站点数据库。后台默认自动识别；「立即识别接口」
用于手动刷新已保存的策略，未保存的策略不会被拿去检测。

2026-10-23 已接入多检测方案、凭证组绑定、手选适配器、监控专用认证及专用编辑表单。
可为 Plus/Pro 分别新增方案，或在唯一方案中选「全部分组共享一次检测」；方案支持多选本地组。
认证输入是密码框，需现有敏感信息授权，支持保留、替换、清空和撤销待提交操作。
「控制策略」选择仅提示或低于阈值禁用；检测地址、适配器、远端分类、模型映射、
刷新间隔、有效期及未知策略均在弹窗内配置。卡牌正反面尺寸不随表单长度改变。
实现与离线浏览器证据见 [第二阶段](progress/2026-10-23-site-health-profiles-phase2.md)。

| 自动模板 | 请求 |
|---|---|
| `new-api-perf-summary` | GET `/api/perf-metrics/summary?hours=24` |
| `model-status-all` | GET `/api/enhancements/model-status/embed/status/all` |
| `channel-health` | GET `/api/channel/health/models?days=1` |
| `model-status-embed` | GET `/api/model-status/embed/config/selected`，再 POST `/api/model-status/embed/status/batch?window=...` |
| `model-health-slot` | GET `/api/model_health/slot_status?window=6h&group=...` |
| `sub2api-v2` | GET `/api/v1/channel-monitor-v2/matrix?range=24h&group_by=platform_group_model`，可重复 `group_id` |

embed 的 POST 只是读取状态，不是生成请求；仅保留 selected 模型，并去重，优先使用
selection 返回的窗口。slot 必须给出明确监控分组，缺少时标记 `parameters_required`，
不会伪造 default 分组。其余模板仍照常检测。自定义 JSON 没有可枚举的固定接口，
本版不进行任意路径扫描，也不会借用账号密钥获取私有监控页。
旧模板 ID 严格只运行该读取器；新系统级适配器在有限能力集合中选择读取器。
监控认证需要明确选择系统或保留旧固定读取器；自动模式只做公开探测，
系统级认证也最多选择一个读取器，不向多个模板广播密钥。New API 与 sub2api 的接口已核验官方源码；其余四项为兼容模板，
不代表官方 One API 或所有第三方站点都支持。证据见
[适配器接口调查](plan/2026-10-23-site-health-adapter-evidence.md)。

识别要求正确 JSON/schema，HTTP 200 或 HTML 页面不算成功。401/403 表示需授权，
不等同站点故障；404/405、不合法 JSON、无样本、超时等分别记录。多个模板均可用时，
选择有效模型证据最多的一种；相同数量按表中顺序优先，不混合不同统计口径。
每种模板保存识别状态与模型数，长驻缓存仅保留所选模板的一份模型表。

`channel-health` 跨渠道先汇总计数，再计算技术成功率：
`(success_count + moderation_count) / (test_count - client_error_count) * 100`。
缺少可选 client/moderation 计数按零处理；无效/不一致计数不作为健康证据。
其他模板的重复模型、无样本和停供状态不能被当作健康。

### 多检测方案与凭证组

旧的平铺 `site_health` 配置继续可读；没有 `profiles` 时仍是全服务商的一份监控。
显式配置 `profiles` 后，仅运行其中的方案，平铺策略不再作为隐式 fallback。
每服务商最多 16 份方案，全实例最多 64 份实际方案。每方案具有稳定 `id`、显示名 `name`、
`account_group_ids` 和完整 `policy`；多个本地组绑定同一方案，只检测一次并共享结果。

`account_group_ids` 引用本地凭证组 ID；`monitor_groups` 是远端接口分类，二者绝不互换。
本地组可分别绑定不同方案；一组不能同时绑定两份方案。空 `account_group_ids` 表示全组共享，
此时该方案必须是服务商唯一方案。显式多方案中未绑定的组不受其他组的监控结果限制。
同一凭证若同时属于多个已绑定组，必须通过所有适用方案的限制。

配置片段（需先存在相应本地凭证组；此例仅展示配置合同，不是已上线数据）：

```yaml
site_health:
  profiles:
    - id: plus
      name: Plus
      account_group_ids: [local-plus, local-shared]
      policy:
        adapter: sub2api-v2
        mode: restrict
        minimum_success_rate: 95
        monitor_groups: ["1"]
        auth: { kind: bearer, token: null }
    - id: pro
      name: Pro
      account_group_ids: [local-pro]
      policy:
        adapter: sub2api-v2
        mode: restrict
        monitor_groups: ["2"]
        auth: { kind: bearer, token: null }
```

New API 的 summary 为全站汇总。指定远端分类时，先取模型目录，再通过
`/api/perf-metrics?model=...&group=...&hours=24` 核对精确分类；每轮最多 16 次详情请求，
超出预算的模型保留未知，报告 `partial_coverage`，不会拿全站成功率填充。
`channel-health` 和 `model-status-embed` 没有已确认的分类身份，指定远端分类时标记
`group_scope_unsupported`。sub2api 使用正整数 group ID，成功率从 0–1 转为百分比；
脱敏计数为零不等于没有样本，`health.overall: unknown` 与缺失分类保持未知。
其 coverage/聚合延迟元数据保留在探测报告中。

### 监控秘密字段

认证只支持独立 Bearer 或 `x-api-key`/`api-key` header，不使用业务 API key/cookie。
sub2api 监控通常需要面板 JWT，而不是模型调用 key。管理草稿必须保持 token 为 null/省略，
使用既有 SecretPatch 的 Keep/Replace/Clear；新增路径为：

- `/providers/{i}/site_health/auth/token`
- `/providers/{i}/site_health/profiles/{j}/policy/auth/token`

普通配置回显与站点健康列表不包含 token。Keep 同时按 provider ID、方案 ID 映射，重排不串密钥，
删除不遗留密钥，同槽位换 ID 不能继承。已配置 token 的 origin、adapter、认证方式/header 或
私网许可改变后不能 Keep，必须显式 Replace/Clear 并通过敏感字段授权。
快照 binding 使用仅进程内随机盐，避免变成公开的密码猜测摘要。
存储复用现有路由秘密字段机制，不宣称新增了静态加密 vault。

### 决策与安全边界

- 默认 `mode: advisory`，仅显示信息，不拦截请求。
- 显式 `mode: restrict` 按 `minimum_success_rate` 检查真正要发送的上游模型，
  可设置「实际上游模型=监控模型」映射；不是按客户端别名盲匹配。
- `unknown_policy` 默认 `allow`，可改为 `block`。缺模型、无效成功率、过期、
  时间倒退或配置变化均属于未知。advisory 始终不阻止请求。
- `max_age_secs` 表示本地获取结果的有效期，不保证站点底层测试样本一定新鲜。
- 路由路径只读内存缓存，不做 HTTP 探测。在池内挑选、映射随机目标选择和软性排序前先排除，
  发送前再次复核；不会记作账号调用失败、不消耗模型请求 attempt，并保留 fallback 的候选索引。
  健康恢复只解除自身限制，绝不改写人工 enabled 或模型禁用状态。
- 结果与配置绑定；配置修改后旧结果不可用于新策略，进行中的旧请求不得覆盖新结果。
  结果只在当前进程内缓存，重启后按 unknown 策略处理，后台重新检测。
- 最多 64 份实际监控方案，每方案最多 4096 模型；默认每 300 秒刷新，结果有效期
  900 秒。当前方案任务串行，内部最多两个并发接口，每方案总检测时限 25 秒；
  慢站点多时可手动调整刷新间隔/有效期，不承诺全量扫描固定耗时。
- HTTP 请求只指向已配置监控站的 origin 根路径，不转发 provider key/cookie，
  禁用系统代理、重定向，解析 DNS 后校验并固定地址；每个响应最多 1 MiB。
  默认禁止内网和特殊地址；内网站点需显式允许，元数据/link-local 等地址仍禁止。
- 关闭面板停止 UI 轮询；切换连接/管理 token、取消及关闭时丢弃过期异步结果。

管理接口为 GET `/v1/internal/gateway/site-health` 和
POST `/v1/internal/gateway/site-health/:providerId/refresh`，均需要现有管理鉴权，
响应 `Cache-Control: no-store`。显式方案使用
POST `/v1/internal/gateway/site-health/:providerId/profiles/:profileId/refresh`，旧刷新入口不会
随意选一份方案；手动识别受忙碌保护和同方案身份 30 秒间隔限制，改策略不绕过该限制。
列表保留旧 `snapshot` 字段，并新增每方案的 `profiles` 观测；显式多方案不伪造全站汇总。

真实调用失败的短时临时硬熔断与本站监控限制是两层独立准入，见
[智能路由](smart-routing.md)。公开 `GET /v1/models` 的本地/YAML 和 PostgreSQL access-key/project
路径均已接入硬准入；PG 保留原授权 provider、具体凭据和实际上游模型身份，过滤后再计算别名遮蔽，
不会借全局同名模型扩大权限或为了列目录消耗试放/round-robin。实现与离线回归见
[第三阶段](progress/2026-10-23-site-health-catalog-phase3.md)；未执行真实 PG 服务集成验收。

本轮没有升级正在运行的 CPA 或 Gateway，也没有自动提交/推送或发布新安装包。

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

在「账号」页面展开「站点健康」，选择站点并启用。新建自定义站点默认开启；
已有站点保持原配置，需单独启用。保存走既有 route document 的校验、revision/CAS
与持久化，不另建站点数据库。后台自动识别，不需要用户选择模板；「立即识别接口」
用于手动刷新已保存的策略，未保存的策略不会被拿去检测。

| 自动模板 | 请求 |
|---|---|
| `new-api-perf-summary` | GET `/api/perf-metrics/summary?hours=24` |
| `model-status-all` | GET `/api/enhancements/model-status/embed/status/all` |
| `channel-health` | GET `/api/channel/health/models?days=1` |
| `model-status-embed` | GET `/api/model-status/embed/config/selected`，再 POST `/api/model-status/embed/status/batch?window=...` |
| `model-health-slot` | GET `/api/model_health/slot_status?window=6h&group=...` |

embed 的 POST 只是读取状态，不是生成请求；仅保留 selected 模型，并去重，优先使用
selection 返回的窗口。slot 必须给出明确监控分组，缺少时标记 `parameters_required`，
不会伪造 default 分组。其余模板仍照常检测。自定义 JSON 没有可枚举的固定接口，
本版不进行任意路径扫描，也没有借用账号密钥获取私有监控页的能力。

识别要求正确 JSON/schema，HTTP 200 或 HTML 页面不算成功。401/403 表示需授权，
不等同站点故障；404/405、不合法 JSON、无样本、超时等分别记录。多个模板均可用时，
选择有效模型证据最多的一种；相同数量按表中顺序优先，不混合不同统计口径。
每种模板保存识别状态与模型数，长驻缓存仅保留所选模板的一份模型表。

`channel-health` 跨渠道先汇总计数，再计算技术成功率：
`(success_count + moderation_count) / (test_count - client_error_count) * 100`。
缺少可选 client/moderation 计数按零处理；无效/不一致计数不作为健康证据。
其他模板的重复模型、无样本和停供状态不能被当作健康。

### 决策与安全边界

- 默认 `mode: advisory`，仅显示信息，不拦截请求。
- 显式 `mode: restrict` 按 `minimum_success_rate` 检查真正要发送的上游模型，
  可设置「实际上游模型=监控模型」映射；不是按客户端别名盲匹配。
- `unknown_policy` 默认 `allow`，可改为 `block`。缺模型、无效成功率、过期、
  时间倒退或配置变化均属于未知。advisory 始终不阻止请求。
- `max_age_secs` 表示本地获取结果的有效期，不保证站点底层测试样本一定新鲜。
- 发送路径只读内存缓存，不做 HTTP 探测。限制发生在共享候选循环、任何实际 attempt
  之前；不会记作账号调用失败，不消耗模型请求 attempt，也保留 fallback 的候选索引。
- 结果与配置绑定；配置修改后旧结果不可用于新策略，进行中的旧请求不得覆盖新结果。
  结果只在当前进程内缓存，重启后按 unknown 策略处理，后台重新检测。
- 最多 64 个启用监控的站点，每站最多 4096 模型；默认每 300 秒刷新，结果有效期
  900 秒。当前站点任务串行，各站点内部最多两个并发接口，总检测时限 25 秒；
  慢站点多时可手动调整刷新间隔/有效期，不承诺全量扫描固定耗时。
- HTTP 请求只指向已配置监控站的 origin 根路径，不转发 provider key/cookie，
  禁用系统代理、重定向，解析 DNS 后校验并固定地址；每个响应最多 1 MiB。
  默认禁止内网和特殊地址；内网站点需显式允许，元数据/link-local 等地址仍禁止。
- 关闭面板停止 UI 轮询；切换连接/管理 token、取消及关闭时丢弃过期异步结果。

管理接口为 GET `/v1/internal/gateway/site-health` 和
POST `/v1/internal/gateway/site-health/:providerId/refresh`，均需要现有管理鉴权，
响应 `Cache-Control: no-store`；手动识别受忙碌保护和同配置 30 秒间隔限制。

本轮没有升级正在运行的 CPA 或 Gateway，也没有自动提交/推送或发布新安装包。

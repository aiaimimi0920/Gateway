# Key 总花费与用户现金结算

## 口径与金额

“总花费”限制该 Key 在启用现金计费期间累计产生的用户应付金额，不是 Token 数量、
供应商余额差、IQ 花费或卡片的成本估算。保留原有“不限额”“请求次数”“Token 总量”。

```text
单次应付 = 实际输入/输出用量对应的模型费用 × 有效权益组倍率 × 实际账户倍率
可用额度 = 总额度 − 已结算金额 − 在途或待核对请求的预占金额
```

币种固定为 USD，不自动换汇。内部以整数 micro-USD 保存，1 USD = 1,000,000 micro-USD。
模型单价沿用 `promptMicrosPer1kTokens`、`completionMicrosPer1kTokens`，单位是每千 Token
的 micro-USD；倍率最多六位小数，范围为 0 到 1,000,000。乘法使用检查溢出的整数计算，
每个请求只在最终金额处向上舍入一次到 0.000001 USD。显式零单价或零倍率允许免费；
缺失、负值、超界或精度不可表示的价格不允许被当作零。

例如 1,000 个输入 Token、输入价 2 USD/千 Token、权益倍率 1.5、账户倍率 0.8，
最终结算 2.4 USD。输出费用先与输入费用相加，再应用相同倍率。

## 价格与权益的唯一来源

- 使用路由实际解析的上游模型，不使用界面别名计算价格。
- 优先使用该模型显式配置的 `modelPricing`；未配置模型条目时可使用供应商显式配置的
  通用输入、输出价格。模型条目不完整则拒绝，不用其他价格补齐。
- 展示用的 `default_registry` 市场参考价格不进入现金账单。现金 Key 调用前必须配置
  完整模型价格。显示卡片上的估算金额不能用作充值、已用金额或结算依据。
- 请求未通过受信任的 `X-Neuro-Account-Group` 指定组时，从 Key 被授权、当前启用且包含实际账户的
  权益组中选择最低倍率；同倍率按组 ID 稳定选择。指定组时只使用该组。
  无权使用、禁用和不含账户的组不参与。未绑定组的旧 Key 在不指定组时使用倍率 1。
  此头保留现有管理转发认证要求，转发方须同时提供有效的 `X-Internal-API-Key`；
  普通 Key 不能自行伪造受信任的管理请求头。
- 账户倍率按实际 credential/account ID 查找，未配置时为 1，不使用供应商卡片的汇总倍率。
- 现有模型定价只有输入、输出两档，因此缓存 Token 使用输入价。OpenAI 已包含在输入
  总数中的缓存不重复计入；Anthropic 独立报告的缓存读取和创建 Token 加入输入。
  这里没有增加独立缓存折扣价、外部支付、充值或人工退款业务。

管理价格接口仍使用 POST，不是 HTTP PATCH：

```text
POST /v1/internal/gateway/provider-accounts/{providerAccountId}/model-pricing
```

```json
{
  "entries": [
    {
      "model": "configured-upstream-model",
      "promptMicrosPer1kTokens": 2000,
      "completionMicrosPer1kTokens": 10000
    }
  ],
  "accountBillingMultipliers": {
    "actual-account-id": "0.8"
  }
}
```

`entries` 可省略，以仅修改账户倍率；倍率值为 `null` 时移除该账户覆盖、恢复默认 1。
上述 ID 仅为格式示例，不表示真实账户。接口保留管理权限校验，不返回账户凭证。
账户卡片右上角的“账户计费倍率”按钮可读取并编辑该实际账户的倍率；留空保存会删除覆盖，
恢复默认 1，显式 0 表示免费。每次只提交所选账户，不覆盖其他账户或模型价格。
保存结果因网络问题无法确认时，必须先重新读取，不能把失败提示当作未写入的证明。
Key 金额上限仍可直接在 Key 编辑界面修改。

```text
GET /v1/internal/gateway/provider-accounts/{providerAccountId}/model-pricing
```

该管理接口仅返回 `accountBillingMultipliers`，倍率统一为六位小数字符串；不返回供应商
payload、API Key 或其他凭据，并设置 `Cache-Control: no-store`。旧服务器不支持该 GET 时，
界面明确报告读取失败并禁止保存，不猜测已有倍率是 1。

## 额度、预占与账单

创建和编辑 Key 使用同一个 `quota` 合同：

```json
{"mode":"cash_prepaid","limit":2400000,"currency":"USD"}
```

`limit` 为整数 micro-USD，范围 0 到 9,007,199,254,740,991。修改总额与 Key 元数据在同一
事务保存，不会清零已结算或预占金额。轮换后新 Key 指向同一个现金账户，旧在途请求仍然
结算到该账户；撤销、删除和重启不会把账单当作未发生。切出现金模式期间不追溯计费；
切回时保留此前现金消费。现金请求的结算独立于 Token/次数账户，不会扣到别的计量单位。

在第一个上游尝试前，固定本次候选的模型价格、权益组、账户和两层倍率，为最贵候选预占
额度。SQLite 使用 `BEGIN IMMEDIATE`，PostgreSQL 使用数据库事务和账户行锁；Redis
缓存不授权花费。价格和组倍率的后续修改只影响新请求。账单与现金余额由同一结算 owner
在同一事务写入；重复完成回调不会重复扣款。

管理查询：

```text
GET /v1/internal/gateway/access/keys/{id}/balance
GET /v1/internal/gateway/access/keys/{id}/cash-ledger
```

余额的可选 `cash` 字段包含 `currency`、`totalMicros`、`spentMicros`、`reservedMicros`、
`pendingRequests`。目录也返回相同余额。账单接口返回该现金账户最近 100 条记录，响应
`Cache-Control: no-store`，仅管理授权可读；包括固定报价、实际用量、金额及状态，
不保存请求正文、API Key 或供应商密钥。

Key 列表的“账单”入口只读展示最近 100 条现金记录，包含状态、预占金额、结算金额、实际
输入/输出及缓存用量，并可展开查看冻结的模型价格和两层倍率。未结算金额显示 `—`，
不会把未知用量视为 0；历史记录不会按当前价格重新计算。吊销、过期、切换额度模式后的
Key 仍可查看保留的现金记录；轮换后的账单可能包含旧 Key ID。该入口仅在服务端声明
支持现金额度时提供，不新增支付、退款或强制释放操作。

| 状态 | 含义 | 余额处理 |
| --- | --- | --- |
| `reserved` | 已预占、尚未获得最终结果 | 保留预占 |
| `settled` | 成功且取得完整用量和对应报价 | 按最终应付额结算，释放多余预占 |
| `unresolved` | 缺少或不完整的用量、流中断、取消后结果不确定 | 保留预占，不生成零元账单 |
| `released` | 确定未尝试上游 | 释放预占，金额为 0 |

进程被强制终止时可能留下 `reserved`；重启不会把它重置或自动退款。当前没有人工核账
或强制释放接口，不确定记录需要后续专门的核账流程。每账户最多 1,024 个待结算请求、
100,000 条账单，现金账户最多 10,000 个；达到容量会明确拒绝，不静默丢账或删除财务记录。

预占不是上游费用担保。若上游违反声明的输出上限，或报告的真实用量超过保守预占，
系统记录完整应付金额，允许显示负可用余额并阻止后续收费请求；不会截断账单来伪装未超额。

## 当前支持边界

现金限额先覆盖具有可核对实际用量的 `openai_compatible`、`anthropic_compatible`
直接 HTTP 文本调用，可从 Chat Completions、Completions、Messages、Responses 入口进入。
不向客户端静默注入默认输出限额。请求以及实际打包后的上游请求必须有正整数
`max_tokens`、`max_completion_tokens` 或 `max_output_tokens`，范围 1 到 1,000,000。

不支持多模态、隐藏历史 `previous_response_id`、多份生成 `n != 1`、`best_of != 1`，
或不能保留输出上限的适配路径。仅能提供估算用量的逆向/浏览器适配器不能作为现金账单
来源。候选数量须为 1 到 64，任一候选缺完整价格都会在调用上游前拒绝。
自动路由聚合 Key 不得借用现金限额源 Key 绕过其账户预占。

对上游输入量使用完整规范请求与打包请求的 UTF-8 大小、协议 framing 作保守预占，
不是把估算输入量用于最终账单。最终仅使用明确返回的实际输入、输出用量；不完整的
非流式报告、只有开始事件的 Anthropic SSE 用量均保持未知状态。

## 存储升级与回滚

本地 SQLite 启动事务增量建立 `gateway_cash_accounts`、`gateway_cash_keys`、
`gateway_cash_requests` 三表及索引，不改根 storage version、不重写旧凭证、旧 Token
余额或审计表。使用旧 schema-1 数据可直接升级，不需要用户手工执行 SQL。

服务端新库 initdb 包含 `deploy/postgres/initdb/004-gateway-key-cash.sql`。已有 PostgreSQL
库需要另行审核、备份并以事务执行该增量脚本；本次本地功能开发不会自动迁移真实服务器。
缺表的旧服务端返回 `cashQuotaSupported=false`，UI 不提供创建现金限额选项，
旧 Key 继续使用原有规则；不得把缺表解释成无限现金余额。

回滚二进制时保留新增表和完整数据，不执行 DROP 或清账。上一版 `key-editor-r1`
不认识 `cash_prepaid`，会拒绝该计量模式，不能继续负责现金 Key 的请求；其他旧 Key
仍按原规则运行。升级/回滚应先停止接收新现金请求并排空在途请求，避免跨版本并发管理
现金配置。SQLite 备份使用在线备份 API，或停止所有使用该数据根的 Gateway 后连同
WAL 状态一起备份，不复制仍在写入的单个主数据库文件。

测试入口为 `cargo test --lib cash_billing` 和 `cargo test --test cash_billing_contract`。
后者使用隔离 SQLite、真实管理路由和 loopback 上游，不调用真实供应商。PostgreSQL
部署、真实支付、真实供应商计价与全协议业务验收须单独报告，不能由本地测试推断。

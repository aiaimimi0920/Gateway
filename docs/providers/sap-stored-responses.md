# SAP 持久化 Responses：受控启用合同

本功能默认关闭，不代表三服务商已收口。真实 SAP 部署资格、费用和无法自动恢复的
交付不明仍是启用门槛。源码、双 EXE、桌面 UI 与真实租户证据分别记录。

## 配置与公共入口

- 仅 SAP 原生 Responses 部署可同时开启 `sap_native_responses` 和
  `sap_stored_responses`；凭据不能自行授予部署能力。不自动探测或开启已有池。
- `POST /v1/responses` 显式传 `store:true`，支持 JSON 和可选的布尔
  `background`；不支持持久化 SSE、任意 conversation、远端 item/file 或内置工具。
  未选择持久化的 SAP 请求保持 `store:false`；非 SAP 原生 `store:true` 保持普通管线。
- 回复 ID 为 `resp_gw_<32 位十六进制>`。后续入口为
  `GET /v1/responses/{id}`、`POST /v1/responses/{id}/cancel`、
  `DELETE /v1/responses/{id}`。调用者不能直接提交 SAP 响应 ID。
- `previous_response_id` 只接受同一主体、同一公开模型、同一扣费来源与部署绑定的、
  已完成的 Gateway 回执；内部解析为原 SAP ID，不重新选账号。
- 输出投影保留受支持的文本、拒绝、函数调用和推理摘要，不原样转发 SAP metadata、
  私有模型名或顶层响应 ID。这不是完整 OpenAI Responses 字段兼容声明。

## 授权、发送与账务

每次操作重新验证原调用 Key、当前模型授权、原 Hosted 聚合成员、platform access、
部署、资源组与实际凭据。撤销原绑定后拒绝旧 ID，不借仍有效的备用成员操作旧对象。
新的独立请求仍可使用其他已授权成员。

创建和续接还核对实时生成资格，并在最终发送点原子领取同一候选的熔断／容量许可；
读取和清理不继承生成 cooldown。公共限流、provider attempt、总预算与发送审计保留，
不执行普通管线的第二次预扣、生成重试或候选回退。

外发前持久化 Gateway ID、归属 HMAC、路由 pin 和唯一预扣，不保存 Service Key、
输入 prompt 或上游回复正文。队列状态不结算；权威终态用量可在后续 GET 到达，
并与原预扣幂等结算。已知用量不可覆盖，未知用量不填零。当前不支持现金 tariff。
按 ID 读取、取消、删除不再扣一条生成额度；本地退款不表示 SAP 退回费用。

每进程最多 32 个耐久操作。HTTP 断开在发送前阻止发送；发送开始后后台任务继续
写回回执和账务，正常 drain 等待这些任务。强制终止或存储故障仍可能留下待处理项。
cancel/delete 先取得数据库单次发送资格；未确认操作不会因重启、超时或独立 GET
自动解锁。确认删除后保留墓碑，防止迟到结果重新发布对象。

## 恢复与尚未完成的运维能力

| 情形 | 当前处理 |
| --- | --- |
| 已知 SAP ID，结算失败 | 返回本地恢复 ID 与 `sap_aicore_response_settlement_pending`；终态及完整权威用量已落库时，后台自动重试本地幂等结算，也保留授权 GET 重试 |
| 已终态，尚无权威用量 | 尚未结算时，后续 GET 只接受同一持久终态的合法用量；token 预付和 failed 的有额预扣保持打开，不估算 tokens。`completed + message_prepaid + reserved=1` 且两项 tokens 均为 NULL 时，后台也可确认已扣一条消息并关闭 hold，不再扣款、不退款、不伪造用量；结算后不会再经该路径补录迟到用量 |
| 发送结果不明、没有确认的 SAP ID | 返回本地 ID，保留预扣；GET 不猜 ID、不重 POST，需原绑定的权威外部证据 |
| 确定未发送但退款失败 | 返回 `sap_aicore_response_refund_pending`；仅当 live submit owner 已独立提交匹配 revision 的未发送证明时，后台自动退款；证明写失败则保留待人工核实，不能仅凭库存状态推断 |
| 取消或删除已提交但未确认 | 保留互斥屏障，不自动重放；GET 终态不能证明原操作已确认 |

管理接口 `GET /v1/internal/gateway/console/sap-aicore-responses/pending`
要求有效的持久管理身份，拒绝普通调用 Key 和公开 passwordless marker，并返回
`Cache-Control: no-store`。响应为 `{"items":[...]}`，每页最多 100 条；满页后使用
末项 ID 作为 `?after=`，直至不足 100 条。返回回执状态和绑定引用，不返回秘密、
owner HMAC、扣费 Key 或 SAP ID。分页限制返回量，不保证扫描历史的成本固定。

此接口只是发现，不是人工结案 API。后台只消费匹配当前收据 revision 的未发送证明、
已有完整权威用量的终态，或上表严格限定的已完成单条消息预付。扫描只是候选，
事务内重读收据／hold 并再次限定无用量例外；不扩大到其他模式、零预扣或 failed。
退款／结算校验原余额账号与模式并关闭 hold；零用量须明确记录，未知用量不填零。
不会访问 SAP／OAuth、补查 GET、重放 POST 或解除不确定 mutation 屏障。

每个正常运行实例注册一个可回收 worker，启动即扫描，之后按 10 秒周期执行；
每批最多 32 条，keyset 游标跨批推进，坏行不阻塞后续项。单个数据库步骤最多等待
5 秒，不代表整批或驱动释放连接的硬上限。drain/shutdown 在各步骤之间协作式停止，
等待当前步骤结束；不将取消或提交回执不明视为事务必然失败，后续仍按耐久状态幂等重读。

未知提交的权威 ID 绑定流程和不确定 mutation 的人工处置入口仍未完成。
不得把这些剩余事项合并成“只差真实账号”。新增人工入口前还必须定义权威关联证据、
原 caller 的授权及跨实例在途 owner 互斥；仅管理员输入 ID 或 GET 对象存在，
不足以证明对象属于该次未知提交。无证明的人工额度返还不能写成未发送证明，
GET 终态也不能代替原次 cancel／delete 的确认。

已将错误／健康事实与交付确定性分开：推理 429／5xx 及明确协议失败进入精确
凭据／模型健康反馈；OAuth、普通 4xx 和传输不明保持中性，不伪造成功。
任何 HTTP 错误都不授予未发送退款或重放资格。必要回执／账务先写入，
其后健康写入单独限时；queued、GET 和清理成功不冒充生成恢复。
普通非持久化请求在 OAuth 后仅复查实时资格，不重复消费 provider 限流窗口。

## 存储与验收

Local 使用既有 schema-1 的附加初始化，保留同数据根的 AES seal；不能删除 seal
后重新建立既有 pin。Hosted 使用相同部署的 `GATEWAY_API_KEY_SECRET`，须先按
`deploy/README.md` 应用附加迁移 `008`–`011`。不自动改线上 PostgreSQL。
旧无 pin 回执可内部读取，但不能据此新授权外发或扣款。
旧 `submitting` 记录不回填未发送证明；缺少 `011` 时明确记录恢复不可用。
SAP 内部模型探测另依赖 `012-gateway-sap-probe-evidence.sql`；可空的
`sap_evidence` 只表示探测事实，不是持久化 Responses 回执或退款依据。
旧行、running 和终态未落库均保持未知，不能读成零发送。

包级隔离入口为 `tools/packaged-provider-fixture/run_sap_stored_lifecycle.py`。
只接受新的 `linshi` 证据目录、已校验 devtest 和专用容器；`status=passed` 才表示
该次夹具完成。不能用旧无状态矩阵替代本生命周期验收。

2026-11-23 原公共生命周期已通过源码 **154/154** 和 API devtest 双 EXE／重启实测：
四个回执、十次发送、十三条终态公共审计、原来源只扣四条消息。
详见[统一验收回执](../progress/2026-11-23-sap-stored-responses.md)；
该旧包不含后来增加的本地恢复 worker。后续恢复包已通过真实公共 create 结算故障、
全部旧实例 drain 后双 EXE 自动恢复、第二周期稳定及零推理／零 OAuth；
全流程 11 次上游操作、14 条公共审计。有证明／未知提交的两条直接造行仅是消费者
对照，不能代替 live proof 生产者测试。来源与边界仍以统一回执为准，
不替代未知提交人工处置、原生窗口或真实租户验收。
后续错误／健康及发送证据批次已通过 **207 项源码回归、11 项隔离 Hosted**
（SAP 5、Command Code 3、SenseNova 3）；`012` 的合成包载荷合同通过。
这些补丁尚未出新包，不能由上面的历史包结果抵扣；精确覆盖和剩余缺证见统一回执。
其后的零发送 fallback／SSE 拒绝记录与消息无用量恢复批次已通过 48 项 Nextest、
12 项同一 EXE 的直接关联复验及 1 项隔离 PostgreSQL 双连接测试；其中新增消息恢复
覆盖 SQLite 落库后重启、双连接竞争及公共 create 故障后的后台恢复，仍未出新包。

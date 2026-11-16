# 首批三个服务商逐项收口

用户于 2026-11-13 明确选择：**Command Code → SenseNova Token Plan → SAP AI Core**。
本批终点是逐项达到“只差真实账号验收”，不是同时铺开 67 项，也不是继续累计局部测试后宣布完成。

## 状态与工作纪律

| 顺序 | 产品 | 当前状态 | 当前责任 |
| --- | --- | --- | --- |
| 1 | Command Code | 开发中，未达到仅待账号验收 | 主线程优先收口；先补目录／协议与剩余非账号门槛 |
| 2 | SenseNova Token Plan | 排队，未达到仅待账号验收 | 保留现有代码与回执，不并行扩写 |
| 3 | SAP AI Core | 排队，未达到仅待账号验收 | 保留现有代码与回执，不并行扩写 |

不撤销其他工作线。用户随后明确授权本线程仅修复已复现的 S06/Gemini feature 关闭路径编译门禁，不接管其余 Gemini 功能；该窄范围修复可以推进。不将尚缺实现改写成“不支持”以满足完成数。
已有明确独立产品、未获准第三方身份或尚无公开合同的功能，需单独记录范围／资格决定，不能假定
真实账号会解决开发或许可问题。若仍存在这种未解决项，该产品不能标成“只差真实账号”。

## 逐项完成门槛

- [ ] 批准范围中的配置、凭据、目录、推理、协议、错误／取消、用量和管理入口已实现。
- [ ] 必要的功能拒绝具有明确范围依据；没有以静默丢字段或转换降级冒充原生能力。
- [ ] 改动相关合成上游、管理 API 与 UI 流程通过，持久化／重启及资源生命周期按风险验证。
- [ ] 独立功能构建、相关格式／有效行／静态检查通过，跨模块阻碍有真实解除回执。
- [ ] 当前源码的可运行手测产物、包完整性、隔离启动及相关产品流程验收完成。
- [ ] 适用的 Server／多实例边界已验证或经明确范围决策处理，不留未标注的开发缺口。
- [ ] 剩余清单仅含必须使用真实账号才能确认的权限、上游行为、额度／消费等事项。

## 1. Command Code

### 2026-11-14 用户确认的范围决定

用户明确同意本批按**公开 Provider API** 收口。身份接入使用 Studio 手工创建的 API Key；
OAuth 自动签发，以及尚未核实的 Responses 查询/删除/取消等远端状态生命周期，移至后续合同确认项。
这些项不标记为已实现，也不判定为上游不支持；不再以它们阻挡本批公开 API 范围的收口。
System One/Jev、公开推理协议、目录、ZDR、用量、管理流程及非账号交付门槛仍在本批范围内。
下文较早进展中“状态生命周期/资格仍开放”的记录为历史状态，以本范围决定为准。

用户进一步批准 System One 的现金边界：普通配额/消息预付及实际用量保留；现金预付明确
拒绝且不发送，现金接入留待上游费用上限合同确认，不影响 Command Code 其他接口。
当前 devtest 包已实测 HTTP 400 / `cash_usage_bound_unavailable`、现金余额不变且无预留。
用户已批准启动 Docker 并创建隔离 PostgreSQL/Redis；服务就绪，数据保留，尚不代表
Server/双实例业务验收通过。具体失败夹具纠正及运行回执见下方手测包文档。

System One 最新检查点：公开决策入口、目录专用 probe、持久化重启、脱敏、鉴权/过滤/体积限制与取消释放的
默认栈 Command Code 回归 **104 passed / 0 failed**，共享路由 **37 passed / 0 failed**，
独立 feature 的 lib/bins 检查通过。尚未完成整卡交付与全部边界验收，不能改为仅待真实账号；
详见 [System One 接入](../progress/2026-11-14-commandcode-systemone.md)。

当前源码 desktop devtest 包已生成并通过包检查、隔离 Local 存储/退款/轮换/重启及包内
System One 401/429/400 入站校验，见 [手测包与隔离运行](../progress/2026-11-14-commandcode-devtest.md)。
不将包内拒绝路径验收扩大解释为上游生成或 Server 多实例验收。

后续 Server 双 EXE 已完成隔离 PostgreSQL/Redis 就绪、管理账号跨实例可见、完整 access
bundle 绑定、同一 Key 在两个进程的 System One 现金拒绝与余额保持、正常 drain 验收。
真实 Hosted 原生 Messages 成功发送/审计与 Redis 槽的新增源码用例已通过（两个独立 AppState，
非两个生成 EXE），测试二进制定向复核 exit 0；不据此关闭整卡或宣称完整调度 worker 周期已验；
见 [Server 验收记录](../progress/2026-11-14-commandcode-server-runtime.md)。

2026-11-13 进展：已保存 Chat-capable 目录模型的协议声明，前后端 schema／旧数据兼容／
正常表单序列化通过；Rust 定向 35 项、UI 15 项、typecheck 通过，见
[协议元数据](../progress/2026-11-13-commandcode-endpoint-metadata.md)。后续已将声明应用于
原生 Responses 文本／函数工具执行，产品回归 39 项、共享 Responses 回归 47 项通过；
Messages、完整输出保真和其他门槛仍未完成，见
[原生 Responses 切片](../progress/2026-11-13-commandcode-native-responses.md)。

2026-11-14 后续进展：原生 Messages 文本／函数工具也已接入，Messages-only 保存配置经
公共 JSON／SSE 两回合、计费及失败退款通过；产品回归 44 项、共享路由 35 项、Anthropic
60 项及独立 feature 编译通过。自动目录导入／专用 probe 与完整保真仍开放，不能勾选整项，见
[原生 Messages 切片](../progress/2026-11-14-commandcode-native-messages.md)。

已有 Chat 文本／函数工具、公共三入口桥接、管理保存、目录、专属测试和自动调度证据。
当前不能据此关闭原生 Responses／Messages、System One/Jev、reasoning／媒体、ZDR 等缺口。

2026-11-14 目录后续：已导入 Messages-only／Responses-only 模型，专用 probe 按声明发送原生请求，
管理 CAS 保留 Key 保存并重建 Local runtime 后的 probe／公共推理通过；
含手工模型兼容修复的最终产品回归 49 passed，目录 35 passed、Responses 60 passed、前端 5 passed。
Server 目录投影、完整保真与交付仍开放，见
[目录与原生探测](../progress/2026-11-14-commandcode-catalog-native-probe.md)。

同日 Hosted 接线后续：DB row／凭证 merge、兼容 reader 和实际缓存转换入口已恢复原生目录与
身份；目录能力与原有 allowed/blocked policy 组合，凭证目录按完整快照覆盖。产品 57 passed，
共享回归 36／3／20／17／29 passed，独立 feature 编译通过。此为生产 reader/builder 的测试，
不等同连接真实 PostgreSQL／Redis 的部署验收，见 [Hosted 目录接线](../progress/2026-11-14-commandcode-hosted-catalog.md)。

原生 JSON 回复保真后续：已接入产品自有 sender 的 bounded JSON passthrough，保留
reasoning／refusal／thinking／签名及工具扩展，维持公开模型别名、用量和错误退款边界。
交叉核验后的用量观察补强已通过最终产品 60 项回归，共享 Responses／Anthropic／native dispatch
分别 60／60／22 项通过，见
[原生 JSON 回复保真](../progress/2026-11-14-commandcode-native-json.md)。请求侧、SSE 扩展和整卡门槛仍开放。

Messages SSE 后续：已保留 thinking／签名／citations／server-tool 与未知事件，支持多个累计
usage 更新并有界延迟发布终态；专用观察模式防止扩展字段污染计费／完成语义。
产品 66 项、共享 stream 53 项、Anthropic 60 项通过，见
[原生 Messages SSE 扩展](../progress/2026-11-14-commandcode-native-messages-sse.md)。
该切片不关闭请求侧、Responses SSE 扩展、工具注入、Server／交付与其他整卡门槛。

Responses SSE 后续：公开入口已保留原生扩展，补齐模型别名、非法终态拒绝和只读权威终态的
专用观察器，避免扩展／别名计数字段干扰结算；incomplete 保留具体完成原因。
最终产品 **71 passed**，共享 stream／Responses／Anthropic **53／60／60 passed**，
SenseNova／SAP 回归 **24／26 passed**，见
[原生 Responses SSE 收口](../progress/2026-11-14-commandcode-native-responses-sse.md)。
下一块为同协议原生请求保真与 admission；现有 Command Code 本就不走工具注入，需补的是
原生 reasoning／媒体／custom 等输入及策略同步，不能靠原样 raw 转发绕过 canonical 修改。

原生请求侧后续：同协议 rich 输入已与跨协议 text/function admission 分离，保留原始历史、
签名、媒体、custom／上游工具和扩展，同时同步 canonical 字段修改；只改顶层策略不重建 rich history。
最终产品 **77 passed**，共享 stream／Responses／Anthropic **53／60／60 passed**，checker 31、
ratchet 与定向格式通过。该切片源码独立 feature 检查已通过（exit 0）；远端状态／background 生命周期、
ZDR、System One/Jev、资格与 Server／交付门槛仍开放，见
[原生请求保真与策略同步](../progress/2026-11-14-commandcode-native-request.md)。

ZDR 后续：已接入账号设置、严格公共 header、跨候选／refresh 的隐私要求保持、原生工具
限制和 422 不重放；公共三协议 JSON/SSE、CAS keep-key 重启及真实 send-loop 的提前跳过
边界已通过。当前产品回归串行 **87 passed**、UI **11 passed**、typecheck 与独立 feature
编译通过。并发回归发现的既有调度测试时间边界已改为比较持久化槽，修正后定向回归通过，详见
[ZDR 与验证记录](../progress/2026-11-14-commandcode-zdr.md)。这不关闭整卡的状态生命周期、
资格、Server／多实例和交付门槛。

- [x] 保存并应用目录的 `supported_endpoints`，避免只保留模型 ID 后猜测协议；合成目录、持久化重启及 Hosted reader/builder 已验，真实账号目录留待账号验收。
- [x] 接通原生 Messages／Responses 的请求、回复、流终态、用量及函数工具流程；原生 JSON/SSE 与 rich 请求回归已通过。
- [ ] 核实并处理 System One/Jev、reasoning／媒体、ZDR 与其他原记录功能缺口。
- [x] 已补 Responses 的 custom／上游内置工具保真及 ZDR 工具约束，并通过相应回归；不以仅函数工具通过代替。
- [x] 明确 Studio 人工创建 Key 与浏览器自动签发的产品边界：按上述用户确认范围执行，不伪造 OAuth 合同。
- [ ] 完成上面的非账号交付门槛，再移交真实账号验收。

2026-11-13 已重新联网读取官方 Provider 文档（https://commandcode.ai/docs/provider），确认
Chat、Responses、Messages、Models、System One 仍为不同路由，模型以 supported_endpoints
声明能力。不得把三个公共入口都转 Chat 解释为已完成三种原生协议。

2026-11-14 再次联网读取同一官方 Provider 文档：System One/Jev 是非聊天、非流式决策接口；
Responses 另有 custom／内置工具与 `x-cmd-zdr` 限制。这些仍列为开发缺口，不改写为“不支持”以关闭服务商。

同日实际复跑：

```powershell
cargo check --locked --offline --lib --bins --no-default-features --features line-commandcode-provider-api -j 2
```

修复前 **failed，exit 101**：S06/Gemini disabled surface 的 15 个缺失符号错误，包括
surface::speech、WebUpload、prepare_media、has_media 等。没有增加 Gemini feature 隐藏错误，
当时没有越界修改保留 lane。后经用户明确授权，仅修复该 feature 关闭路径；同一
Command Code、SenseNova、SAP 独立构建及无 feature 基线均已通过，四个关闭入口的
无网络执行测试也已通过，见 [构建门禁回执](../progress/2026-11-14-gemini-disabled-build-gate.md)。
日志：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-first-three-20261113/commandcode-feature.log`。

System One 后续：公开 `/v1/systemone` 已接线，真实 404 红测后 **9 项定向测试通过**，覆盖结构化
answers、目录能力选择、固定身份、别名、用量/预付扣费、失败退款、不重放和 ZDR。
后续目录 probe 与默认栈完整产品回归已通过（100 项），独立 feature 的 lib/bins 检查也已通过。
旧目录预期和默认栈重启组合测试问题已修复。更多公共入口边界正在补验，整卡交付仍待完成。
独立 feature 的 lib 测试另遇到 FreeBuff 测试钩子的条件编译错误，保留原始失败，不以默认
feature 测试代替独立构建验收，见
[System One 进行中记录](../progress/2026-11-14-commandcode-systemone.md)。

## 2. SenseNova Token Plan

- [ ] 核对原生协议、reasoning／媒体的既有缺口，补齐批准范围内实现与本地回归。
- [ ] 单独处理控制台 OAuth／积分池的公开合同与客户端资格，不以普通 Key 接入抵扣。
- [ ] 补当前缺失的自动调度／原生启动与持久化使用流程证据。
- [ ] 独立构建、UI／原生包、适用 Server 边界完成后，才列为仅待真实 Key／订阅／消费验收。

## 3. SAP AI Core

- [ ] 补自动部署发现，明确资源组、部署选择、模型及协议声明的配置流程。
- [ ] 核实并补齐批准范围内其他云 endpoint、x.509、Orchestration／Bedrock／Responses、reasoning／媒体。
- [ ] 保留 client_credentials 的隔离、有界缓存、超时、不重放及 secret keep 绑定保证。
- [ ] 独立构建、UI／原生包、适用 Server 边界完成后，才列为仅待真实 Service Key／部署／费用验收。

## 参考

- [剩余范围总览](magpie-remaining-2026-11-13.md)
- [Command Code 合同](commandcode-chat-contract-2026-11-10.md)
- [SenseNova 合同](sensenova-token-plan-contract-2026-11-10.md)
- [SAP 合同](sap-aicore-chat-contract-2026-11-10.md)

本文件是批准名单、顺序与未完门槛的记录，不是三项完成回执。

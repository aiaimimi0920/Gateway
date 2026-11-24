# 首批三个服务商逐项收口

用户于 2026-11-13 明确选择：**Command Code → SenseNova Token Plan → SAP AI Core**。
本批终点是逐项达到“只差真实账号验收”，不是同时铺开 67 项，也不是继续累计局部测试后宣布完成。

## 状态与工作纪律

| 顺序 | 产品 | 当前状态 | 当前责任 |
| --- | --- | --- | --- |
| 1 | Command Code | 已批准的公开 Provider API 范围：非账号验收通过 | 待真实 Studio Key 验证目录、权限、上游行为及套餐用量／费用 |
| 2 | SenseNova Token Plan | 原生三协议、双 EXE 图片／推理历史及发送前拒绝、双后端自动调度和原生账号目录拉取／保存／重启读回已验 | 核实剩余媒体、控制台 OAuth／积分池范围及真实账号验收 |
| 3 | SAP AI Core | Hosted 合成矩阵 48 条；持久化 Responses 公共生命周期、无用量后台恢复、Hosted 管理探测失败与零发送 JSON／SSE 回退均有双 EXE 回执 | 未知提交及不确定 mutation 人工处置、剩余协议及真实部署／费用仍未收口 |

不撤销其他工作线。用户随后明确授权本线程仅修复已复现的 S06/Gemini feature 关闭路径编译门禁，不接管其余 Gemini 功能；该窄范围修复可以推进。不将尚缺实现改写成“不支持”以满足完成数。
已有明确独立产品、未获准第三方身份或尚无公开合同的功能，需单独记录范围／资格决定，不能假定
真实账号会解决开发或许可问题。若仍存在这种未解决项，该产品不能标成“只差真实账号”。

2026-11-23 合并验收：[媒体、推理历史与发送前拒绝回执](../progress/2026-11-23-provider-media-admission.md)。
新 `gateway-devtest-provider-media-admission-20261123-194746` API 包通过五组串行双 EXE
验收，共 49 条成功公共审计，另有 SenseNova 四条非法输入零发送／零实际归属且不罚健康凭据。
同包已包含 SAP 32 项矩阵；Command Code 三协议和 System One、SenseNova 三协议均回归通过。
这不关闭下列合同、人工处置、原生窗口及真实账号缺口，也未更新线上实例。

## 逐项完成门槛

> **批处理提速执行纪律（强制）**：以下门槛是**整张服务商卡片**的最终收口门槛，严禁套用到单个字段或微切片上。推进任一服务商时，必须先一次性盘清该产品获准范围内的全部剩余代码缺口，成批完成源码与单测修改（编码期使用增量 `cargo check`、`cargo nextest run` 与 `check:effective-lines:fast`）；**仅在该服务商全部剩余代码开发完毕后，集中执行 1 次手测出包（纯后端用 `-Edition api`，含前端改动才用 `-Edition desktop`）、1 次包级/多实例联合验收与 1 次回执归档**，禁止每改一个小功能点就重复打桌面包、跑双进程调度或新建零散 progress 文档。后续批量服务商接入须并发派发子代理预研官方合同与拆分模块，按 3–4 个服务商成批合入共享单次构建验收。

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

2026-11-21 后续：测试夹具栈溢出已消除。两个 Server AppState 的真实定时周期、Redis 单槽、
结果跨实例读取及重建保留均执行到通过；联合验收仍因两次自动探测在 PostgreSQL 中没有审计而失败。
普通 Hosted Messages 两次发送、审计与 Redis 槽对照仍通过。正式探测审计主体是非账号缺口，
不得记为整项通过；见 [Server 自动探测与审计复核](../progress/2026-11-21-commandcode-server-scheduler.md)。

2026-11-22 已实现独立的内部探测审计与 `007` 迁移。隔离 PostgreSQL／Redis 联合回归
**4 passed**：两个真实周期各一次发送、各一条内部完成审计及准确用量、零公共审计，
重建后结果和 Redis 槽保留；普通 Hosted Messages 对照、失败／取消、审计不可用零发送均通过。
这是双 AppState 源码回执，不是双 EXE 上游生成验收，也未更新实际部署。
新桌面 devtest 已核验包内 `007`、EXE 哈希和隔离 SQLite／管理页，未验证原生业务点击或包内双 EXE 探测成功流程。
当时的交付状态见 [Server 探测审计回执](../progress/2026-11-22-commandcode-server-probe-audit.md)。

- [x] 独立 devtest 的双后端真实定时周期、两条内部审计、零公共审计、Redis 槽与重启读回已验。
- [x] 原生桌面实际提交合成账号，保留 Key 编辑模型，关闭后重启确认掩码和模型；没有尝试真实目录或调用。
  两份独立证据见 [包内定时探测与原生账号补验](../progress/2026-11-22-commandcode-packaged-scheduler.md)。

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
- [x] System One/Jev 与 ZDR 已有独立源码回归；System One 现金预付按上述用户确认范围在发送前拒绝。
- [x] 2026-11-23：原生 Chat 图片的当前源码 JSON／SSE 回归通过。首轮 Nextest 82 通过／1 失败，复现公开 SSE 泄出上游模型名；修复复用现有有界流守卫，在原始模型观测后统一投影公开别名。复跑 Command Code、共享 Chat SSE 守卫与 SenseNova 合计 **135 通过／0 失败**，包括逐帧别名、原始图片参数、准确用量及无效输入零发送／零扣费。日志：`linshi/gateway-packaged-provider-fixture/commandcode-nextest-{resume,alias-fix}-20261123.log`。
- [x] 2026-11-23：`gateway-devtest-commandcode-hosted-20261123-080148`（`api`、`devtest`）通过包完整性和 Hosted 双后端正向验收。第一进程写入合成账号与授权，第二进程从三个公共入口各跑一次 JSON 和 SSE；六次原生发送、逐帧 Chat 公开别名、Chat 图片参数原样到达、隔离 PostgreSQL 中六条精确 2/3/5-token 审计、缺 CA／错误 CA 和正常 drain 均通过。证据根 `linshi/gateway-packaged-provider-fixture`，回执 `commandcode-hosted-gateway-devtest-commandcode-hosted-20261123-080148/result.json`；EXE SHA-256 `ff983886c93d1dcb83321971476afbf40995c2b878692e116191c14a04be36a8`。源码双 AppState 的 `commandcode-source-server-70ec0322eee94e96aec3b5128ceb86d2/result.json` 另证精确 opt-in 用例 2/2。本 API 包不含桌面；此回执不覆盖 System One 正向、实际图片抓取或真实上游。
- [x] 同一 `api` devtest 不重编，另以 `run_commandcode_scheduler.py` 验证双后端两个真实定时周期：恰好两次固定 Chat 发送、两条内部完成审计、零公共审计，Redis 槽、重启读回和缺 CA 门禁通过。回执同证据根下 `commandcode-scheduler-api-20261123-94ededd115ad45eea7a9a3d269282db9/result.json`，manifest 与 EXE SHA-256 均与 Hosted 回执相同。
- 2026-11-23 复核公开 Provider API：未给出逐模型 Chat reasoning 参数或历史扩展。原生 Messages／Responses 的 reasoning 与富历史证据不替代 Chat；不猜测字段，不扩大跨协议 fallback，也不判为上游不支持。
- [x] 已补 Responses 的 custom／上游内置工具保真及 ZDR 工具约束，并通过相应回归；不以仅函数工具通过代替。
- [x] 明确 Studio 人工创建 Key 与浏览器自动签发的产品边界：按上述用户确认范围执行，不伪造 OAuth 合同。
- [x] 2026-11-23：`gateway-devtest-commandcode-systemone-20261123-084916`（`api`、`devtest`）的 System One/Jev 双后端 Hosted 正向通过。第一进程保存合成 Studio Key、目录、别名及消息预付授权，第二进程产生一次非流式 `/provider/v1/systemone` 发送。验证原生结构化请求和答案、公开别名、2/3/5-token 精确 PostgreSQL 审计、消息额度 2→1、缺 CA／错误 CA 与正常 drain。旧包同一用例在授权通过后以 `commandcode_fixture_request_invalid` 拒绝，零上游发送；新接线未放宽正式运行、现金预付或外网边界。
  证据根同上：`commandcode-systemone-gateway-devtest-commandcode-systemone-20261123-084916-2a2f0769498a43609ba05bbb6bc30d4e/result.json`；同包三协议 JSON／SSE 回归 `commandcode-hosted-systemone-package-92247c3340b94d6d8e78edd6dc08229b/result.json`。EXE SHA-256：`191b3f70d27983250dc15454d629a50dfbdbfbdac2e053c3ed55d948487983ec`。Rust 定向 18/18、Python 夹具 9/9、56 个 manifests、31 项 checker 与 ratchet 通过。未调用真实上游；不替代原生桌面或真实 Studio Key 验收。
- [x] 同一 EXE 定时探测也已复跑：两个后端、两个真实周期、两条内部审计、零公共审计、Redis 槽与重启读回。回执 `commandcode-scheduler-systemone-package-953a70bd1db04dc59aa0a81ed57dcb79/result.json`，EXE 和 manifest 哈希与上述两份正向回执一致。`cargo check --locked --offline --lib --bins --no-default-features --features line-commandcode-provider-api`（当前源码）退出 0。
- [ ] 真实 Studio Key 验收：实际目录与方案权限、三原生端点及 System One、ZDR 与上游用量／费用。已有原生桌面合成账号提交／编辑／重启回执来自独立桌面包，本次 API 包未重跑桌面。11 月 14 日已确认后置的 OAuth、远端 Responses 生命周期和现金接入，不计作已实现。

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

2026-11-22 已复核官方 Messages／Responses 文档；[原生 Messages](../progress/2026-11-22-sensenova-native-messages.md)
合成产品回归 28/28 通过。[原生 Responses](../progress/2026-11-22-sensenova-native-responses.md)
已接入固定路由、无状态准入与原生 JSON／SSE；最终 SenseNova 合成回归 31/31、共享回归及
独立 feature 检查通过。

[Server 自动探测与内部审计](../progress/2026-11-22-sensenova-server-probe-audit.md) 已完成：隔离库回归 **4/4**，真实调度周期合计两次发送、两条内部完成审计、零公共审计；关闭池、终态写入失败及取消路径有独立验证。[Local 管理保存后重建和 Server Hosted 公共推理](../progress/2026-11-22-sensenova-persistence-and-hosted.md) 已通过合成产品测试；完整隔离 Server 回归 **5/5**。桌面 devtest 已完成包完整性、Local 重启及隔离原生凭据池验收。

- [x] 验证 Chat 图片按真实上游模型准入：三个模型的公共 JSON／SSE 共六次请求验证图片透传、正确端点和用量；五个无效输入零上游发送、预付余额不变。SenseNova 默认功能回归 **34 通过／5 项隔离测试未跑**。见 [Token Plan 合同](sensenova-token-plan-contract-2026-11-10.md#gateway-行为)。
- [x] 补齐原生 Chat 工具多轮 `reasoning_content`：`deepseek-flash` 合成上游的公共 JSON／SSE 各两轮，验证回复与请求原样保留、工具 ID、模型路由、用量只结算一次；畸形历史零发送且余额不变，策略改写不会被原始消息覆盖。SenseNova 默认功能回归 **37 通过／0 失败／5 项隔离用例另跑**；同一测试 EXE 又在隔离 PostgreSQL／Redis 中执行这 **5 项，全部通过**，包括 Hosted 三端点 JSON／SSE 和真实调度周期。独立 `line-sensenova-token-plan` 的 lib／bins 检查通过，共享 DIM 公共链路、OpenAI 打包、Chat SSE 守卫分别 **8／4／17** 项通过。未为此改动重新出包或调用真实上游。
- [x] 新 API 包补验图片 JSON／SSE、工具推理首轮和实际 assistant 续轮；修复旧包中连续非法输入误计 attempt 并冷却健康凭据的问题。连续四条非法输入均精确 400、peer 零新增 POST、空实际归属、拒绝账号／凭据／模型／原因准确、余额及凭据健康保持；另有正常三协议 6 条回归。详见上述合并回执；保证范围是本次 `validate_plan` 拒绝，不扩为所有零发送路径。
- [x] 后续源码补齐 content 边界：原始非法 text 不再被归一化为合法空文本，未知媒体／块及最终计划中的非法 content 形状在发送前拒绝。公共 JSON／SSE 四种 role 的 40 次拒绝均零发送、余额／健康不变，同账号随后正常成功；文本扩展、空 assistant 工具内容与 reasoning 保持。该补丁未出新包，不抵扣视频合同，也不声称封闭所有顶层媒体参数；见[源码合批回执](../progress/2026-11-23-sap-stored-responses.md#拒绝记录与消息预付恢复)。
- [ ] 核实剩余媒体能力。`token.sensenova.cn` 的公开合同提到 DeepSeek Flash 支持 Chat 视频，但未给出足以核实对象类型、字段、编码与限制的请求示例。先取得该端点的正式合同或供应商确认；不借用 `token.sensetime.com` 的模型或请求格式，不以图片或推理历史回归抵扣。
- [ ] 单独处理控制台 OAuth／积分池的公开合同与客户端资格，不以普通 Key 接入抵扣。2026-11-22 核对了另一套官方 `token.sensetime.com/api/v1` 与 `stk-` Key 合同；两域名目录的匿名请求均未重定向，未找到 Key 互通、迁移或新增视频／OAuth 合同，不混用；见 [合同复核](sensenova-token-plan-contract-2026-11-10.md#2026-11-22区分两套公开-api-合同)。
  2026-11-23 再次匿名读取 `platform.sensenova.cn/docs` 当前引用的官方文档资源 `210-35a7025cd2eaebe6.js`（SHA-256 `b4cec554990cb8deef4eeb05949bbdec4b1a85bdf45e7e09e1a982e442eae813`），未在该资源中找到 `video_url`、`pool-usage` 或第三方 client 注册说明。这不是上游不支持的证据，两个合同缺口继续保留。
- [x] Server 自动调度：包内两个后端 EXE 共同运行，两个真实周期各发送一次；独立 namespace 恰有两条正确归属的内部完成审计、零公共审计，并通过重启读回及 Redis 活跃槽验证。见 [包级调度回执](../progress/2026-11-23-sensenova-packaged-scheduler.md)。
- [x] 验证 Local 管理保存后重建使用、Server Hosted 公共推理（不同 AppState，非双 EXE）。
- [x] 验证独立桌面手测包双 EXE、隔离原生创建空池与账号重启读回。
- [x] Server Hosted 双后端 EXE 合成公共推理：[包级回执](../progress/2026-11-22-sensenova-packaged-hosted.md)原样 runner 通过，覆盖三入口 JSON／SSE、逐帧 Chat 公共别名、精确审计、TLS 负对照和 drain。
- [x] 包内真实管理 API 的合成目录：草稿 Key／已保存账号各读一次，校验授权、文本过滤、revision、不写配置及零推理；见[目录回执](../progress/2026-11-22-sensenova-packaged-catalog.md)。
- [x] 原生 WebView2 已实际拉取已保存的合成账号目录并保存；重建独立后端与窗口后读回，未回填 Key、零推理。见[原生回执](../progress/2026-11-22-sensenova-packaged-catalog.md#2026-11-23原生账号目录保存及重启)。
- [ ] 真实 Key／订阅／费用单列验收。
- [ ] 独立构建、UI／原生包、适用 Server 边界完成后，才列为仅待真实 Key／订阅／消费验收。

## 3. SAP AI Core

- [x] 接入只读部署发现与显式部署选择：已见 [SAP 合同](sap-aicore-chat-contract-2026-11-10.md#只读部署发现与手动绑定)。[源码验收回执](../progress/2026-11-22-sap-deployment-discovery.md)。真实租户目录与原生窗口测试仍单列。
- [x] 在显式启用的 GPT-4o Chat 部署接通内嵌图片：公共 Chat／Messages／Responses 的 JSON 和 SSE 经实际 Gateway 路由与本地模拟 SAP 验证；[图片输入回执](../progress/2026-11-22-sap-chat-image-input.md)。不是原生 Responses，也未使用真实租户。
- [x] 接入 X.509 Service Key：客户端证书只用于 OAuth mTLS，复用现有短期 Token 缓存、部署查询和 Chat；合成 TLS 握手、轮换隔离、错误域名／重定向零推理与失败退款已验。见 [X.509 回执](../progress/2026-11-22-sap-x509-service-key.md)。真实证书和租户仍未验。
- [x] 验证 Server Hosted 的 X.509 公共推理：两个独立 AppState 经 PostgreSQL 共享账号、凭据与别名；三公共入口 JSON／SSE 各一次，另有认证负对照及精确用量审计。见 [Server Hosted 回执](../progress/2026-11-22-sap-hosted-x509.md)。双后端 EXE 与真实租户仍未验。
- [x] 接入 SAP 官方列出的 Azure EU20 API origin：控制台空池与 Service Key 绑定、只读部署发现和本地模拟 Chat JSON／SSE 已验；旧 AWS 保持兼容。见 [Azure EU20 回执](../progress/2026-11-22-sap-azure-eu20.md)。真实 Azure EU20 租户未验。
- [x] 当前 Azure EU20 桌面 devtest 的包校验、隔离 SQLite、原生空池创建和包内双后端 Hosted 负向链路已验。账号保存通过管理 API，尚未验原生账号提交／重启读回或双 EXE 成功推理，见[手测包回执](../progress/2026-11-22-sap-eu20-devtest.md)。
- [x] 显式启用 o3-mini Chat 的 `reasoning_effort` 和 `max_completion_tokens`：Local 合成 JSON／SSE、前端配置与独立编译已验，见 [o3-mini 回执](../progress/2026-11-22-sap-o3-mini-chat.md)。[Hosted 部署能力归属与合成推理](../progress/2026-11-22-sap-hosted-reasoning.md)也已通过：两个独立 AppState 的 JSON／SSE、凭据不能覆盖部署能力、精确用量审计。包内 Hosted 成功生成及真实部署未验。
- [x] 显式启用原生无状态 Responses：Local 公共 JSON／SSE 文本和函数工具、无状态推理历史准入、取消退款、不重放与 Hosted 双 AppState 用量审计均通过；控制台、独立构建、新 devtest 完整性与通用隔离 SQLite 已验证，见 [原生无状态 Responses 回执](../progress/2026-11-22-sap-native-responses.md)。该文本切片不包括远端状态、后台任务、内置工具和媒体。
- [x] 原生无状态 Responses 已接入显式开启的内嵌 PNG／JPEG；本地 JSON／SSE、OAuth 前拒绝、用量、设置归属及控制台回归通过。新桌面 devtest、隔离 SQLite 与包内管理 API 保存／重启脱敏读回已验；原生账号提交和打包双后端成功推理仍未验。见[内嵌图片回执](../progress/2026-11-22-sap-native-responses-images.md)。
- [x] Orchestration v2 非流式文本 Chat 的源码、控制台与合成公共链路已验：固定模型、真实请求路径、最终结果及用量、拒绝和不重放。此处记录初始 Local 验收，Hosted 后续见下。见[回执](../progress/2026-11-22-sap-orchestration-v2.md)。
- [x] Orchestration v2 文本 SSE 与终态用量已接线，官方夹具独立回放和 Gateway 公开文本用例（正常终态、错误尾部、取消退款）均通过，见[流式回执](../progress/2026-11-22-sap-orchestration-v2-sse.md)。
- [x] Orchestration v2 原生函数工具与两轮结果回传已接线；公共 Gateway JSON／SSE 两轮、用量、退款及不重放均通过，见[函数工具回执](../progress/2026-11-22-sap-orchestration-v2-tools.md)。隔离 PostgreSQL／Redis 双 AppState 的 JSON／SSE 两轮、账号固定模型和 4 条精确审计通过；Hosted SAP **5/5**。包内成功推理与真实租户仍未验。
- [x] Orchestration v2 增加只读部署发现：固定场景与运行状态，只返回匹配 Key origin 和部署 ID 的候选；显式选择 URL 后仍需填写固定模型。授权、配置不变、前端提交及 Azure／Bedrock 回归通过；新桌面 devtest 的包完整性、隔离 SQLite 和包内查询授权／拒绝转发边界通过；同包已通过原生窗口创建空池及合成账号实际保存，已通过同包新后端进程读回和原生窗口重启后的列表显示，见 [部署发现回执](../progress/2026-11-22-sap-orchestration-discovery.md)。
- [x] SAP 原生 Bedrock Converse 文本模式已接线；公共 JSON／EventStream、退款与独立构建通过，见[文本切片回执](../progress/2026-11-22-sap-bedrock-converse.md)。
- [x] SAP 原生 Bedrock Converse 客户端函数工具已通过 JSON／SSE 两轮公共回归，含异常退款和取消不重放；SAP **75 passed / 0 failed / 3 ignored**，见[函数工具回执](../progress/2026-11-22-sap-bedrock-converse-tools.md)。媒体及真实租户仍单列。
- [x] 接入 SAP Bedrock 只读部署发现与显式选择：默认兼容 Azure，按资源组查询运行中 Bedrock 候选，拒绝交叉协议及异源部署，切换协议或资源组不复用旧结果。SAP **76 通过／3 跳过**、前端 **44/44**；[验证回执](../progress/2026-11-22-sap-bedrock-deployment-discovery.md)。不证明候选支持 Converse；真实租户、Hosted 和原生打包验收仍单列。
- [x] SAP Bedrock Converse 内嵌 PNG／JPEG 已通过本地公共 Chat JSON／流式回归、用量结算、OAuth 前拒绝与 Hosted 部署开关归属测试；SAP **79 通过／3 跳过**。见[图片输入回执](../progress/2026-11-22-sap-bedrock-inline-images.md)。另在隔离 PostgreSQL／Redis 中完成 Hosted 双 AppState 推理、凭据负对照和精确审计，Hosted SAP **4/4**；当前桌面 devtest 已通过包完整性、隔离 SQLite 回归，以及 Bedrock 开关管理 API CAS 保存、重启读回与公共图片准入（仅到拒绝转发的合成 OAuth 边界）。真实租户、原生 UI 业务操作与包内双 EXE 成功推理仍未验。
- [ ] 核实并补齐 Azure EU20 之外批准范围内尚未接入的云 endpoint、Orchestration／Bedrock、其余推理参数与媒体；持久化 Responses 的恢复能力与真实部署资格仍单列。
  - Orchestration v2 的[推理和图片官方 SDK 合同](https://sap.github.io/ai-sdk/docs/python/examples/orchestration-service2)已定位：推理档位放在 `modules.prompt_templating.model.params.reasoning_effort`，JSON `final_result.choices[].message` 和 SSE `final_result.choices[].delta` 上的 `reasoning_content` 都是块数组，续轮需保留完整块（不能只拼接文本或丢掉签名）；图片支持 `user` 消息文本混排和 data URL。Orchestration 图片已接入独立的池级开关，见 [产品合同](sap-aicore-chat-contract-2026-11-10.md#orchestration-v2-chat-模式)；本轮 Nextest **13/13**（`fe26ec9a-0d65-49ba-b0d7-e377d54b04f3`）覆盖公共 Chat JSON／SSE、图文／函数模板、精确用量、非法图片零 OAuth 和 Hosted 账号／凭据能力合并；桌面 typecheck 与 SAP 相关 Vitest **16/16** 通过。未验证打包、原生窗口、真实 SAP 图像模型或费用。
  - 2026-11-23：Orchestration v2 推理已接入独立的池级开关 `sap_orchestration_reasoning`，默认关闭，不能由凭据开启或覆盖，不沿用 o3-mini Chat 的档位限制。SAP 合同允许 `xhigh`，实际档位由部署模型决定。Gateway 合成公共 Chat 验证 JSON 推理块、原样续轮、SSE 分块签名与终态、`high`／`xhigh` wire、未启用或非法档位零 OAuth，以及两次成功调用各 5 tokens、共扣两条消息。最终 Nextest **14/14**（`eabd21a4-a442-4f6c-8435-5b0f9ab462c1`）、桌面 typecheck、串行 Vitest **16/16**、有效行检查器 **31/31** 与 ratchet 通过。此回执是本地受控上游和组件测试；未用新 EXE、Hosted 双进程或真实租户复验推理。
  - 持久化上游合同依据：[SAP Java SDK 官方指南](https://sap.github.io/ai-sdk/docs/java/foundation-models/openai/responses)，以及 `ai-sdk-java` 1.22.0、commit `f3ec77445fa383ec09e3c5684b3cdad8776ba565` 的部署 destination、HTTP client 与映射夹具。合同包含创建、后台轮询、retrieve／cancel／delete 和前序关联；不推定所有 Azure 部署有资格，不自动添加 Chat 的日期 `api-version`。
  - 2026-11-23 已将此前的协议、双库回执、耐久账务、mutation 屏障、迟到用量及原路由授权整合为[受控公共生命周期](sap-stored-responses.md)。`sap_stored_responses` 默认关闭，依赖原生 Responses；显式启用后支持自有 Gateway ID、创建／续接／GET／cancel／DELETE。保留原主体与实时授权、原来源结算、单次发送、断开后写账和 drain，不进入普通重试／回退／第二次预扣。
  - [统一源码与双 EXE 回执](../progress/2026-11-23-sap-stored-responses.md)：原恢复批次 Nextest **177/177**、Python **20/20**、定向格式及权威行数门禁通过；全仓格式检查仍有已列出的无关差异。原 API devtest 的四个回执、十次发送、十三条终态公共审计、重启读回、并发单次取消、墓碑及原成员撤销零发送、按原成员只扣四条消息均通过。新增 `011` 耐久未发送证明和正常 runtime 的本地账务恢复已验双库源码；只退款匹配证明或结算已记录的完整权威用量，不访问上游、不填零、不解除 mutation 屏障。新恢复包 `gateway-devtest-sap-recovery-resume-20261123-185420` 另验真实公共 create 结算故障、全部旧实例 drain 后双 EXE 自动结算，以及 synthetic proof／unknown 消费者对照和第二周期稳定；全流程 11 次上游操作、14 条公共审计，自动恢复零推理／零 OAuth。合成行不证明 live owner 产生证明。未改 `4200` 或真实账号。
  - 后续错误／健康和发送证据批次：Nextest **207/207**，同一新测试 EXE 的隔离 Hosted **11/11**（SAP 5、Command Code 3、SenseNova 3），Python 包载荷 **7/7**、56 条 manifests、定向格式及 6160 文件 ratchet 通过。保留真实推理状态和阶段，OAuth／未知传输中性，未知交付不退款不重放；流式断开不伪造完成；Hosted probe 的 `012`／`sap_evidence` 缺失时零发送，running／旧行／终态写失败均未知。另修复 SAP OAuth 后重复限流计数。该批尚未出包。
  - 精确零发送回退与恢复后续：**48 项 Nextest＋12 项同一 EXE 关联复验＋1 项隔离 PG** 全过。真实 OAuth barrier 后撤销原候选，公共 JSON／SSE 各验证普通 fallback 唯一实际发送／归属和一条消息扣款；补齐 SAP 拒绝原因及 SSE snapshot 保留。后台新增严格的 completed／单条消息预付／无用量结算，不估算 tokens、不退款、不解除 mutation。2026-11-24 双 EXE 已另外验证无用量终态后台结算、OAuth 429／推理 503 探测失败；旧包的普通 Chat JSON 回退通过，但 SSE 暴露上游模型名。详见[统一回执](../progress/2026-11-23-sap-stored-responses.md)。
  - 普通 Chat SSE 模型别名已修复；新 API devtest 双 EXE 的 JSON／SSE 回退均通过，SAP 零推理、普通备用恰好各发一次、公开别名／用量／终态正确、精确拒绝和实际发送分别记账。见[新包回执](../progress/2026-11-23-sap-stored-responses.md#2026-11-24发送前撤销与普通-chat-回退)。
  - 仍未完成：未知提交的权威 ID 绑定／人工结案、无成功落库证明的退款处置、不确定 mutation 的人工处置、未覆盖的上游错误语义及现金 tariff。pending 库存只是发现，有明确依据的自动本地恢复不等于未知提交对账完成；默认关闭不抵扣这些开发缺口。真实部署资格、费用和本功能原生窗口验收仍单列。
- [x] 保留 client_credentials 的隔离、有界缓存、超时、不重放及 secret keep 绑定保证（源码与包级互补）。
  - 定向 Nextest **7/7**（`ccdc351d-eeaf-4bc4-8131-0a756ae46a85`）：新增 256 槽全忙时既有槽可复用、新槽非重试拒绝、释放后只回收空闲槽的回归；同一个合成 Service Key 的不同部署／资源组各独立交换并保留各自 Token。并复验 secret 轮换／异常回包、X.509 轮换、Keep 跨绑定拒绝及原生 Responses 失败不重放。
  - 源码：`TokenCache` 按完整 Service Key、部署 URL 和资源组分槽，最多 256 槽，只驱逐无使用者的槽；共享 15 秒等待／交换预算。既有源码回归覆盖 X.509 证书轮换不借用旧 Token、Keep 不跨资源组／部署／产品绑定、原生 Responses 推理失败不重放。
  - 包级：复用 `gateway-devtest-sap-native-catalog-20261123-102429` 的 API 包，`run_sap_auth.py` 证实合成 client secret 交换在 15.172 秒返回 `sap_aicore_token_timeout`，本次零推理且不重放；后续两路并发共用一次 OAuth，过期后重新交换。3 条完成审计、1 条非完成审计、净扣 3 条消息，且正常 drain。回执 `linshi/gateway-packaged-provider-fixture/sap-auth-budget-0b8fe1a7808c471d93aab3acc90b0a86/result.json`。未在包内压满缓存、重验 Keep 或验证 X.509 超时；未连接真实租户。
- [x] SAP Hosted 双后端 EXE 正向推理（合成矩阵）。`gateway-devtest-sap-hosted-alias-20261123-095637`（`api` 手测包）通过 **18/18**：合成 client secret 与 X.509 各覆盖 6 条公共 Chat／Messages／Responses 转 Chat 的 JSON／SSE；原生无状态 Responses、Orchestration v2 文本、Bedrock Converse 内嵌 PNG 各 2 条。核对 5 次 OAuth、18 次推理、18 条归属精确的 `2/3/5` token 审计、公开模型别名、缺 CA、两类错误 CA 零推理和正常 drain。
  旧包实跑发现公共 Chat 及经 Chat 转换的 Responses SSE 暴露上游模型名；修复复用共享 Chat 守卫，在协议转换前投影到公开别名。定向 Nextest **98/98**、隔离 Hosted 双 AppState **5/5**、Python 夹具 **13/13**。证据根 `linshi/gateway-packaged-provider-fixture`；包级回执 `sap-hosted-gateway-devtest-sap-hosted-alias-20261123-095637-b5780a34c5fe4f3da90a10e7c2333295/result.json`。该矩阵不替代部署发现、原生窗口业务验收、未列出的协议参数／媒体组合及真实租户。
- [x] 同一 API 手测包通过只读部署发现：Azure 默认／Bedrock／Orchestration 三种目录，合成 client secret 与直连 X.509，3 次 OAuth、4 页 GET、零推理；筛掉跨协议、异源与停止项，按原始记录数分页并去重。未登录、缺敏感授权、非夹具身份均零发送；路由 revision 与文件字节未变。回执 `linshi/gateway-packaged-provider-fixture/sap-catalog-azure-default-875810e9be4c41a4b579a239847ad271/result.json`。
- [x] 复用同一 API 包扩展媒体与推理矩阵至 **28/28**：新增 Azure Chat 内嵌 PNG 的三公共协议 JSON／SSE 各两条、原生无状态 Responses 内嵌 PNG 两条、o3-mini Chat reasoning 两条。逐项校验图片字节及 `detail`、原生 Responses `store=false`、o3-mini 版本与 `reasoning_effort`／`max_completion_tokens` 映射；28 条精确审计、8 次 OAuth、负对照及 drain 均通过。回执 `linshi/gateway-packaged-provider-fixture/sap-hosted-strict-media-reasoning-8687b475e6e443f1b4db9c9e605b69db/result.json`。不证明 JPEG、其他模型／版本、原生窗口或真实租户。
- [x] 更新后的仓库夹具针对上述恢复包 EXE 将 Hosted 矩阵扩至 **32/32**：新增 Orchestration 内嵌 PNG JSON／SSE，以及 `high` JSON 首轮实际 assistant 回传、`xhigh` SSE 续轮的完整 reasoning／opaque signature 块；核对 10 次 OAuth、全部 32 次 inference 尝试、32 条精确审计及每次成功扣一条消息。回执 `linshi/gateway-packaged-provider-fixture/sap-hosted-orchestration-20261123-1930/result.json`。包内原脚本仍是 28 条，未篡改原包；不证明密码学签名、图片＋tools 组合、错误 CA 后的退款／失败审计、原生窗口或真实租户。
- [x] 当前 API devtest 的双后端 Hosted 矩阵扩至 **48/48**：Orchestration／Bedrock 的纯文本工具及图文＋工具各走 JSON、SSE 两轮，实际首轮调用原样回传。14 次 OAuth，48 次推理及精确审计。client secret／直连 X.509 的两次错误 CA 均验证零 HTTP、零发送归属、预扣返还、健康不变。见[包与回执](../progress/2026-11-23-sap-stored-responses.md#2026-11-24双-exe-工具续轮与-tls-拒绝)；不代表真实租户、原生窗口或未验证的包级场景完成。
- [x] 当前桌面 devtest 的 Orchestration 串联验收：原生 WebView2 成功查询、显式选择、创建空池并提交账号；从首轮提交的同一 Redis revision `r2-c236f1789420` 启动新后端和新窗口，读回原部署与账号，Key 未回填，状态未改，重启过程零上游发送。见[原生串联回执](../progress/2026-11-22-sap-orchestration-discovery.md#2026-11-23-原生窗口成功查询并保存)。
- [ ] 独立构建、UI／原生包、适用 Server 边界完成后，才列为仅待真实 Service Key／部署／费用验收。

## 参考

- [剩余范围总览](magpie-remaining-2026-11-13.md)
- [Command Code 合同](commandcode-chat-contract-2026-11-10.md)
- [SenseNova 合同](sensenova-token-plan-contract-2026-11-10.md)
- [SAP 合同](sap-aicore-chat-contract-2026-11-10.md)

本文件是批准名单、顺序与未完门槛的记录，不是三项完成回执。

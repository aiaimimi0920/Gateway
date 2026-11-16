# FreeBuff：signed session 与最终 completion bytes 传输

日期：2026-11-06。工作区 `C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`，`main` / HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`。保留 inherited dirty edits；HEAD 不是本批实现的交付凭据。承接[稳定目录选择与业务目标保存](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-06-provider-channel-expansion-freebuff-business-target.md)，不代表 R06/W04、整卡 offline_ready、P50/P60/P70 或总目标完成。

证据根：[gateway-freebuff-business-execution-20261106](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-business-execution-20261106)。最终以该目录的 `closeout.json`、`audit.json`、各阶段 receipt/冻结输入和实际日志为准。没有读取真实凭据、调用实际平台、购买会话、发生成请求、操作容器或在线实例；P34授权已经消费，不复用历史问卷。

## 1. 本批实际实现

- `CatalogSession::business_requests` 绑定当前账号 token/origin/client、设备 capability 与同一 catalog 的开放稳定 key；借用 snapshot 固定 handle/fetchId，不能混用另一快照或外来设备。
- 新 signed session wire：完整 pre-join GET、host生成 `cli:<uuid>`/attempt、显式单次 admission POST、同 claim full heartbeat、精确 attempt DELETE。session无body，签名覆盖空bytes；不构造 Desktop购买身份，不 takeover/renewal/旧账号级泛化release。
- 新 final completion wire：校验最终 body 的模型为该快照 handle，按调用者预算边序列化边限制，冻结一份bytes；对实际bytes签名，再将同一bytes交给固定completion路径。没有二次`.json()`、caller headers覆盖或猜测的catalog-protocol header。
- 固定origin/path、无HTTP retry/redirect/unsigned fallback；session send+decoded body最多20秒/1 MiB；completion请求总timeout最多300秒且继续覆盖返回后的body。取消/drop不创建后台重投或自动DELETE。
- scope/claim/body的Debug不显示材料，错误仅固定安全代码。`BusinessReply`保留HTTP和内部原始JSON，不是公开DTO或active/refund能力；transport2xx不能直接放行业务或记退款。

详细API、公开来源和信任边界见[业务wire合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/provider-freebuff-business-wire.md)。

## 2. 联网证据与不猜协议

本轮联网读取官方固定 `CodebuffAI/freebuff@6f567ec21542afc07dbe447bce0dcaf7a5a73a8d` 的35个完整公开文件，逐文件Git blob校验一致；URL/blob/SHA-256清单为证据根的 `source-references.json`。不执行第三方代码，也不将固定commit当最新线上服务或发行binary回执。

公开CLI证实：admission用dedicated路径和handle，wallet=`session`不是金额上限，价格/余额变化须重新确认；active有必填instance/model/期限字段，admission没有`queued`状态。正确catalog root是`base3-free-catalog`，不是key/handle或旧agent；本批尚未创建agent/run。

dedicated reuse只核到路径和“精确live instance、不买或抢占”的常量说明，没有完整method/body/headers/handler，因此没有照抄admission凑reuse。CLI的模拟幂等测试不是公开服务端事务证明；当前POST不自动重投。未证明的Desktop、renewal/takeover、独立lease token同样不发明。

## 3. 已执行检查与覆盖边界

- 首个新feature红测因API尚不存在产生E0599/E0433，exit101且输入无漂移；实现后定向通过，不将它描述成既有生产bug复现。
- catalog协议定向 **36/36**，含12项新增正常feature测试；最终FreeBuff定向 **232 passed / 1 ignored**。ignored为需要独立浏览器/显式目录的旧host，本批未启动；两个阶段有重叠，不相加。
- 真实随机回环HTTP覆盖最终Unicode/转义JSON原样验签、冻结后原对象变化、handle/fetch同步刷新与fetch缺省/空串、CLI/Desktop inspection、claim/scope负例、错误状态不重投、五类redirect不携密、编码后请求上限、响应cap/坏JSON、send+body统一deadline、caller取消、body Drop和返回response的总timeout。成功用例正常关闭并join peer；没有浏览器/真实管理TCP/SQLite公共业务联合验收。
- 生产runtime-defaults lib check与官方全workspace Rustfmt check通过。仅关闭FreeBuff渠道的完整定向 **158/158**，包含1项新增关线行为测试。四个最终阶段各3252项选定输入均无漂移；后续审计再次逐项核对最终文件。关线构建保留本模块条件编译未使用字段/常量警告和既有警告，不声称零warning。
- checker **31/31**、ratchet **4623 files**，14 immutable runtime、governed source >700=0。本批9源码最大288有效行，6新增源码最大288；没有改baseline/exception。catalog入口163→177、device91→107、transport155→155，均未触及旧超限文件；root报告运行前直接备份并原字节恢复。

本批没有前端改动，不重复上一批101项UI/类型/build或浏览器检查；没有出EXE或执行正式发布矩阵。最终来源、UTF-8无BOM、scope差异、链接、`git diff --check`与关线结果，以最终audit/closeout为准，不报全仓绿。

## 4. 下一批必须接通的真实入口

当前正常管理/UI和公共pipeline **没有调用这组新业务wire**；目录模式仍在网络前返回 `409/freebuff_business_admission_required`，不因库API出现而自动购买或冒充已可推理。

接下来优先完成当前需求的管理/执行owner，而非再做一个平行传输层：

1. 从signed session snapshot解析真实active/费用/余额/价格/退款字段；由正常管理身份、Secret Grant和revision保护逐次明确消费确认，变化后重新提问。只选目录/保存配置、`MaySpendFreebucks` marker或Rust一次性对象都不能替代此流程；服务端无金额ceiling须如实说明。
2. 在发送前持久化精确claim/pending，补unknown对账和进程恢复。现有claim只是进程内类型；timeout/取消/坏响应可能已扣费，不新建购买蒙混恢复，也不把DELETE当自动退款证明。
3. 按正确catalog agent/run合同接新执行owner、同账号/目标/model的active期限与请求metadata，再接公共JSON/SSE、probe、取消/stream lease和既有正常shutdown。不得删除旧guard后直接走旧自动session或unsigned completion，不发送虚假零credits记账。
4. 继续新设备/目录应用UI→真实管理TCP联合验证、Server秘密/事务owner、跨页面/进程与原生包；其他原定渠道与P50/P60/P70均保留，不因reuse证据不足停掉其他可做工作。

本批新增模板0，累计仍16，整卡offline_ready增加0；总目标保持active。未提交、推送、部署或交付新包。

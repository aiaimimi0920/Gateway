# 服务商分渠道扩展：P40 Perplexity 正常入口与公共调用

> 2026-11-04；本批正常接线、管理保存及feature合同已完成定向离线验证：246个不同Rust测试通过，前端161项通过。R12/W08、P50/P60/P70及总目标保持执行中，不把第11个新增模板算作完整平台卡。
> 工作区 `C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`；`main`，HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`。起点483项前序/并行脏改受保护，未提交或推送。

## 1. 实现范围

在P37单轮底层、P38材料/目标保护与P39来源保真之上，接入独立 `perplexity-web` preset、`perplexity_web` profile/special、`perplexity_web_compatible` adapter和独立leaf feature。runtime-defaults启用该line；关闭feature时真实HTTP transport不编入，材料身份与受众验证仍保留。Sonar/Search的Key、preset和原实现不替换、不合池。

前端正常catalog新增空池模板，沿统一材料入口精确调用严格Perplexity parser，复用正常apply、秘密暂存、SecretPatch和原revision/CAS。原会话UA纳入暂存绑定；删除、改值、大小写重复或provider继承均拒绝，session/clearance/显式expiry取最早值后在普通提交阶段继续保留。没有自动账号、模型目录、路由、探测、权益或蓝色验收状态。

普通buffered/stream dispatcher先prepare，再强制 `Session::from_payload`，固定HTTPS消费端URL、Cookie和原UA；不转发caller Authorization/Cookie/UA或client默认API headers。选路后必须为 `turbo` pinned auto/concise策略，caller alias可以映射；仅支持单user、单纯文本part、单轮Chat/Responses。全局timeout可能大于120秒，普通执行取两者较小值，低层显式deadline仍严格拒绝零值/超过120秒。

`stream=true`等待完整上游后才输出canonical SSE，不是实时token streaming。最终答案与同snapshot来源经明确`gateway_sources`保留，usage/实际后端model仍未知。Messages、其他模型策略、tools、reasoning、多轮、媒体、未支持参数、通用raw JSON/binary及compact快捷入口在网络前拒绝，不改body偷跑。generic planner gate保留，只将旧“未接线”错误换为专用请求计划不支持。

精确账号probe只有显式model test才调用同一dispatcher，无request返回Unsupported，不伪造`/models`、自动生成或注入不支持的max_tokens。复用正常probe审计/health owner，完整非空stop才算通过，usage为null；错误只导出受限code，回复清除所选材料并限长。

## 2. 运行证据

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-provider-expansion-20261104-p40`。仅合成材料、随机loopback端口及非转发proxy兜底，不读`.ng`、历史账号或真实上游，不需要Redis。

| 检查 | 实际结果 |
| --- | --- |
| 行为红测 | `red-dispatch.log`：两项dispatcher测试因旧`perplexity_web_dispatcher_unavailable`实际失败；`red-ui.log`：9项失败，证明模板与普通材料入口原先未注册 |
| 后端过程失败 | `green-rust-v1.log`：新测试tuple的E0283，修正类型推断；`green-rust-v2.log`：28 passed/4 failed，四项都在配置构造时因复制了备用credential ID而被正确拒绝，尚未进入pipeline。仅给备用账号独立ID，不放宽生产唯一性或no-fallback断言。`integration-enabled.log`另因外部测试调用private snapshot方法而E0624；改用既有public store读取及正常HTTP回包的脱敏document/secrets，不扩大生产可见性 |
| 最终Perplexity Web定向 | `green-rust-v3.log`：32 passed/0 failed；包含真实管理router精确probe、公共Chat/Responses buffered/聚合SSE、来源/unknown usage、alias→turbo、精确credential audit、授权/不支持请求零发送、401/429/不完整终态不重试及取消释放audit/permit/连接 |
| 相邻官方Perplexity/API/Search | `regression-perplexity.log`：43 passed，含上述32项，不重复累计；API与Web仍各自使用原URL/身份 |
| registry / lines / call_flow / routing | `regression-registry.log` 22、`regression-lines.log` 37、`regression-call-flow.log` 40、`regression-routing.log` 34 passed；存在重叠，不能直接相加 |
| 正常管理保存及八产品往返 | `integration-enabled-v2.log`：配置/秘密71项、raw planner gate 1项、enabled feature 1项、正常管理保存1项，共74 passed；两个合成fixture opt-in均显式执行，无ignored。普通保存验证missing grant/wrong origin、正常SecretPatch/原revision/CAS、秘密不回显、Session/原UA、stale409及改UA后Keep拒绝 |
| feature-disabled | `feature-representative.log` exit0；`integration-disabled.log` 2 passed，在`line-gemini-web-reverse`代表性配置下证明line/profile/preset一致关闭、公开execute返回compiled-out且loopback trap未连接、材料受众保护仍存在。不声称全feature矩阵 |
| 前端 | `green-ui-final.log`：11 files/161 tests passed；`typecheck-final.log` exit0，包含最终TS源码及八产品合成fixture导出 |
| manifest / checker / ratchet / Rustfmt | `manifests.log` 45项/0 errors；`checker-tests.log` 31/31；最终`ratchet-final-v2.log`和`rustfmt-final-v2.log` exit0，checker/baseline/exception未改 |
| 最终源码与diff审计 | `audit-final.json`覆盖52项（36 existing/16 new），UTF-8无BOM、尾空白和独立before→after diff检查；非文档源码均匹配`source-frozen-v3.json`，最终文档hash见`source-final.json`。仓库`git diff --check`另见`git-diff-check.log` |

`test-summary.json`按target与完整名称去重，最终**246个不同Rust测试**：lib171、配置/秘密71、raw gate1、feature2、管理保存1。日志共284次通过，包含32项Web重复回归、跨filter重叠和enabled/disabled的同名映射合同，不能直接相加；不累计红测、旧版本及失败编译。所有最终运行在对应冻结源码上完成，仅文档随后更新。没有运行全仓、真实上游、打包或正式发布门禁。

公共失败配置保留可用备用provider，检查每个已发送失败仅一次wire调用，不是靠删除fallback候选获得“没有重试”。取消覆盖完整上游聚合等待期间的buffered/stream两条路径，记录cancelled、中性health/unknown usage及归还permit；不将此外推为所有下游断连、Server/PG或故障时序的穷尽验收。

后端统一`--locked --jobs 4 --no-default-features --features runtime-defaults`，TEMP/TMP/TMPDIR为本批linshi，测试进程`RUST_MIN_STACK=16777216`。不启用默认management-ui发布/删除hook，不升级依赖、不清缓存，Cargo串行。等待期间另项目的Rust构建不属于本任务，未停止或干预。

本批确实执行了仓库标准`npm run check:effective-lines --prefix scripts`；它会写`scripts/artifacts/effective-code-lines.json`，该报告原先已dirty且没有本批前副本，不能声称保留了其原字节或猜测恢复。后续最终ratchet使用相同checker的无报告写入入口，未改治理baseline或exception。

44个受测量源码均不超过460有效行：client454→460、request_plan_part_2432→442、passthrough411→417、console_model_probe359→369、line catalog325→330、profile inference248→250、原Perplexity upstream入口163→22；新transport189、公共pipeline225、失败合同140、probe157/测试127、管理HTTP合同139、feature外部合同67、前端新合同72。完整前后值和hash由冻结/审计记录，不按物理行数制造超限豁免。

## 3. 未完成与接力

P40是执行批次号，不等于总计划中“P40媒体”工作包已完成。模板现11个，新增Session产品现8个；模板可添加、一个明确子集的离线公共调用通过，都不等于R12/W08整卡offline_ready、真实账号可用或新增渠道全部完成。

Perplexity尚未实现多轮、diff/JSON Patch、Research、附件、其他模型策略、模型目录、session refresh或自动challenge。必要能力和协议漂移需继续有据冻结；订阅额度与Sonar/Search、真实session/challenge/终态及来源分布留到集中录制验收，不伪造余额/模型身份。

Accio/Kiro/Vertex适用OAuth、Grok跨revision/永久Sending/其他refresh与durability、device/listener及跨Console/进程保存恢复、完整业务保真，Qwen旧OAuth可行性和各API剩余保真/Server边界、Vertex Express与Luma Agents新owner，以及其余Web/媒体卡仍按总计划推进。P50无凭据集成/包、P60集中录制、P70逐渠道真实调用及额度验收均未完成。

没有读取或试用历史凭据、真实上游调用、付费生成、创建Redis/Docker资源、出包、替换运行实例、提交或推送。P34容器授权已在前序消费并精确清理，本批不复用。

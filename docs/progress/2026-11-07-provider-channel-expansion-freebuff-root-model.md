# FreeBuff：同目录快照的 root 模型输入与 compiled 身份

日期：2026-11-07。工作区：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`；`main` / HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`。继承未提交改动，HEAD不是本批来源凭据。本批模型输入链已实施并完成下列终态验证，不是R06整卡或总计划完成。

## 1. 承接的真实缺口

前一批[统一步骤owner](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-07-provider-channel-expansion-freebuff-step-owner.md)已接prepare/send/decode的原run、预算与协作取消；但目录parser丢弃root所需的`compaction/legacyDigests`。本批补网络目录→parser→同一row选择→`BusinessRequests::root_model_input`的输入链，不把一段opening、元数据或可解析目录冒充完整harness/公共业务。

`RootCompactionPolicy`按固定公开schema读取required cacheExpiryMs/cacheExpiryMinTokens、optional maxContextLength/lean。字段整体可缺，旧目录仍可浏览/选择；明确null、错类型、非整数或非法数值拒绝，不把坏值降成默认。maxContextLength缺失仍为None，不从opaque handle、显示名或任意默认值猜token预算；lean是builder指导，不作为runtime compactContext已实现的证明。JSON safe-integer上限、legacyDigests最多64项/单项1024字节且无control是Gateway本地准入政策，不声称上游schema具有这些额外上限。

`SelectedModel`从同一row借用handle、displayName、policy和legacyDigests，同时保留同snapshot fetchId。内部`RootModelInput`不Serialize、Debug隐藏目录值，不落盘/进入管理DTO/秘密保存；本地读取不发网络、START、购买或推理，也不是准入capability。

输入层保留None，不等于后续builder可永久拒绝缺policy的旧row。固定base3在缺policy时确有`compactionPolicyForModel(model)`默认：opaque handle不命中明文DeepSeek ID，cache idle/floor为3600000ms/140000；这不是已知模型context budget，也不能因compiled身份为Flash就偷偷改成其15分钟分支。后续须按真实builder/runtime合同接默认与上下文预算，不把“任意默认不可取”误写成“固定来源默认不存在”。

## 2. 精确 compiled 身份和模型提示

固定参考的`SUPPORTED_FREEBUFF_MODELS`为21项。按row的legacyDigests原顺序取第一个已识别身份，不是扫描到任何flag就合并；unknown、空串、未来格式兼容且不猜模型，大小写精确。静态digest/id pairs只用于root输入提示，wire model仍是原handle，不用compiled ID做路由fallback、权益/额度或认证判断。固定公开身份与名字不代表当前平台仍可用。

按相同固定base3合同计算turn-ending-note和CLI batching提示，lean=true禁用batching；这些只是交给后续builder的输入，尚未插入prompt或执行工具。21项静态表避免每次选择重新hash/分配，查询上界为64×21；独立JavaScript UTF-16/Math.imul向量与Rust重实现核公开FNV digest，含中文/代理对和dated ID，不执行第三方TS。

## 3. 来源、验证与失败分类

本批不重新联网，复核同一公开commit `6f567ec21542afc07dbe447bce0dcaf7a5a73a8d` 既有归档SHA-256 `5065429370f6147aa512cbf23792aa5fd916931e0403febafd4eafa8adee3edc`。11份catalog/schema、policy、model/ID/entitlement/config、free-agents、base3、CLI builder、compiled lookup和Apache-2.0 LICENSE原文的逐项SHA见[证据根](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-root-model-20261107)的`reference-provenance.json`。`compiled-model-manifest.json`静态读取21个row/ID符号及其定义，`digest-golden.json`为独立向量；不复制/运行SDK runtime，不声称当前服务端合同已验。

首次参考提取预期的`docs/freebuff-model-catalog.md`不在归档，工具以assert失败；后续明确记录该文件和`docs/freebuff-base3-harness.md`均缺失，不猜其内容。与上批一样，注释引用的服务端root校验实现仍无公开原文。失败分类见`reference-extract-v1-failure.json`。

`red-v1`是真实行为红测：4 failed/0 passed，错误policy/digests原被当作未知字段丢弃而接受；5197项选定输入无漂移。不是编译失败，也不把预期失败当通过。修复后首次`focused-v1`完整13项终态通过，无失败重跑或降低断言；新增13项完整身份与前批393项逐项集合对账，旧项无丢失，不叠加选择器重复或历史通过数。

| 终态检查 | 实际结果 |
| --- | --- |
| `cargo test --locked --offline --lib freebuff_root_model_ -j 4 -- --test-threads=1` | 13 passed / 0 failed / 0 ignored；含两项真实回环TCP catalog/registration/signed refresh |
| 正常lib选择`freebuff` | 360 passed / 0 failed / 2 ignored；两个旧browser host未显式执行 |
| 正常lib选择`pipeline_lifecycle` | 6 passed / 0 failed |
| 正常lib选择`local_runtime::` | 49 passed / 0 failed |
| 同一lib harness按完整名去重 | **406 passed / 0 failed / 2 ignored**，其中13项本批新增 |
| 默认生产lib、仅移除FreeBuff的代表性关线生产lib | 两项locked/offline/jobs4检查通过；关线不启用management-ui，不冒称完整feature矩阵 |
| 8份Rust官方`rustfmt --check` | 通过 |
| `npm.cmd run test:effective-lines --prefix scripts` | 31 passed / 0 failed |
| `npm.cmd run check:effective-lines --prefix scripts` | ratchet通过，4887文件；governed source >700为0，不等于strict或全仓功能通过 |

9个终态阶段各5201项选定输入before/after无漂移，且与最终源码逐项一致。8源码、6文档共14份范围文本UTF-8无BOM、写前hash、逐文件preimage/new diff与`git diff --check`通过；公共guard/baseline/exceptions保持原bytes，根报告按直接preimage恢复，scripts报告未变，本批报告另存证据根。11份固定参考与21对静态digest/id逐项核SHA/独立manifest/JavaScript向量通过。见`audit.json`、`test-identities.json`、`scoped-diff.patch`和`effective-code-lines-final.json`。

官方lexer按真实preimage/终态复测：catalog index 201→209、model 142→159、business 269→272；5份新增policy28、root input72、policy tests76、root tests238、wire tests122有效行。8份最大272，新增最大238；无baseline/exception调整。安全/资源审查：复用原1MiB catalog上限与目录/固定origin/签名owner；仅边界反序列化时校验，不在getter重复校验可信数据；有界静态lookup无每次hash/分配、后台任务/新队列/重试，无新存储格式/凭据写入。两个peer均正常await close，最终只读快照本批verifier/Cargo/rustc/lib test均为0，未终止其他项目进程，见`final-process-inventory.json`。

本批只对代表性关线做生产lib编译，未重跑其单测；新增模型输入/lookup生产函数无feature分支，正常配置单测与相邻选择实际覆盖，完整matrix留在P50，不以本批代替。未跑前端/浏览器/全仓、独立进程/原生EXE/整包或真实平台；现有warning如实保留，未做无关cargo fix。

## 4. 接力边界

仍需按框架职责完成root/prompt、实际上下文预算/compaction与step/instructions提示消费、host工具/child及其费用归属；不得将Loom或其他产品实现搬入Gateway，也不能因有canonical opening就宣称支持完整root。公共producer/正常UI逐次消费确认、三公共协议/closeout/下一次购买/probe、START/FINISH及公共请求取消和持久run/远端lease、Server/跨进程/原生包与真实平台仍缺。

公共`freebuff_business_admission_required` guard保持原bytes，模板仍17、无新增整卡offline_ready，P50/P60/P70和总目标继续。不读取真实凭据、不调用真实供应商、不操作Redis/Docker/现有实例、不出包、提交或推送，不复用已消费P34授权。

# Luma Agents 公共媒体入口、原生成功事实与本地结算

日期：2026-11-06（Asia/Shanghai，测试于 11 月 5 日启动）。状态：本子批定向验证通过；不是 R14/W10、P50 或总计划完成。
仓库：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`，HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`。
承接 [真实管理/UI 与持久化重建](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-05-provider-channel-expansion-luma-agents-local-ui.md)；本批不重复开发模板、UI 或底层生成执行器。

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-luma-pipeline-20261105`。
所有材料为合成值；无真实凭据、平台调用、Redis/Docker、在线实例、出包、提交或推送操作。P34 历史容器授权已消费清理，没有再次使用。

## 1. 四个实际生产补漏

1. **引用式图片编辑准入**：公共 `/v1/images/edits` 的非空 `source` 复用现有 prompt canonical normalizer，再明确为 ImagesEdits；URL、generation_id、file_id、inline 图像不再被“必须本地上传”提前拒绝。产品 source/参数仍在 auth、路由和实际型号解析后由 Luma mapper 校验；无 source 的原上传路径不变，未修改 Gemini/S06 内部实现。
2. **真实上传字段兼容**：Luma mapper 接受正常 multipart/JSON parser 的 `base64`，保留 `base64_data`/`data_url`；两种 base64 或 data_url 与任一 base64 混用明确拒绝，不修改公共 parser 的兼容语义。
3. **原生成功事实审计**：仅当实际 selected adapter 为 `lumalabs_agents_compatible`，将已由正式 sender 验证的生成 UUID/model/type/state/usage.credits_charged 白名单写入既有 route_trace，并以生成 UUID 写 response_id；不复制资产 URL、输入引用或任意 metadata。其他 adapter 不能伪装扩展，原 audit schema/storage version 不变。
4. **非媒体 planner 边界**：扩大负测发现该 adapter 曾落入 generic custom HTTP planner。现明确返回 `luma_agents_request`，两种 stream 参数均在网络前拒绝；受支持的图片/编辑/视频继续走正常 JSON dispatcher，不新增替代网关或测试专用生产接口。

## 2. 验证的实际链路

测试使用生产 `runtime::build_app_state`、正常 management router 创建普通公共 Key/充值、正常 public router `oneshot`、真实 loopback HTTP 上游和真实 Local SQLite audit/balance owner。无 PG，Redis pool 关闭，UI/同步和真实后台服务关闭；没有手写 SQL 或改变持久化版本。

普通接入 Key 经过真实 auth/模型权限/预占；`public-luma-image → uni-1`、`public-luma-video → ray-3.2` 经正常 model map，精确选择 `exact` credential。同池保留 unrelated/disabled 账号，另有可用 `luma-backup` 与不同 Key，错误用例明确断言两候选存在但不 fallback。

上游替换仅复用现有 cfg(test) 的两个精确 task-local origin remap；loopback fixture 从不转发 CONNECT。它不能证明生产公网 DNS、TLS 或真实 CDN 的实际连接成功。
默认线程栈、boxed bootstrap；没有设置 `RUST_MIN_STACK` 或复制大栈 runner。测试末尾 drain/shutdown、等待 AppState 引用释放并关闭 SQLite/FreeBuff，wire server abort 后 await。

**本批不是前端真实 TCP、原生 EXE 或进程重启验收**；上批正常 UI/真实管理 TCP 的证据独立保留，不能把两批拼成未执行的单次浏览器生成流程。

## 3. 成功、失败与额度边界

- 三个公共操作：images generations、JSON images edits、videos generations，均为一次 POST → 同 UUID GET poll → asset GET；另验真实 multipart PNG 与四种 source 引用。
- 精确实际型号/credential/Bearer 与审计身份保持；caller Cookie/Origin 不转上游，资产下载无认证头；成功审计不含 Key/签名资产 URL，Token 字段仍为 None。
- 普通 `message_prepaid` 成功由 2→1；307/401/403/429/503、异步 failed、错误 UUID、私网资产、假 PNG、资产跳转均不得成功，恢复为 2。HTTP 错误明确 candidateCount=2、fallbackEligible=false，实际仅一次 POST。
- submit/poll/asset 三阶段分别验证 deadline 和 caller Drop，关闭 HTTP source、释放 permit、退款；caller cancellation 不增加模型失败计数，失败/取消没有虚构 winning profile。
- 无效公共 Key、无权模型、n=2、stream、错模态、chat 入口、坏 source/混源/重复 base64 均零上游网络。
- 图片使用真实完整 PNG；视频只使用结构性 ISO-BMFF/MP4 fixture，**不证明可播放或解码**。

三种账务事实必须分报：

| 项目 | 已证明 | 没有证明/没有实现 |
| --- | --- | --- |
| 普通公共 Key 次数 | 成功一次扣一次，范围内失败/取消退款 | 退款不等于远端生成取消或供应商退费 |
| 普通公共 Key Token | 缺真实 token usage 时按既有规则保留估算预占；0.25 credits 不换 Token | 未获得真实媒体 Token，用例不是精确 Token 计费通过 |
| 供应商 native credits | 成功且资产验证后将 null/0/0.25 原样保留到 audit | 不是完整费用账本；失败/取消/晚到扣额仍未收口，cash 媒体结算未实现，不把 credits 当 USD |

不承诺客户端重试/跨进程 exactly-once；不因未知费用补 0、自动重提生成或执行远端 DELETE。

## 4. 实际测试与编译

所有 Rust 阶段为 `--locked --no-default-features --jobs 4`；测试及生产主组合为 `--features runtime-defaults`，不启用默认 management-ui，不触发 Web publisher/删除 hook。

| 回执/过滤器 | passed / failed / ignored |
| --- | --- |
| green-v2 / luma_agents | 32 / 0 / 0（12 项本批新增，含 2 项 audit unit；20 项原有） |
| adjacent-finalize / pipeline::stage_finalize::tests | 10 / 0 / 0 |
| adjacent-images / http::routes::images::tests | 2 / 0 / 0 |
| adjacent-gemini / protocol::gemini_business | 32 / 0 / 0 |
| adjacent-planner / build_request_plan | 8 / 0 / 0 |
| adjacent-call-flow / upstream::client::call_flow_tests | 1 / 0 / 0 |

按完整测试名去重 **85 个 Rust 测试通过**，没有把零用例或前批 238/382/2 项加入本批累计。
green-v2 编译 12m21s、测试 17.09s；相邻同源码过滤器直接复用同一测试二进制，Cargo 1.37–5.90s，没有宣称增量编译的通用提速倍数。

- 生产非测试库：`cargo check --locked --no-default-features --features runtime-defaults --lib --jobs 4` 通过。
- Luma 关闭代表性组合：`cargo check --locked --no-default-features --features line-gemini-web-reverse --lib --jobs 4` 通过；不是全 feature 矩阵，也未声称已知 Gemini 全关闭缺符号问题消失。
- 编译保留现有 warnings，不执行 cargo fix 或无关清理；没有构建新 EXE 或重建 UI。

## 5. 规模、静态与来源核对

最终官方 lexer 计数如下；有备份的文件列备份值，未保留改前正文的两个已有文件不猜造前值。五个新文件分别为 audit owner 与四个 pipeline fixture/tests。

| 源码路径（相对本仓库，完整绝对路径见 scope 回执） | 备份 → 最终有效行 |
| --- | --- |
| src/protocol/luma_agents.rs | 85 → 92 |
| src/http/routes/images.rs | 389 → 393 |
| src/protocol/luma_agents/media_ref.rs | 未记录 → 96 |
| src/pipeline/stage_finalize/luma_agents_audit.rs | 新增 58 |
| src/pipeline/stage_finalize/audit_persistence.rs | 196 → 200 |
| src/pipeline/stage_finalize.rs | 134 → 135 |
| src/upstream/client/request_plan_part_1.rs | 165 → 168 |
| src/upstream/luma_agents.rs | 60 → 64 |
| src/upstream/luma_agents/test_support.rs | 未记录 → 213 |
| src/upstream/luma_agents/pipeline_fixture.rs | 新增 208 |
| src/upstream/luma_agents/pipeline_tests.rs | 新增 167 |
| src/upstream/luma_agents/pipeline_failures.rs | 新增 187 |
| src/upstream/luma_agents/pipeline_admission.rs | 新增 95 |

最大 393，无 501+ 文件或新增例外；没有变更 policy、baseline、exception。已逐文件审查 source/secret 边界、有界数据、任务/连接退出与费用语义，未引入 detached 生产任务或第二套计费 owner。
Rustfmt、完整 git diff --check、包含未跟踪文件的 scoped whitespace、UTF-8 无 BOM 通过。checker tests 本批已 31/31 通过且脚本未变化，最终不重复；最后 planner/fixture 修补后的 ratchet 再次通过，扫描 4524 文件、4510 governed source 无 >700，14 immutable runtime artifacts 独立分报，violations/warnings 均空。共享 artifacts 报告运行后按原字节恢复，实际本批报告独立保存。

green-v2、五组相邻测试和两个生产编译阶段各冻结 **3025 项选定 Rust/配置/manifest/test 输入**，运行中及最终源码均无漂移。最早 red-v1 之后变更/新增路径仅在上述 13 项内；该快照已有最初测试夹具，不是整批开发前的干净工作树。其他既有 dirty edits 原样保留，不据 HEAD diff 误认均为本批修改。

## 6. 失败记录与分类

1. red-v1 为测试编译错误：audit view 没有 route_attempt_count，response_id 为 String 而非 Option；按真实类型修测试，不扩大生产 API。
2. red-v2 为 fixture 错误：公开别名误放 supported_models，只修为真实型号/独立 model_map，不放宽配置。
3. red-v3 为真实行为红测，0 passed/3 failed：response_id 仍是 gw-audit-*、上传编辑 luma_agents_request、source 编辑 missing_input_image；三处生产补漏后全部转绿。
4. green-v1 扩大负测后为 27 passed/4 failed：发现 generic HTTPS/CONNECT 逃逸及 fixture 无 path unwrap 导致 mutex poison，修 planner 与只读观测；另外失败/取消本来没有 winning selectedProtocolProfile，只修测试为 completed 有 profile、其他终态为 null，精确身份/一次 attempt 断言不放松。
5. 首轮静态 runner 的空 stdout 写日志失败，以及错误传绝对 --json 路径导致 CLI exit 2，均不是 formatter/规模失败；后续按真实 artifacts 报告与空输出安全处理完成。
6. 最终 scoped verifier 首轮误把 git diff --no-index 的普通差异 exit 1 当 whitespace 错误；原 JSON 保留为 scope-initial-verifier-error.json。以合成 clean/尾随空格两个控制样本验证退出码/诊断语义后修正调查脚本，源码未变，未豁免真实 whitespace 输出。

所有 red/green 日志及输入快照保留，不把失败改写为从未发生。
主要证据：[最终收口](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-luma-pipeline-20261105/closeout.json)、[来源/测试名单/规模](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-luma-pipeline-20261105/scope-final.json)、[最终 Luma 测试](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-luma-pipeline-20261105/green-v2.json)、[最终 ratchet](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-luma-pipeline-20261105/ratchet-final-report.json)、[静态回执](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-luma-pipeline-20261105/static-final.json)、[资源审计](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-luma-pipeline-20261105/resource-audit.json)。

## 7. 后续接力边界

本批闭合的是 **Luma Local 公共媒体调用、范围内失败/取消、本地次数结算与成功原生事实**。模板累计仍 16，不新增整卡 offline_ready。

- 精确账号媒体 probe 仍需显式操作/费用授权合同；不能把现有默认文本测试偷偷转换为付费生成。
- video_edit/video_reframe/layering、Files/LLM 等其他实际范围、HDR+10s 文档矛盾、完整 native 费用/晚到费用、cash 结算继续。
- 真实 CDN/DNS/TLS、视频播放、Server Redis/PG/Hosted usage aggregation、强退/原生 EXE、旧 Web 渠道与真实额度关系未验。
- 其他原定渠道/OAuth 兼容恢复/Qwen 受限项、P50 全范围无凭据集成与来源正确 devtest、P60 集中录制、P70 逐渠道真实模型/额度/刷新/重启/产物验收全部保留。现在不要求录制凭据，也不将总 goal 标记完成。

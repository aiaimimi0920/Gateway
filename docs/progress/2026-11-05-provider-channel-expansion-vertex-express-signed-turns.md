# Vertex Express 有序签名 turn 与显式历史往返子批

日期：2026-11-05。范围：R11/W07 的固定 v1 text／完整 functionCall 状态保真。
这是一个开发子批，不是完整 Express decoder、R11/W07 整卡 `offline_ready`、P50 或总目标完成。
新增模板累计仍为 14；真实材料、模型、刷新和额度留到集中验收。

## 改了哪些内容

1. `CanonicalRelayResponse` 增加缺省不序列化的可选 `vertex_turn`。独立 typed owner 保留实际请求上游模型、
   原 Part 顺序和边界、空 text、显式 thought boolean、原样 opaque thoughtSignature、完整调用与公共 ID。
   thought summary 不混入普通答案，不冒充 Anthropic 原生 signature 或 OpenAI encrypted_content。
2. Express 独立 projection 在公共工具 ID 分配后建立 turn；三公共协议 JSON 根对象输出
   `gateway_vertex_turn`，Chat/Messages/Responses 的 SSE 按各自 finish 位置只输出一次。
   仍是聚合后桥接，不是真实实时 Google Part／参数流。
3. `gateway_vertex_history` 显式指定准备后 model Content 的 ordinal，恢复前验证实际模型、普通文本、
   工具数量／顺序／公共 ID／名称和 object 参数。原样恢复 Google Parts，公共 ID 不发 Google；
   保留既有 v1 按公共 ID 聚合、排序工具结果并移除 vendor ID 的合同。
4. 原始 Parts 不能直接注入 thought/signature 或 snake-case function_call/function_response。
   有签名历史时 Chat/Messages 的有效 contents 覆盖在取 plan contents 前拒绝；Responses 不扩原 extra 白名单，
   被忽略的 contents 不会覆盖已验证历史。不允许跨产品回输，不补签名或降级为无签名历史。
5. Chat 签名历史和 typed 响应的工具 JSON 参数不做旧 HTML entity repair；带 turn 的响应不执行字面 XML
   tool markup 注入。无扩展旧行为保留，避免修补签名路径时改变原服务商兼容合同。
6. SSE 新增有界 projection witness：文本只存 SHA-256 增量摘要，工具 ID／名称／参数合计最多 4 MiB／4096 项。
   验证扩展与实际输出一致，拒绝早到、重复、同帧增量、接受扩展后的新内容及错误层级。
   Responses→Chat 在实际输出队列出队前验证，不仅信任 terminal snapshot；不重新解析 opaque native passthrough。
   无扩展旧流超预算只丢 witness，不改变旧流；后续声明扩展则拒绝。

具体 wire、ordinal 规则、预算与客户端要求见
[API 合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/provider-vertex-signed-turns.md)。

主要 owner：
- [typed turn](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/vertex_turn.rs)、
  [wire 校验](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/vertex_turn/wire.rs)、
  [流 witness](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/vertex_turn/stream.rs)
- [Express 原 Part 投影](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/vertex_response/turn.rs)、
  [显式历史恢复](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/vertex_history/signed.rs)
- [真实公共调用测试](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/client/vertex_express_pipeline_signed.rs)、
  [失败与输入边界](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/client/vertex_express_pipeline_signed_failures.rs)

## 公开合同与保守拒绝

本轮已联网只读核实，无实际平台请求或材料：

- [Google Cloud thought signatures](https://docs.cloud.google.com/gemini-enterprise-agent-platform/models/thinking/thought-signatures)：
  要求原 Part 回传，不合并签名 Parts；text／functionCall 可带签名，流式末尾空 text 也可能携带签名。
- [Content REST](https://docs.cloud.google.com/gemini-enterprise-agent-platform/reference/rest/Shared.Types/Content) 及
  [v1 content.proto](https://github.com/googleapis/googleapis/blob/master/google/cloud/aiplatform/v1/content.proto)：
  Part 级 thought、base64/bytes thoughtSignature 和互斥 data。

签名与 Preview partialArgs/willContinue 的精确关联没有足够合同依据；两者并存时静态 502、非重试 Abort，
不把签名移给重组调用。未支持的带状态媒体／代码／额外字段、signature-only Part 与 snake-case 上游签名均拒绝。
无状态 Preview 构造子集保持。每 turn ≤4096 Parts／4 MiB、每签名 ≤1 MiB，历史 ≤64 条／8 MiB；
签名仅作 base64 语法校验并原样保存，不解密、自行验签或推测 TTL／跨账号资格。

两个特殊字符串 `skip_thought_signature_validator`、`context_engineering_is_the_way_to_go` 明确拒绝。
第二个是 Gateway 保守政策：本轮旧 Gemini API URL 已重定向到 AI Studio Interactions 文档且不再包含它，
不能冒称已新联网确证其当前 Cloud API 合同。来源原文见证据目录的 `public-sources.json` 和 `public-sentinel-source.json`。

## 实际验证

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-vertex-express-signed-turn-20261105/`。
所有 Cargo 串行、jobs=4，明确关闭 management-ui；没有设置 RUST_MIN_STACK、放大线程栈或用新 task 绕开失败。
源码在每次 Cargo 和同 EXE 回归期间冻结，原始失败日志与 inputs receipts 保留。

| 检查 | 结果与证明边界 |
| --- | --- |
| 生产未改红测 | `red-signed`：1 passed／4 expected failed，复现 state 缺失、尾部空签名丢失、非法签名被当成功及公共往返缺扩展。首次仅测试 frames 可见性编译错误不算功能红测 |
| 首轮绿色 | `green-signed`：79 passed／0 failed／1 ignored；之后发生审查修复，不能用它代表最终源码 |
| 审查红测 | `red-review`：12 passed／6 expected failed，复现 forward 投影不符、晚到内容、reverse snapshot 不符、第二特殊字符串、Raw 状态绕过及 Chat HTML entity 真往返失败；对应生产缺口已修 |
| 最终第一尝试 | `green-final`：85 passed／1 failed／1 ignored。失败为测试误把 Responses 忽略的 contents 当作透传覆盖；沿原白名单修测试，并新增真实两轮断言证明它不会替换 typed 历史，不改生产白名单 |
| 最终 Express 定向 | `green-final-v2`：87 passed／0 failed／1 ignored（22 项本批新增）；命令为 `cargo test --locked --no-default-features --features runtime-defaults --lib --jobs 4 vertex_express -- --test-threads=1`；ignored 为既有 opt-in 浏览器 host，不算通过 |
| 相邻协议 | 同最终测试 EXE 的 OpenAI／Anthropic／Responses／sources／canonical／canonical SSE／Gemini facade 共218 passed／0 failed／0 ignored；输入及 binary hash 前后核实未变。与Express重叠两项，合计按名去重303项，不简单加总 |
| 格式／规模／来源 | 最终 Rustfmt、checker31项、ratchet、57项本批文件的UTF-8无BOM／scoped whitespace／有效行审计通过；2933项绿色编译输入及测试EXE未漂移，Rust变化未越出本批owner。全工作区diff-check仍失败，见下文；不能称最终全仓检查全绿 |

核心运行覆盖为 Chat／Messages／Responses × JSON／SSE：正常管理 Key、生产 local bootstrap、真实 SQLite、
非转发 loopback 上游的两轮 signed 工具往返；公共 ID 与原 Google Parts 精确恢复，消息余额 2→1→0、usage total=5。
非法历史网络前 400、失败 audit、余额不变、上游零调用；非法上游状态静态 502、无成功终帧或材料泄漏、仅一次上游调用。
安全过滤丢整个候选（文本、工具、summary、签名），保留 usage；既有错误、取消退款与 source Drop 用例一并定向回归。
以上不是 Google 真实业务／账单或 native EXE 进程重启证明。

按测试名去重为 **303 个不同 Rust 测试通过**（22 项新增），另1项 opt-in ignored不计通过。
52项Rust和5份文档的前后差异、哈希、来源核对及完整原始结果保存在
[closeout.json](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-vertex-express-signed-turn-20261105/closeout.json) 与
[scoped-changes.patch](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-vertex-express-signed-turn-20261105/scoped-changes.patch)。
所有本批Rust文件均≤500有效行；新owner分别为146（typed）、139（stream）、89（wire）、112（history）、74（capture）；
新增测试最大212。`to_anthropic.rs` 为487→500、`canonical.rs` 为476→478、反向Responses转换357→399，无新增规模例外。
ratchet扫描4409项，14项immutable runtime artifacts单列，governed source >700=0；不是strict或完整发布门禁。

**全工作区 `git diff --check` exit 2，未通过：**
- `apps/desktop/src/features/console/accountCardTypes.ts:79: new blank line at EOF.`
- `apps/desktop/src/styles/pilot-dialog.css:52: new blank line at EOF.`
- `apps/desktop/src/styles/provider-account-library.css:346: new blank line at EOF.`

这三项不在本批写入范围，没有为本批检查改写或撤销其他lane的前端编辑。
本批包含未跟踪文件的逐文件写前快照→当前差异均已单独通过whitespace检查；全仓失败仍完整记录。
closeout审计会保存结果并保持非零退出，状态为 `scoped_checks_passed_global_diff_failed`，
不是偷偷放宽全仓门禁后报通过。没有把缺少UI写前快照的这三项断言为已证明的历史缺陷来源。

## 安全、兼容与接力边界

签名 Debug 仅打印 opaque 提示；不写入 tool raw、sources、模板或模型声明。没有新增 grant、秘密存储、DB/storage 版本、
依赖、task/thread/process/锁或无限重试。上游仍用既有整次 64 MiB bounded reader，公共扩展另有预算，资源随请求释放。
`vertex_turn: None` 缺省兼容旧 canonical 响应。共享 Gemini 的必要接线仅为五个文件七处 None 初始化，
不修改 S06 encoder/decoder/parser/translator 业务逻辑；逐文件前后有效行与哈希由最终 closeout 记录。

**客户端必须显式保留并回传扩展。** 忽略扩展仍可读旧文本／工具，但带签名工具的后续请求可能被 Google 400 拒绝，
不只是“性能较差”；不宣称所有原生 SDK 自动兼容，也不把普通 flat 内容当成原始 Part 顺序的无损表达。

继续开放：多 candidate、未支持 Preview 表示、signed Preview 的关联、全部声明 Part／模态、聚合 SSE cache 细项和真正实时参数流；
Full Vertex OAuth 独立 client/context/project/location、登录、refresh 与持久化；其余原定 API/OAuth/Web/媒体渠道，
P50 全范围无凭据集成/devtest、P60 集中录制、P70 逐渠道真实权益验收。
前批仅官方 Vertex 线 feature-off 的 15 项共享 Gemini Web 缺符号失败未在本批修复或重新跑，不能报完整 feature 矩阵绿色。

本批不改 UI，不构建前端，不执行 publish/prune、正式发布或手测包流程；未读取真实 `.ng`／历史凭据、调用真实上游、
创建 Redis/Docker、修改在线实例、提交、推送、reset 或 clean。历史容器批准不复用。

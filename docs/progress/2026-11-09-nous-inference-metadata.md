# Nous：补充公共推理与实现线清单

本轮继续原三图 42 项、红框 23 张卡及聚合子渠道目标，不将 Nous 单个切片或
manifest 数量当成全范围完成。没有修改真实运行实例、账号或数据库，没有付费调用、提交、推送或出包。

## 改了哪些内容

1. **legacy Completions 的明确边界。** 现有 Nous 实现使用 Chat 桥接；调查发现数组 prompt
   会变成连续 user 消息，`echo`、`logprobs`、`suffix`、`n`、`best_of`、`logit_bias`
   等无法由当前 canonical 回复无损表达。现在 Nous owner 在 request-plan 构建前拒绝
   非字符串 prompt、非默认候选数以及上述实际启用的专属控制，返回
   `nous_legacy_completions_unsupported`，不静默丢字段或发送可能收费的请求。
   这没有修改通用 OpenAI owner，也没有宣称实现了 Nous 原生 `/completions`。
2. **复用真实公共链路。** 原 Nous LocalRuntime、管理 API 创建的预付次数 Key 和
   loopback HTTP peer 增加 Subscription / Welcome 参数；typed context 与 provider base URL
   一起绑定目标。没有增加生产 origin override、共享 Hermes 账号或放宽鉴权。
   测试入口对 Embeddings / legacy Completions 使用其真实 normalizer。
3. **独立实现线登记。** 新增 `manifests/lines/nous/portal-oauth.json`，绑定现有
   `line-nous-portal-oauth`、`nous_portal_oauth`、`openai_compatible` 及产品 owner。
   无凭据示例只有两个固定站点的空账号池，不猜测模型、价格或账号能力。
4. **补齐已存在的名单漂移。** 另外四个已有 manifest：Cloudflare Workers AI、DIM、
   Perplexity Web、River，均已存在 Cargo feature、preset 和 profile，却遗漏于旧 44 项
   canonical inventory。核对后连同 Nous 登记；不改它们的推理代码。
   重新生成 inventory 与 reference report / guide，并让测试校验完整 ID、四字段引用、
   精确唯一集合及真实本地/历史引用分类，保留 missing canonical 和重复/秘密泄露拒绝。
5. **修复两项运行中发现的问题。** 首次 Rust 结果为 55 passed / 2 failed，不是夹具字段错误：
   通用 usage parser 要求 `completion_tokens`，因此合法 input-only Embeddings 用量漏入审计；
   现在 finalization 先保留既有可识别 usage，仅此前未识别的 Embeddings 调用独立 parser，保留实际 prompt/total，
   仅这个 endpoint 的 completion 为 0，缺失或非法用量仍为 unknown，不放宽 Chat parser。
   legacy SSE 低层转换器原本会用上游 model 覆盖传入 fallback；增加显式固定 reply-model
   入口，仅 Nous Chat→legacy 分支使用。原低层入口和 native legacy passthrough 不变，
   前置原始 model declaration 观察不受影响，不修改旧保真测试来迁就新行为。

## 本地测试覆盖的合同

- 两个固定目标：三公共文本入口的 JSON/SSE、两回合函数工具、模型及账号选择、审计和扣次。
- 两个目标的 `/v1/embeddings`：字符串、字符串数组、token 数组、token 批次；
  float/base64、dimensions 与 user 参数原样到达上游；`data`、索引、编码、usage 和
  上游声明 model 通过既有 JSON passthrough 返回。这里不把原生 passthrough 强行改成 Chat 回复。
- 两个目标的单字符串 `/v1/completions`：JSON/SSE 返回 `text_completion`，保持文本、
  公共模型名、stop / `[DONE]`、usage，以及已有的七项 common generation controls。
- live SSE 的断开、源 body 回收、取消审计、退款、账号健康，扩展到 Welcome 与 Completions。
- 未知上游拒绝扩展到两个目标的五个入口，不重放、不扣次；不将这类夹具冒称反射 JWT 脱敏验证。
- 不可表达的 legacy 控制有 request-plan 表驱动拒绝测试；批量 prompt 另经公共 API
  验证零 HTTP 发送与不扣次。

## 验证回执

本轮日志目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-nous-inference-20261109`。

- Rust：首轮 55 passed / 2 failed；最终 r3
  `cargo test --locked --lib nous_ -- --test-threads=1` 为 **57 passed / 0 failed**。
  两项原失败断言不变；本次 build 有 75 条 warnings，不宣称零警告。
  同一最终源码另跑 OpenAI 协议 66 passed、结算/用量 16 passed、原始 model declaration
  观察 5 passed，保护原有上游模型保真、typed errors、取消释放和旧 usage 形状。
- Python：Nous manifest 3 项、inventory 11 项、reference 14 项、line list 2 项已通过；
  manifest validator 单元测试 5 项与全 49 manifest 校验通过。
- inventory validator / reference validator 通过：49 条 metadata-only 实现线，196 条引用、
  176 个唯一路径，其中 21 条本地引用、175 条历史引用、0 条未分类引用。
  metadata-only 不是 49 家真实接入成功，也不是新增 Cargo 编译证明。
- `test:effective-lines` 31 项通过；最终 12 份 Rust 格式检查、ratchet 通过。
  ratchet：5245 files、14 immutable runtime artifacts、governed source >700 为 0。
  未调整 baseline 或例外。`scope-r4.json` 与 `final-validation.json` 记录最终范围 hash、
  UTF-8 无 BOM、无源码漂移及 scoped diff-check；不冒称整个继承 dirty tree 的逐文件审计。
- 当前改动的手写文件最大 493 effective lines；生产 `runtime.rs` 从 132 到 156。
  本次没有新增异步任务、句柄、队列、重试或存储格式；拒绝消息只含静态分类、不反射输入秘密。

保留中间失败：默认 Python 没有 `jsonschema`，临时 runner 只设置 PATH 仍选错解释器；
改为显式使用已存在且符合 `tests/python/requirements.txt` 的 `jsonschema==4.25.1` venv，
没有安装或升级依赖。inventory 第一次有效运行另抓到旧 `metadata_only == 43` 断言，
改为独立 canonical 集合数量减去该测试的一条显式证据后，完整 11 项通过。
复核另发现初版 Embeddings 分流会遗漏旧 input/output alias 与自动合计；因此主动中止
尚在编译的 r2（未执行测试），补为“旧识别优先、input-only 后补”并添加两项兼容回归。
没有终止另一个 Otter 仓库的构建，也没有修改原始两项 Nous 失败断言。

## 仍未完成

真实独立 client 使用资格、真实账号登录/订阅/续期/费用、浏览器视觉、Server/Redis、
原生包及跨应用产品验收仍开放；未重新尝试上一轮不可用的浏览器控制工具。
legacy 批量/token prompt、echo/logprobs/后缀/多候选/非空 logit_bias 是明确不支持，
不是本轮已实现的原生 Completions。媒体及其他未经合同和测试覆盖的能力也不宣称完成。
初始 device receipt 的有界进程内保存边界仍按上一轮合同保留。

完整剩余范围见 [magpie 看板](../providers/magpie-workboard-2026-11-08.md)。

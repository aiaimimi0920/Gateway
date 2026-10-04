# Issue #3 S3：共享发送预算源码检查点

## 状态和授权

基于 `main / 4cd118efe93ad5c58ff0fc24867baef3935486d7`，实施前及提交准备前均核对
远端 main SHA。本检查点的源码验证已完成，记录于提交前；此时尚未提交/推送或交付 S3 新包。提交后的
实际 SHA、云端 CI、不可变包和接续点追加到 [Issue #3](https://github.com/aiaimimi0920/Gateway/issues/3)。

按用户既有授权独立完成 S3，不接管 S06 内部 Rust/Gemini，不改变 Platform 计费，
不调用真实账号/付费模型，不放宽安全/行数门禁。三个原有 dirty cache/report 的
hash 3/3 保持；Neuro 根仓库和其它子项目修改未接管、未撤销、未纳入本项。

## 实现和兼容性

完整合同见 [request-budget.md](../request-budget.md)。主要闭环如下：

- Context 唯一 owner；普通请求 6 次 adapter execution / 300 秒；复用现有可信
  1–300 秒 policy，从原起点只收紧。跨候选、refresh/recovery 和新 retry invocation
  不重置；准入不再冒充 execution，begin 后才计 route count / attempted IDs。
- send-stage deadline 覆盖排队、准入、keepalive readiness、恢复、S1 sleep、preflight
  和 bridge 累积；Drop 持久取消，迟到成功被丢弃。大 dispatch future 装箱，不靠
  扩大线程栈绕过 Windows 默认测试栈上的实际溢出。
- JSON/Binary/SSE 成功都移交 owner；SSE 返回后原流的 deadline/error/Drop 不换
  供应商、不重复文本/工具 delta。预算原因先标记再释放源；本地 timeout/cancel
  仍 failure-finalize，但不惩罚供应商健康或 AIMD。
- 工具定义/choice/历史调用或 Tool-role，以及媒体/ResearchCreate 保守最多一次
  adapter execution；不声称 adapter 内所有物理 HTTP/内部子操作 exactly-once。
- 合并队列和发送前复验精确 DB provider 白名单；未经可信来源明确点名的异账户
  不自动 fallback。YAML 命中 enabled rule、local key providerIds 和 validated access
  projection 从各自同一 owner 传递 provenance，保留 standalone 显式多账户配置。
  自动 supported-model/all-provider discovery 不给自己授权；已有 group、key、policy、
  协议和余额限制仍执行，不调整计费/预扣 owner。
- budget 终止保留最后真实 error 的 message/kind/status/code/provider，只改 retryable
  和固定 Abort 分类；HTTP 用两个固定响应头暴露 budget_exhausted / stop reason，
  不导出任意 hint。admission/keepalive/quota/授权错误不污染最后模型错误。

重要兼容性变化：未配置精确可信授权的自动跨账户回退被收紧，也包含免费或中转
账户。没有猜测品牌/URL/profile 的付费性质；首次选择沿用既有路由权限，不声称
存在通用 is_paid 分类或金额硬预算。跨账户未授权返回独立 code，原真实错误仍在
owner；全 policy 拒绝且零发送返回明确 403。详情和配置来源见合同。

## 实际验证

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-issue3-s3-20261004/`。

| 检查 | 实际结果 |
| --- | --- |
| 旧实现上的行为红灯 | 多候选可发第七次，新增回归按预期失败，不是环境失败 |
| S3 聚焦最终 r4 | 40 passed：24 runtime / 12 budget owner / 4 explicit route provenance |
| 真实 loopback 发送 | 多候选总 6 次、第三账户 0；in-flight/queue/S1 sleep deadline；media/tools 最多 1；未授权官方标签候选 0；SSE 提交前 fallback 与提交后 timeout/drop/transport error 不 replay |
| 真正 rate-limit rejection | SQLite-backed memory limits：拒绝后 count 0；已发 1 次后的 retry admission 拒绝不新增 send/observation，不覆盖真实错误 |
| 邻近 integration | 47 passed；2 个需要独立 Redis fixture 的 opt-in 测试 ignored，未冒充执行 |
| 聚焦/邻近 unit | 137 passed：retry、send helpers、routing config、local access keys；不是全部 3,404 unit 或全业务 suite |
| locked/offline all-target check | exit 0，312.52 秒；`all-target-check-r1.json` |
| formatter / diff / 编码 | 最终 fmt --check / diff --check 通过；本项 37 个新增/改文本 UTF-8 无 BOM |
| checker 自测 / ratchet | 31 passed / exit 0；34 个本项源码/测试最大 448 有效行；没有例外或 baseline 重生成 |
| 安全发布契约 | 23 passed；不是 findings-free 在线扫描或公开发布批准 |
| 原有 dirty 文件 | hash 3/3 保持，未 stage/cache 覆写 |

失败尝试保留，不用最后绿灯抹掉真实原因：first-green 在 default Windows test stack
上发生 `0xc00000fd / STATUS_STACK_OVERFLOW`，装箱 dispatch 修复；r2 的 3 个 SSE
fixture 因 fake session 触发 Redis finalizer，已去掉不必要身份，provider gate 对无需
provider-rate-limit 的策略按既有 rule builder 的早退条件处理。r3 的 account-group
fixture 拼错 `::default` ID，被 strict validation 拒绝；修正后 r4 全部 40 项通过。

## 独立只读复核与逐文件安全审查

### 完整 HTTP 链的 CI 回归修复

首次源码提交 `be721c706e54eeabc3fac31940fe657cc691dbd8` 的 fresh 官方本地构建
exit 0 / 1785.38 秒，但未交付：Linux CI job `111416379819` 在
`local_storage_pipeline_contract` 的完整 HTTP/SQLite 流发生 stack overflow。
同提交 Windows 默认测试栈也复现 `STATUS_STACK_OVERFLOW`，说明直接 send-stage
合同通过不等于完整 HTTP 链通过。失败日志及后续修复回执均保留在上述证据目录。

统一 `run_pipeline` 入口改为普通函数，立即返回装箱的 private async 流水线；
所有既有 handler 仍在原任务 await，不 spawn，不改变阶段顺序、鉴权、计费或
Drop 所有权。新增 caller-frame 回归在旧入口测得 28,560 字节并失败，修复后
只持有一个指针大小的 future。该尺寸断言不证明 inner poll 最大栈，完整 HTTP
fixture 是独立必要验证。只读复核核对了 18 个既有调用点，未发现兼容/生命周期
阻断项；它没有代替运行测试。

`http-stack-green-r2` 实际 44 passed：原 40 项、新 caller-frame 回归和 3 项完整
HTTP/SQLite 合同。第一次 green 尝试的 HTTP 3/3 通过，但 rate-limit fixture 清理
遇到 Windows error 32；现在只在本 fixture 自建目录对 error 32/33 有界重试，
最多 39 次 25 ms 等待，非 Windows 或其它错误立即失败，不掩盖资源泄漏。
修复后 locked/offline all-target check exit 0 / 226.31 秒、fmt --check、diff --check
和 ratchet 通过；三个改动 Rust 文件有效行数分别为 365、57、324，均无例外。
修复提交后的 CI/新包结果另记最新 Issue 回执；
旧 `be721c7` 构建不会冒充修复后的包，先前 137 unit 回执不冒充新提交全套测试。

两路独立只读复核先发现错误 code 被改写、非发送错误归属污染和 SSE budget stop
仍处罚 provider。均已修复并复核；新增 YAML/local-key/access-projection provenance
再单独核验，未发现候选自授权或覆盖适用限制的源码阻断项。复核不运行 Cargo，
不把其结论冒充测试通过、业务验收或正式发布批准。

- budget/stream：单一短锁、一个最后错误、无无界历史/后台 timer task；计数原子，
  stop 原因不可被 cancellation 覆盖，释放源后 callback。输入无预算重置入口。
- retry/gate/recovery：admission、begin、observation 的顺序独立；所有 direct send
  接缝共享 gate，refresh 前检查剩余额度；取消指标不写入最后真实错误。
- route/key/projection：只传精确账户 ID，不传 token；同 snapshot/transaction 来源，
  不复制第二套 alias/glob、不用自动候选作为授权。projected row 索引未重排。
- HTTP error：仅四个完整固定 Abort reason 可产生预算头，不输出自由文本；原 code
  与 body 结构保留，预算终止无 Retry-After；真实同名 upstream code 不漏记健康失败。
- fixture：仅 127.0.0.1/合成凭证/临时 SQLite；每 server 最多八条请求，watch release
  后 shutdown + 有界 join。SQLite 显式 close 后清理自己在 linshi 创建的目录。

## 不冒充完成的范围

没有主动 timeout auth/filter/route/finalizer，也没有接管 S06/浏览器/adapter 内每条
物理 HTTP。真实供应商 recovery、生产负载改善、PostgreSQL 配额/退款整链、部署
代理/CORS 对新增响应头的可见性、全 feature matrix 和业务验收未由这些测试证明。

S2 包不包含 S3。S3 必须正常独立提交/推送 main，核远端 SHA 后 fresh 官方构建
新的不可变版本；7 个 package gates、原生窗口/SQLite/ZIP 和复制后 integrity 分别
验收。严格公开发布仍按同提交云端 findings 和工作流核验，不复用旧扫描放行，
不关闭整个 Issue 来掩盖这些交付/发布边界。

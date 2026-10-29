# 上游 IP 封禁冷却与授权内回退修复

日期：2026-10-25。范围为会话 `01a169dd-7e45-7943-8611-33295badf24b`
最后提出的 GPT 503 排查和明确 IP 封禁后的智能路由，不包含其他进行中的模板、UI 或网络恢复开发。

## 已观察的两类错误

1. `gpt-6.1-sol` 返回 `503 / authorized_candidates_unavailable`，消息为
   `Authorized upstream accounts are disabled or temporarily unavailable`。
   原会话审计为 `candidateCount=0`、`routeAttemptCount=0`、
   `attemptedProviderIds=[]`，说明请求在本地路由候选过滤后已无可发送目标，
   不是上游接收到该次请求后返回的 503。
2. 上游 HTTP 403 明确包含 `该ip已被封禁，原因：bulk probe guard: ip ...`。
   这表示该服务商拒绝当前出口 IP，而不是某个模型的普通故障或某一凭证失效。

GPT 503 的历史证据只能确定发生阶段，不能区分当时的手动禁用、模型规则、
discovery、circuit 或 site-health 硬过滤。该访问密钥仅授权 `default` 组，
不能为了绕过 503 自动跨组、启用账号或清除故障限制。本轮只读检查的运行实例和
配置已较原请求时变化，当前持久化健康记录不能证明历史状态或当前内存 circuit。
因此，不将历史精确根因或真实账号请求恢复记为已证。

## 实现与修复边界

- 错误分类严格识别 HTTP 403、固定中文消息结构、合法 IP、正数模型数、时间窗和
  offense，以及数字 user ID；拒绝多余尾部字段。仅自报
  `provider_ip_temporarily_blocked` 的上游错误码不能获得新增重放资格。
- JSON 和纯文本形式均保留识别所需的脱敏错误消息。匹配后进入
  `ProviderUnavailable`，不标记凭证永久失效。
- 为服务商建立独立的 `provider:availability` 熔断键，第一次明确 IP 封禁直接
  进入第三级冷却 32 分钟；到期单请求试放仍失败升至第四级 60 分钟。
  若可信 `Retry-After` 更长则保留更长限制。两次完整成功至少间隔 30 秒才关闭熔断。
- 限制覆盖该服务商的其他模型、账号和本地组；其他服务商不受此条限制影响。
  普通模型、transport、channel 和余额故障不能累计到服务商级键；余额限制仍归
  具体账号的 `balance:` 键。旧在途成功不能清除较新的封禁。
- 明确 IP 拒绝已证明上游可达，不触发阻塞所有路由的全局出口检查；已有独立网络
  故障检查保留，网络恢复也不能清除真实 IP 封禁。
- 响应移交前，可在同一请求预算内继续尝试另一个明确授权的服务商；仍执行
  access key、账号组、模型规则、provider/model 白名单和发送前资格检查。
  没有合规备用仍失败，不捞回手动禁用或越权目标。已移交 SSE 不重放。
- 冷却仅保存在当前进程的有界状态中，不修改凭据、数据库版本，不增加后台计时任务。
  冷却结束只代表允许试放，不代表上游已经解除 IP 封禁。

主要代码入口：`src/error/admission_rejection.rs`、`src/error/classification.rs`、
`src/provider_failure.rs`、`src/routing/admission.rs`、
`src/routing/model_circuit.rs` 和 `src/routing/model_circuit/failure.rs`。

## 回归证据

证据根目录：
`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-provider-ip-block-20261025`。
测试使用合成凭据、临时 SQLite 和本地 loopback HTTP，不调用真实上游或付费模型。

- 红测复现普通模型故障污染服务商级 circuit，以及仅伪造错误码获得重放资格、
  纯文本错误丢失分类证据的问题，见 `resume-red.log` 和 `resume-error-red.log`。
- Circuit 定向验证通过 26 项，见 `resume-circuit-green.log`，覆盖故障作用域、
  32/60 分钟试放、迟到完成、独立网络恢复、余额隔离和既有 circuit 合同。
- 相关集成验证通过 81 项，见 `resume-routing-contracts.log`：账号组合同 6、
  pipeline-send runtime 51、共享预算 12、准入拒绝预算 8、fallback 授权 4。
  本轮新增 HTTP 和 SSE 启动前 IP 封禁用例均通过。
- 最终 error 相关、provider-failure 和 circuit 单元测试共 77/77 通过，见
  `resume-lib-final.log`；其中包含前述 circuit 26 项，不重复累计。
  上游 401 / 本地 unauthorized 合同的修正也已重新编译并通过。
- checker 自测 31/31 通过，ratchet 通过且无 governed source 超过 700 有效行，
  最终回执见 `resume-checker-tests-final.log` 和 `resume-ratchet-final.log`。
- 官方格式最终检查通过，见 `resume-format-final.log`。
- `git diff --check` 通过；相关源码和文档的 UTF-8 无 BOM 检查通过。
  本轮涉及源码均不超过 500 有效行，不修改 baseline 或 exception。

Rust 验证使用 locked dependencies、`devtest` profile、`runtime-defaults`、4 个
Cargo job，串行运行测试套件；`GATEWAY_DEVTEST_ISOLATED=1`、
`GATEWAY_PREBUILT_WEB_UI=0`，TEMP/TMP 指向上述证据根目录。实际入口为：

```powershell
cargo test --locked --profile devtest --no-default-features --features runtime-defaults --jobs 4 --test request_budget_rejected_channel --test request_budget_contract --test request_fallback_authorization_contract --test access_key_groups_contract --test pipeline_send_runtime_contract -- --test-threads=1
cargo test --locked --profile devtest --no-default-features --features runtime-defaults --jobs 4 --lib -- error:: provider_failure:: routing::model_circuit:: --test-threads=1
```

两次最终 Cargo 验证均为 exit 0。编译仍有本次范围外的非阻断 warning，未修改
无关代码或升级依赖；没有执行全仓回归、正式发布审计或真实付费上游验收。

真实 HTTP/SSE fixture 位于 `tests/access_key_groups_contract/ip_block.rs`：
首请求带工具声明，初始预算只有一次发送资格；明确准入拒绝后只能发送
`fixture-blocked`、`fixture-backup`，不得发送同服务商 sibling 或组外账号。
随后另一模型也必须跳过被封服务商；仅授权该服务商的 key 返回
`503 / authorized_candidates_unavailable`，无模型权限仍返回 403。
其中 SSE 用例是上游在 SSE 开始前返回 HTTP 403，不证明输出部分内容后可以重放。

验证中发现两处旧测试仍将上游 401 期待为 `Abort`，与当前已有上游鉴权拒绝
允许切换的合同冲突。仅更新测试预期，并增加本地 unauthorized 仍为 `Abort` 的断言，
不扩大产品的鉴权或重放权限。

## 交付状态

本项是源码修复与开发侧定向验证，不是线上修复验收。没有生成新的分发 EXE 或
手测包，没有替换、重启或修改用户 4200 实例，没有修改真实凭据或数据库，
没有提交或推送。当前运行的旧手测包不会自动包含本轮源码修改。

# 2026-10-20 Codex 网络保真修复

## 目标与范围

消除 Gateway 在转发原生 Codex 请求时增加的 HTTP 内容和传输指纹差异。
参考本机实际 Codex CLI 0.155.1 / Windows，使用相同回环接收端、合成密钥及真实客户端流量。
入口为 `/v1/responses`，`openai_compatible`、Direct HTTP、Codex/非 Claude 自动模式，
不含协议桥接、工具转换、产品专用通道或上游 WebSocket。具体边界见 [网络保真说明](../codex-wire-fidelity.md)。

## 实际改动

- 修复空 `tools: []` 被误当作真实工具注入的条件；保留 developer、message ID/type、
  metadata、cache key 和未来未知字段。真正工具注入仍沿原有转换路径。
- 新增 bounded 原始请求证据 owner。最终 RequestPlan JSON 未发生业务变更时复用原始
  body 字节；否则仍执行模型映射、缺省设置及工具/协议转换，不能用保真绕过策略。
- 保留原始 query 的重复键、顺序及转义；为 raw 转发增加重复 JSON 对象键拒绝规则。
- 固定白名单保留原生软件身份、会话/turn/beta 信息、合法 trace 和头顺序；不转发入口凭证、
  Cookie、任意设备头或 Gateway 内部控制头，服务商认证始终由选定账号配置建立。
- 不再注入额外 Version 和浏览器默认头；仅在缺失时补 Accept；不向上游外发 Gateway
  自动生成的 request ID，本地日志与响应追踪不受影响。
- 增加独立 `reqwest 0.12.28 + native-tls` sender，streaming/buffered 共用，保留预算、重试、
  熔断、并发许可、body 上限、SSE completion、背压和取消清理；错误先去除 URL。
- 将新增 native/legacy dispatch 和 buffered JSON attempt 在同一任务内装箱，避免大型
  future 进入每层 retry/gate 的栈帧；不新增任务、发送次数或改变取消/lease 的 owner。
- Windows 原生通道局部采用参考客户端对应的 TLS1.2 ceiling，其他通道不受影响，
  不降级整个项目的 TLS 依赖、不关闭证书或域名校验。

生产职责分别位于 `src/http/native_json*.rs`、`src/protocol/codex_wire.rs`、
`src/http_client/native_codex.rs`、`src/upstream/client/native_codex*.rs`、
`src/upstream/native_response.rs` 及原有 pipeline/profile 的最小接线处。
Docker 构建/开发阶段显式声明 OpenSSL headers，运行阶段显式声明 `libssl3`，
避免 native-tls 依赖由其他包偶然满足；本轮没有构建或部署 Docker 镜像。

## 最终证据

最终源码对应的 debug binary 已重新构建并完成重抓包。Gateway SHA256：
`92bef6e9dc70cfe2c5f0ad14fdfd57331ee0873371716289214674ef195dadc5`。
参考 Codex binary SHA256：
`eba0f32c976667cb9298efafd98513e823eeda7b576a03ec658bb8be8d336316`。

### HTTP

先捕获真实 CLI 直发，再将同一原始请求送入 Gateway，比较接收端完整 HTTP bytes，
避免独立会话本身的随机 ID/context 差异。没有排序或剥离 HTTP 字段再比较。

| 请求 | body bytes | HTTP bytes | 完整 HTTP / body / ordered headers |
| --- | ---: | ---: | --- |
| exec 主请求 | 55302 | 56547 | 全部一致 |
| exec resume | 55585 | 56830 | 全部一致 |
| 原始重复 query | 55302 | 56584 | 全部一致 |
| TUI 主请求 | 56704 | 57953 | 全部一致 |
| TUI title，空 tools | 20642 | 21893 | 全部一致 |

每组完整请求的两个 SHA256 相同，见 `http-verification-verified.json`；
query 用例只在参考请求行中附加同样的 query，不做 URL 归一化。
真实 CLI 直发/经 Gateway 的 exec 与 resume 均 exit 0；TUI 直发/经 Gateway 均捕获主请求和 title 请求。
TUI harness 在捕获完成后关闭自己的 ConPTY，不将人为结束会话误报成自然退出验收。

### TLS 与安全边界

`tls-verified/verification.json`：3 次原生直发 + 3 次隔离 Gateway，均捕获首次 ClientHello。
每条 TLS record 为 165 bytes；仅清零 `ClientHello.random` 的 32 bytes 后，整个 record 字节相同。
cipher、extension 顺序和内容、SNI、ALPN、session ID 均相同；JA3：
`6a5d235ee78c6aede6a61448b4e9ff1e`。不是仅 hash 相同的证明。

`boundaries-verified/verification.json`：7 组真实二进制黑盒用例全部通过，涵盖显式 Accept、
合法/非法 request ID、过长 session/turn metadata、重复 JSON 键、过长 query；
Cookie 和无关设备头不外发。最后两项返回 400，接收端请求数量为 0。

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-codex-fidelity-fix-20261020`。
`src`、`tests` 的 Rust 文件及根 Cargo/build.rs 清单为 `final-v2-source-manifest.json`，
1840 个文件；构建期间及最终复验后核对这部分源码未变化，聚合 SHA256：
`d767e196a4f034ad3e9f92a2e4394a066e646e1a22aaf6caa8009751ea6d7c78`。

## 门禁

- Python：隔离 venv 安装仓库固定测试依赖后，362 tests，4 skipped，exit 0；
  `python-suite-delivery-final.log`。未修改全局 Python 环境或放松测试。
- Node scripts：2124 tests，2122 passed，2 skipped；`node-suite.log`。
- Desktop typecheck：exit 0；`desktop-typecheck.log`。
- line manifests：44 checked，0 errors；使用包含固定 jsonschema 依赖的隔离 venv。
- effective-line checker tests：31 passed；ratchet exit 0；
  3070 files，14 exact-hash immutable runtime artifacts，governed source >700 为 0。
- Rust 默认特性全量回归：106 个 suite，4127 passed、0 failed、104 ignored，exit 0；
  `full-cargo-verified.log`。命令为 `cargo test --locked --offline --no-fail-fast -- --test-threads=1`，
  明确清除 `RUST_MIN_STACK`，使用默认测试线程栈；ignored 不计入实际通过。
- 默认栈专项回归：账号发现完整路由、原生保真、OpenAI 保真和 profile/runtime 共 23 tests
  通过；`stack-and-fidelity.log`。没有设置 `RUST_MIN_STACK`。
- 两份 Dockerfile 的 `docker buildx build --check` 均通过、无 warning；使用空的临时 context，
  只做 Dockerfile 检查，没有将工作目录内的运行数据或凭证送入 context，也没有生成镜像。
- `cargo fmt --all -- --check`、`git diff --check` 最终通过。

汇总回执为 `verification-final.json`，将上述结果、源码清单和实际测试 binary SHA256 绑定。
最终核对源码清单未变化，binary hash 与 HTTP/TLS/边界测试记录一致；只读进程检查显示本轮
`target/debug/gateway.exe` 和隔离 Codex 实验进程残留为 0。

代码行 before/after 存于 `effective-lines-before.json` 和 `effective-lines-after.json`。
关键原有文件：Responses route 43→53、pipeline context 377→379、upstream client 412→415、
headers 426→433、OpenAI plan 480→484、profile headers 119→145。
新增生产模块均少于 250 有效行；未修改 baseline、policy 或 exception。

## 中间失败与修正记录

- 第一轮 HTTP harness 在代理持久连接时，只转换第一条 absolute-form 请求行；第二条被
  错误记作 Gateway 差异。以逐 message framing、每条请求重新校验回环目标的代理重抓，
  没有通过排序头、删字段或归一化请求行来伪装 HTTP 字节相同。
- 新版 native-tls 默认开放 TLS1.3，初次 ClientHello 与参考客户端不同；核对 Codex 锁文件
  和旧 native-tls Schannel 实现后，只在本次 Windows Codex 通道复现参考版本的 ceiling。
- 原生 sender 单元夹具一度缺失 `instructions`，正常缺省策略会改变计划；修正夹具而不是
  绕过缺省策略。显式 Accept 覆盖问题也已修复并新增合同测试。
- 第一次全量 Rust 默认并发运行：3485 passed、5 failed、36 ignored；5 个失败都出现在
  ChatGPT Web stream 测试遇到 `chatgpt_web_solver_busy`，本次原生通道测试没有失败。
  后续串行 lib 为 3490 passed、0 failed、36 ignored，没有修改该产品实现。
- 串行全量继续到 `account_discovery_contract` 时，默认线程栈出现 `STATUS_STACK_OVERFLOW`。
  参考仓库既有的 future 装箱边界修复新增 dispatch/JSON attempt；原测试不改断言，以默认栈
  重跑已通过，最终 106 suite 全量也通过；没有借用栈修复前 lib 的成功代替最终门禁。
- 一次 Python 重跑误经 bare `python` 选中了缺少 jsonschema 的全局解释器；改为显式调用
  已安装固定依赖的隔离 venv Python，完整重跑通过，失败日志保留。

## 不可外推的结论

完整 HTTP 字节一致、ClientHello 去协议随机值后一致，不等于完整 HTTPS/TCP 会话逐字节相同，
更不等于真实服务商已确认无法识别。TLS 接收端主动断开；本轮没有真实上游调用或风控回执。
普通非 Codex 输入缺少的 prompts/tools/session metadata 不会被伪造。
自定义 CA/Rustls、不同 OS/CLI 版本、辅助 API、模型/工具转换等分支不在当前抓包证明内。

保留原有 dirty edits、真实凭证、运行数据和实例；未提交、推送、部署或重启真实服务。

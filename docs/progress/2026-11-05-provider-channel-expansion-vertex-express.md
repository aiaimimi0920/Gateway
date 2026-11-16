# Vertex Express 独立身份、静态材料及最终发送子批

日期：2026-11-05。仓库：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`。
状态：本后端子批已完成定向离线验证；正常前端入口及后续链路继续，不是 R11/W07、P50 或整计划完成。
分支 `main`，HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`；保留原有约 946 项 dirty entries，不提交或推送。
合同与官方来源见 [Vertex 分渠道实施合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/plan/provider-vertex-channels-contract.md)。

## 1. 实际改动

1. 新增后端 preset `vertex-express-api`、profile `vertex_express_api` 与内部产品 marker `vertex_express_api_key`，
   使用已有 `gemini_api_compatible` 和 `line-google-agent-platform-official`，不是新转发服务或 OAuth 别名。
2. 固定 `https://aiplatform.googleapis.com/v1/publishers/google`，复用 Gemini 的默认模型路径与请求体，
   不修改 S06 拥有的共享 Gemini 编码器、parser 或 translator 内部。
3. 在原声明、父级及每个最终账号合并、三适配器准备、最终 plan/header 处保持产品和 Key 受众绑定。
   空池/清空可保存，发送必须非空；单段精确模型拒绝 URL 编码、斜杠、query、fragment 或 wildcard。
   OAuth/refresh/context、session、keepalive、discovery/借用目录、自定义路径/头、额外 body 及外族指纹不能混入该静态渠道。
4. 将 Express 纳入现有完整 Keep destination/consumer 比较，复用原 SecretPatch：允许 label/model 编辑保留 Key，
   无 secret access 时不允许改变产品、有效目标、profile/flow/refresh 或新增继承消费者。
5. 最终 sender 先验证，再重建 `x-goog-api-key`/Content-Type/Accept，清除 caller 的 Host、Cookie、OAuth Bearer、
   Key 覆盖与 attribution headers；不将 Key 放 query，不跟随重定向。
6. 普通 buffered 和先聚合后桥接的 SSE 继续走共享 HTTP sender/decoder；媒体 JSON/binary 及无效模型在网络前拒绝。
   本批不是实时 token-by-token 或全部 Gemini/Vertex 模态保真的证明。
7. 默认栈复核暴露 direct `execute` / `execute_stream` 包装方法内联携带大型多服务商 future。
   本批在这两个生产转交边界按已有 public pipeline 的方式同任务 `Box::pin`，不增加 task、重试、线程栈或后台生命周期。

直接 owner：

- [protocol/vertex_express.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/vertex_express.rs)
- [routing/config/vertex_express.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/routing/config/vertex_express.rs)
- [upstream/vertex_express.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/vertex_express.rs)
- [Keep binding](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/console/secrets/keep_bindings.rs)
- [buffered direct wrapper](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/client/request_plan_part_1.rs)
- [streaming direct wrapper/final sender](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/client/streaming.rs)

## 2. 验证证据与失败记录

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-vertex-express-20261105`。
后端命令固定 `--locked --jobs 4 --no-default-features --features runtime-defaults`，不启 `management-ui` 发布/清理 hook。
最终测试的 TEMP/TMP 在本批 `tmp`，显式清除 `RUST_MIN_STACK`；不通过扩大线程栈冒称默认栈成功。

### 2.1 首轮及隔离结果

- `red-preset.log`：生产未实现时精确运行 1 项，因独立 preset 不存在而真实失败，不是匹配 0 项。
- `green-v1.log`：测试使用了错误的 `TokenUsage.input_tokens/output_tokens` 字段，编译失败；按真实类型改为 prompt/completion 字段。
- `green-v2.log`：暴露错误的 YAML `auth_header_name` 负测及两项默认栈溢出。
  实际 `ProviderConfigYaml` 没有该覆盖字段；夹具改为确认未知 YAML 不覆盖 preset，并另在真实 raw payload 层证明非法 header 被拒绝。
- `stack-isolation.log`：16 MiB 测试栈下另暴露代理 absolute-form URI 与 path-only 断言不一致。
  非转发服务器继续校验实际 authority，仅将记录字段改成真实 `path_and_query`，没有放宽产品 URL 或发送验证。
- `green-v3.log`：代码审查发现 feature-off 夹具缺少必要 `api_key` 字段，在新编译尚未完成时主动取消；不计通过。
- `green-v4.log`：13 项新增及随后 158 项库回归通过，但当时用了 16 MiB 测试栈；
  `pre-stack-affected-secrets.log` 记录当时的四个已有秘密合同目标，均非最终默认栈回执。
- `default-stack-isolation.log`：在清除 `RUST_MIN_STACK`、仅运行单项时再次真实溢出，不能当并行夹具竞争或已证明纯环境问题。
  随后才修改生产 direct wrapper 的 future 所有权。此前关闭配置验证被精确中断，保留 `pre-stack-feature-off.log`，不计通过。

### 2.2 最终验证

最终运行显式清除 `RUST_MIN_STACK`，在 Windows 默认测试线程栈上通过；不沿用增栈轮次的成绩。

| 验证 | 最终结果 | 证据 |
| --- | --- | --- |
| Express 主配置定向测试 | 13 passed / 0 failed / 0 ignored | `green-v5-default-stack.log` |
| 扩大库回归，包含上述 13 项 | 158 passed / 0 failed / 0 ignored | `affected-lib.log` |
| Bedrock、Qwen、Kiro Key、Console 文档秘密合同 | 92 passed / 0 failed / 1 ignored | `affected-secrets.log` |
| Kiro Key / Grok OAuth 相邻实际 sender | 6 passed / 0 failed / 0 ignored | `adjacent-senders.log` |
| 主配置按 target/test name 去重 | **256 passed / 0 failed / 1 ignored** | `test-summary.json` |
| 展开 runtime-defaults，仅关闭 Google Agent Platform 执行线 | 6 passed / 0 failed / 0 ignored | `feature-off.log` |
| 有效行 checker | 31 passed / 0 failed | `checker.log` |
| 25 个本批 Rust 文件的官方 formatter、有效行 ratchet、Git diff | 通过 | `rustfmt-final.log`、`ratchet-final.log`、`diff-check-final.log` |

三个 Cargo 阶段退出码均为 0，见 `affected-exits.json`。feature-off 与 checker 单列，不累加为主配置测试数。
唯一 ignored 为 `session_material::session_material_frontend_to_secret_resolver_roundtrip`，需要显式导出的合成前端夹具，未计入通过。
关闭配置仅证明这一个代表性组合，不是所有 feature 组合均通过；编译保留非零 warnings，不宣称 warning-free 或全仓绿。

`source-final-audit.json` 对本批选定的 **34 项源码/依赖输入**复核 SHA-256，均与最终测试前冻结一致；
这不是全仓源码、生产 EXE 或包来源凭据。25 个新增/修改 Rust 文件全部低于 500 effective lines，无例外：
protocol / config / header owner 分别 153 / 58 / 31 行，主测试 243 行；`client.rs` 473→477，
`keep_bindings.rs` 237→254，两个 direct wrapper 所在文件分别 163→164、369→382。
ratchet 扫描 4,369 文件，14 个 hash-bound immutable runtime artifacts 单独分类，governed source >700 为 0。
最终 scoped diff、四份相关文档的 UTF-8/no-BOM 和本地链接核验另存于本批证据目录。

## 3. 边界与接力点

- 新增的是后端 preset，不是已新增前端模板；本计划累计新增模板仍为 13，真实验收/蓝色标记均不增加。
- 没有请求或读取真实 Key/OAuth client/账号、历史凭据或 `.ng`，没有真实 Google 生成、刷新或费用验证。
- 未创建 Redis/Docker 容器、没有复用历史容器授权、没有清空现有数据；没有出 EXE、运行正式发布流程或替换在线服务。
- 本批仅普通配置编译、SecretPatch resolver 与最终 loopback sender；不是管理 HTTP/Grant/Origin/CAS、SQLite/Redis/Hosted
  全载入/持久化、公共 router→路由→finalize、取消/扣费或原生 EXE/重启的完整验收。
- 原始 Gemini decoder 复用不证明 Express 全部成功终态、工具/reasoning、多 choice、阻断、usage/audit 或外层 retry/fallback。
  loopback 单次 sender 不自重试，不等于已证明公共 pipeline 的不重放准入。
- 下一子批继续正常 Express 空池模板、Key 添加/Keep/清空与暂存受众、明确手工模型和精确账号测试，
  再做公共调用/取消/结算；不误用 AI Studio 目录或虚构默认模型。
- Full Vertex OAuth 的独立 client/scopes/redirect、project/location、typed context、Bearer 与持久刷新仍未实现。
  不能把本批 Express Key 成果、旧 Vertex REST 或 Antigravity token 当作 OAuth 完成。

其他原定渠道与集中录制/真实验收仍保留；总目标保持 active。

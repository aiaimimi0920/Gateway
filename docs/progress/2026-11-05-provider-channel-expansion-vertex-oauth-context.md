# 服务商扩展：Full Vertex typed 授权上下文与目标合同

日期：2026-11-05。对应总计划 R11/W07，接续 [独立 desktop OAuth wire](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-05-provider-channel-expansion-vertex-oauth-wire.md)。

## 范围与完成边界

本批仅在独立 `vertex_oauth` 协议 owner 内实现可运行、可序列化的授权上下文、固定 Google publisher 目标构造，以及合法 refresh rotation 比较。
合成 HTTP 测试连接 code exchange → context JSON 往返 → 专属 refresh；不是正常 Console 保存、数据库持久化、runtime 重建或 scheduler 验收。

**该类型尚未注册到 `ProviderCredentialYaml` 或运行 payload，不能通过普通管理接口保存或使用新 typed 渠道。**
因此不提前开放模板，不把原有 generic refresh 当作 Full Vertex 专属刷新，不修改旧 Gemini header、Full Vertex aliases 或 S06 共享 Gemini 实现。
累计新增模板仍14，不新增整卡 `offline_ready`，不标 R11/W07、P50 或总目标完成。

## 同日联网核实的目标合同

无真实 OAuth、token、生成、IAM 或额度请求，仅只读官方文档：

1. [Full Vertex generateContent](https://docs.cloud.google.com/vertex-ai/generative-ai/docs/reference/rest/v1/projects.locations.publishers.models/generateContent)：
   publisher resource 包含 `projects/{project}/locations/{location}/publishers/*/models/*`；另有 tuned endpoint 形状，不能混为同一已实现能力。
2. [Deployments and endpoints](https://docs.cloud.google.com/vertex-ai/generative-ai/docs/learn/locations)：
   global 的域名不带 location 前缀，resource location 为 `global`；regional 使用相应区域 endpoint。
   文档的模型/区域资格和数据驻留限制不能由语法校验代替。
   后续补查当前 [canonical locations](https://docs.cloud.google.com/gemini-enterprise-agent-platform/resources/locations)，明确 REST 的
   `LOCATION-aiplatform.googleapis.com`，以及 `us` / `eu` multi-region 分别使用 `aiplatform.us.rep.googleapis.com` / `aiplatform.eu.rep.googleapis.com`，路径仍为对应 location。
   以该页完整可见的 endpoint 小节为目标构造依据，不从旧 URL 的不同摘要推断 multi-region 也使用普通区域公式。
3. [Set the quota project](https://docs.cloud.google.com/docs/quotas/set-quota-project)：
   REST 可使用 `x-goog-user-project`，调用主体仍须具备 quota project 的 `serviceusage.services.use` 等相应权限。
   本批仅保存显式 quota-project 元数据，**尚未接业务 header**；不从 project、邮箱或模型名推断共享余额。

抓取原文保存在 `C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-vertex-oauth-context-20261105/public-sources.json`、`regional-search.json` 与 `canonical-locations.json`。
locations 页面抓取有长度截断；本批仅使用截断之前完整可见的 endpoint 小节，不宣称已核完全部模型/区域表，也不复制其中模型名作为可用能力。

## 实现

- `AuthorizationContext` 为 closed schema：只承接 desktop authorization-code grant、显式 client ID/可选 client secret、固定 requested `cloud-platform`、是否实际报告 scope、明确 target 和可选 refresh 绝对期限。
- 新授权构造必须来自具有 refresh token 的 wire material；access-only 响应不能伪装为新授权。context 不包含 access/refresh token，也不声称已验证 Google 用户身份、IAM 或模型权益。
- optional client secret 会随 context 序列化；这不是公开管理响应。后续注册 schema 时必须同批接原 nested SecretPatch、redaction/descriptor、Keep 和暂存受众，不可先暴露该字段。
- `VertexTarget` 显式绑定 project、location、可选 quota-project。固定 `v1`、`publishers/google`，分别构造 global、regional 与 `us`/`eu` multi-region base，不接受任意 base、publisher、URL 或 query。
- project/quota-project 做有界资源 segment 与 header 安全校验，保留 numeric project number、legacy domain-scoped ID 的表示；不是自创官方 project-ID 正则或存在性检查。
  location 做有界小写 DNS label 校验，`location + -aiplatform` 不超过63字节，不冻结官方区域列表，不承诺某语法合法区域可用。
- refresh 期限保存完整 RFC3339 精度。已过期证据仍可读取/保存，恢复 `RefreshGrant` 时才在网络前拒绝。缺失期限不能清掉已知期限，较长的新期限不能延长旧期限，首次收到期限允许补充。
- `permits_rotation` 要求 client/secret、grant、requested scope、project/location/quota-project 不变；只允许期限相等/缩短和固定 scope 的报告证据由未知变已知。
  以实际时间比较等价 offset/小数表示，不通过序列化拼写延长寿命；已报告 scope 不能降为未报告。
- context 的 Debug 固定脱敏；错误为现有固定 `OAuthError`，不包含 target、client secret、token 或上游 prose。

## 验证记录

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-vertex-oauth-context-20261105/`。

- 实际功能红测 `red-context`：29 passed / 8 expected failed / 0 ignored；37项中含24项原测试与13项本批初始测试。
  失败验证 scope、目标注入、regional origin 和 rotation 约束缺失；不是编译失败。
- 首次绿色 `green-context` 已通过41项；随后根据补查的 canonical multi-region 合同增加目标分支与专属测试，最终结果以下述重新编译记录为准，不以首次绿色覆盖后续改动。
- 最终绿色 `green-context-final`：**42 passed / 0 failed / 0 ignored**，其中18项本批新增；包含两项真实 synthetic HTTP 的 context/code/refresh 往返测试。
- 代表性 feature-off：关闭官方 Vertex、保留共享 Gemini Web core，**30 passed / 0 failed / 0 ignored**。
  其中29项与默认组合重复，另1项为已有的网络前 feature-off 拒绝测试；不是完整 feature matrix，不回写前批仅官方线的共享 Gemini Web 15项缺符号失败。
- 同最终默认栈 EXE 的相邻全名匹配测试：Accio40、Grok75、Kiro46，合计 **161 passed / 0 failed / 0 ignored**。
  其中独立协议 owner 为25/52/25，合计102；其余59项覆盖已有 Console/管理 HTTP/probe/sender。相邻 filter 比前批的仅协议 filter 更宽，不把新增覆盖误报为新实现。
- 最终按测试名去重为 **204个不同 Rust 测试通过**，其中18项本批新增；不把 feature-off 的29个重复项再累加。
- checker31项通过，最终 ratchet4429文件、14项immutable runtime、governed source >700为0；独立官方 Rustfmt check 通过。
- 最终2949项编译输入无漂移、相邻执行前后 EXE 哈希不变；11项本批文件严格 UTF-8 无 BOM、scoped whitespace 与来源审计通过，写前快照中无本批范围外输入变化。
  当前全工作区 `git diff --check` 也通过。证据见 `green-context-final.json`、`feature-off.json`、`adjacent.json`、`checks-final.json`、`audit.json`、`closeout.json`。

```powershell
cargo test --locked --no-default-features --features runtime-defaults --lib --jobs 4 vertex_oauth -- --test-threads=1
cargo test --locked --no-default-features --features line-gemini-web-reverse --lib --jobs 4 vertex_oauth -- --test-threads=1
```

首次绿色的源码后来发生了已明确记录的 multi-region 改动；最终42项是在重新编译后的源码上运行，不复用首次41项 EXE作为最终证明。

所有 Cargo 命令 `--locked`、`--jobs 4`、无 management-ui、默认线程栈；没有设置 `RUST_MIN_STACK` 或增栈绕过。
没有真实凭据、`.ng`、ADC、Antigravity client/token、Docker 容器、运行实例变更、数据库迁移、出包、提交或推送。

## 规模与安全复核

本批新生产文件为 `src/protocol/vertex_oauth/context.rs` 与 `target.rs`；测试按基本 schema、rotation、目标和合成 HTTP 链分开。
写前已测量原协议入口115有效行，新增模块注册后127；新源码/测试最大161有效行，不新增例外或修改 baseline。
没有新增依赖、I/O owner、队列、后台 task、重试或秘密存储。metadata 在 validate、派生目标与准备 refresh 边界做长度和语义校验；closed serde schema 拒绝未知/重复已知字段，但反序列化本身不代表已经通过 validate，未来外部文档入口仍须使用其已有 body 预算。
网络仍复用既有 bounded/cancellable wire owner。
fixture server 仅使用现有 synthetic loopback 设施，每个测试正常 stop/join；不以测试 endpoint 替换生产目标合同。

## 下一接力点

1. 将 context **作为 credential-only 字段**接 schema、原声明/最终 payload 校验、DB provider-context stripping；不能从父级继承/递归拼接 grant。
2. 同批接 client secret 的 nested SecretPatch/redaction/descriptor/Keep、整体 destination/consumer 和暂存受众，确保 access/RT 也绑定完整 target/client/scope。
3. 接现有 RefreshMaterial/Attempt、GrantMaterial、专属 Prepared、Sending → Result → Published、revision、projection/replay/cache 全链。
   `refresh_expires_at` 与 scope report 可合法变化，必须作为 mutable rotation context 贯穿；新增 None extension 保持旧 material hash。
4. 最终 Full Vertex 发送独立验证 URL/method/resource/region/model，并重建当前 `Authorization: Bearer ...` 及可选 quota header；旧 `auth_token` 遮蔽问题仍未修复。
5. 独立正常管理授权/listener/session/create/status/cancel/import、稳定 ID/普通保存/UI/重启恢复、公共请求响应/usage/结算/取消和最终模板继续；真实 client/账号/IAM/模型/额度留 P60/P70。

Express 多candidate、未支持 Part/Preview/模态、signed Preview 关联与实时参数流，以及其他原定渠道、P50/P60/P70均不从原范围移除。

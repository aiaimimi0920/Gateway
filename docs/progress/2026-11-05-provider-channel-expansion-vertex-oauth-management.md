# 服务商扩展：Full Vertex 正常授权管理与服务端保存

日期：2026-11-05。状态：本后端子批定向离线通过，不是整卡完成。对应总计划 R11/W07，接续 [credential-only/持久刷新子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-05-provider-channel-expansion-vertex-oauth-lifecycle.md)。

## 范围与未完成边界

本批实现独立 Full Vertex 的正常管理后端：实际 loopback listener、实例内 session、create/status/complete/cancel/import，以及原 SecretPatch、revision/CAS 与配置持久 owner 保存。
不借用 Antigravity client/token、ADC 或其他产品账号，不改共享 Gemini 编码器/decoder，不新增秘密存储或刷新 scheduler。

**本批不含正常 UI/API client、最终新模板、公共 router/Key/结算、完整 Full Vertex 响应、真实 Redis/PG/Hosted、跨进程 receipt 恢复、原生 EXE 或真实账号验收。**
新增模板累计仍为14，不新增整卡 `offline_ready`，不标 R11/W07、P50、P60/P70 或总计划完成。

## 实现合同

### 1. 正常管理权限与显式目标

- 新增 `/v1/internal/gateway/console/vertex-auth-sessions` 的 create、`/:session_id` status 及 `/:session_id/:action` complete/cancel/import。
- create/action 沿原管理认证、Secret Grant 与 Origin 校验；status 仅需相同管理身份，不读取或推进授权。owner 绑定管理 token fingerprint、Host 和 client IP。
- JSON 使用 closed schema，显式 client ID、可选 client secret、project/location/可选 quota project，不允许请求选择授权/token endpoint。
  create 限16KiB，action 限32KiB；成功、权限拒绝和 extractor 错误都覆盖 `Cache-Control: no-store`。
- 先核可写 coordinator、未 draining/shutdown、真实 provider/typed Full Vertex 产品、稳定 credential ID 与原 revision，才监听或交换授权码。
- 新账号的 project/location 必须匹配已保存父级业务目标；quota project 由这次显式输入捕获到 credential context，不从父级缺省 context 推断。
  父级原始 headers 不能被带入新账号，因为原 schema 只允许从 credential context 派生 quota header；在交换前拒绝此类不可保存配置。
  稳定 ID 重授权则核对完整已有 target，包括 quota project，不能偷偷改变受众。
- action body 必须携带原 session revision。complete 与首次 import 重查 live revision；已接纳保存的重复 import 只查原 receipt，不因新版配置被错误阻断。
  cancel 可以取消旧 revision 的 session，但不能借当前 revision 操作另一会话。

### 2. 单次授权与实际 loopback

- 先绑定 `127.0.0.1:0`，再用实际端口创建独立 Google desktop PKCE attempt；wire 的固定 Google origin/cloud-platform 与原绝对期限保持。
- listener 只在回环地址服务，每会话顺序处理一个连接；32KiB header、64项 header、5秒完整读取预算，响应写入最多1秒。
  接受精确 Host 的 origin-form `GET /?... HTTP/1.1`，拒绝 absolute target、重复 Host、非零 Content-Length、Transfer-Encoding 和同批附加 body/pipeline 数据。
  callback URL 从已捕获 redirect 与 query 构造，不信任请求 Host 选择目的地。
- 自动 callback 与手动 complete 共用同锁消费门：取走 attempt/client 后进入 Exchanging；无效 callback、取消、超时或调用方 Drop 都不能恢复可重发的 code。
- 自动交换由 Registry 的 joined listener task 持有，不依赖浏览器连接存活；callback 仅交换到 Ready，**绝不自动保存**。
  手动 complete 是远端浏览器的粘贴原 callback URL 备用入口；其 HTTP waiter 丢失会留下明确失败/未确认交换，不重发 code。
- Ready 只在服务端保留 token/context；session view 和浏览器固定纯文本响应均不返回 client secret、access token、refresh token 或上游诊断。
  普通所有权释放不是内存 zeroization，不宣称物理内存已清零。

### 3. 有界生命周期与保存

- 每 runtime 最多16个被保留 session、32个尚未 join/reap 的操作；TTL600秒。取消、过期、失败会停止 listener 并移除临时授权材料。
  失败/取消/过期 session 会被回收，不提供跨进程或长期错误 receipt；前端后续需将不可用会话视为需重新授权。
- listener/exchange 与 accepted save 都归 Registry JoinSet；runtime 后台启动 expiry owner，HTTP server 收尾请求 shutdown 并 await join。
  admission 与 shutdown 共用锁；异步 bind 不在锁内等待。Drop 只作为 abort fallback，不替代正常 join。
- import 从当前 snapshot redaction 开始，目标 access/RT/client secret 分别 replace/clear，其他秘密 keep，经过原 document validation；新 ID 为 `vertex-{uuid}`，重授权保留稳定 ID/operator metadata。
- token expiry 保存 exchange 时已得到的绝对值，不因 Ready 等待或保存重设；refresh endpoint/client/context 归原 credential owner。
- prepare 成功后释放 Ready 秘密，进入 Importing；accepted save 不由 HTTP waiter 持有，publication lock 等待与原 revision/CAS commit 共用20秒预算。
  成功才发 Succeeded receipt；CAS 输掉、activation/timeout/Drop 为 SaveUnconfirmed，不自动重交、不换 revision 重基、不恢复 Ready。
  已接受保存的重复 import 是幂等查询，包括操作容量满时；非秘密保存 receipt 保留300秒。

## 运行证据与失败记录

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-vertex-oauth-management-20261105/`。
Cargo 均使用 `--locked`、最多4 jobs、无 management-ui、默认线程栈；临时状态和 TEMP/TMP 均在该目录。
只使用合成材料和随机 loopback peer，不读取 `.ng`、ADC、历史凭据或真实客户端配置，不向 Google 发送授权/刷新/业务请求。
测试专用 endpoint 捕获后显式传入 spawned task；缺少 fixture 时仅能到回环拒绝端口，不新增生产 endpoint override。

1. `red-management`：实际 **0 passed / 2 expected failed**，尚未注册路由，404分别不满足200/403；输入无漂移。
2. `green-initial`：`E0616`，HTTP module 访问 session 私有 target；改用 crate 可见只读 accessor。该次未运行测试，不计通过。
3. `green-management`：**12 passed / 6 failed**。带父级原始 quota header 的夹具可建空池却不能编译有 context 的账号；据真实 schema 修正管理准入和夹具，未放宽 schema。
   新账号保持显式 quota，稳定 ID 重授权比较完整 target；新增父级原始 header 与重授权 quota mismatch 回归。
4. 同轮取消测试的2秒 timeout 不是 listener 残留证明：独立复测期间62次 OS listener 采样均无新增 listener；对已关闭随机 loopback 端口的原生连接拒绝实测2074ms。
   测试等待扩大到10秒以容纳 Windows connect error 延迟，生产读取/写入/取消/join 预算不变；单独的短预算 join/半截连接关闭继续验证真实生命周期。

### 最终通过与来源

| 检查 | 实际结果 |
| --- | --- |
| `green-corrected`，`runtime-defaults`、无 management-ui | **83 passed / 0 failed / 0 ignored**，其中21项本批新增；真实 callback TCP、手动 complete、单次消费竞争、取消/TTL/半截连接/join/容量、普通 SecretPatch/CAS/保存未知、稳定 ID 与 public client 清旧 secret 均有定向证据 |
| `production-check`，同一生产运行组合 | `cargo check --locked --no-default-features --features runtime-defaults --lib --jobs 4` 通过；不含测试 endpoint wrapper 的生产接线已编译 |
| `feature-off`，只保留 `line-gemini-web-reverse`，关闭官方 Vertex | **40 passed / 0 failed / 0 ignored**；含本批 parser 及关闭专属网络/监听前拒绝。不是完整 feature matrix |
| 同默认 EXE 的相邻过滤 | Accio40、Grok75、Kiro46、nested secrets5、token_refresh133/4 ignored、Express87/1 ignored，均0 failed；共386次通过执行，过滤间存在重叠 |
| 静态 | Rustfmt、checker31项、ratchet、工作区 `git diff --check` 通过；ratchet4458文件，14项immutable runtime，governed source >700为0 |
| 来源与编码 | 默认、生产编译、关闭组合各2978项选定输入无漂移；相邻 EXE 前后 SHA256 不变；21项本批 Rust 与本批文档严格 UTF-8 无 BOM，新增15项 Rust 独立 whitespace 无诊断 |

按完整单元测试身份去重，**457个不同 Rust 测试通过，其中22个本批新增**（默认21项、唯一关闭专属1项）。
5个不同 ignored 不计通过：4项需要真实隔离 Redis URL，1项需要专用浏览器 host/前端 fixture，本批未启用。
不重复累计绿色重跑，不把失败 run、ignored、编译检查或未执行的集成/浏览器流程算测试通过。

默认83项与全部相邻过滤使用同一个 EXE：
`C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/target/debug/deps/neuro_gateway-615bb893bc934abc.exe`，
SHA256 `6899FE28C511673534115C4F8C365183FE3112ECC9369F5C70C2068CCBE5A65A`。
关闭组合使用独立 `neuro_gateway-c2994882ae3475b9.exe`，SHA256 `499CEA9BE707BF0639B46CCAA6761FD0AEC5B38B13AD5DEDDEE58987B16C22AF`；
各阶段的输入 manifest 和原日志分别保留，不将它们混称同一 EXE。最终两份测试 EXE 均无残留进程，见 `final-runtime-processes.json`。
权威机器回执：`green-corrected.json`、`adjacent.json`、`production-check.json`、`feature-off.json`、`closeout.json`、`source-audit.json` 与 `checks-classified.json`。

临时检查脚本首轮出现 PowerShell switch 管道语法错误；修复后 formatter/ratchet/diff/audit 各 exit0。
其后脚本把 Git `--no-index --check` 的普通差异 exit1 误判失败；通过有效样本 exit1/无诊断、非法 trailing whitespace 样本 exit3/有诊断确认语义，
分类回执保留原始 exit code，15项新 Rust 均无 whitespace 诊断。没有修改生产源码、checker 或 baseline 来掩盖错误。
之前仅开官方线、缺共享 Gemini Web core 的15项缺符号失败未修或重跑，不报完整矩阵绿。

### 规模与安全复核

本批21项新增/修改 Rust 均低于500有效行，15项新文件，最大409；不新增例外、不重写 baseline。
既有接线文件的准确有效行：console mod45→46、state188→189、runtime377→382、internal_console405→409、management_config338→360；
测试专用 transport helper23→29。写前字节副本或与写前 SHA256 严格相符的重建用于 before 计数，未用猜测前值。

输入/权限/秘密检查保留在原边界与最终网络前；并发消费在 session 锁内完成，所有等待均有预算，listener 和 accepted save 有明确 join owner。
没有新增配置可选的 OAuth origin、无界任务/连接队列、隐式重试、依赖升级或凭据发现。
同一 callback/手动路径、保存与 publication 的测试分别证明其真实改变的路径；仍不把合成 coordinator 或 unit runtime 当作实际 Redis Lua、SQLite UI、OS 进程或真实 OAuth 验收。

## 下一接力点

1. Full Vertex 专用正常 API client/UI、显式 client/target、稳定 ID 重授权及同 owner 的未知保存恢复，再开放最终模板；保持旧 generic Vertex 语义。
   可参考现有 [accio-auth API](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/apps/desktop/src/api/console/accio-auth.ts)、
   [useAccioOAuth](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/apps/desktop/src/features/console/useAccioOAuth.ts) 与
   [AccioOAuthPanel](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/apps/desktop/src/features/console/AccioOAuthPanel.tsx) 的 managed-auth/未知保存 owner，
   但 Vertex 有实际 listener、`redirectUri` 与显式可选 client secret，不能复制其“没有自动监听”文案或其他产品 client。
   UI 只保留必要控件、短状态、错误与即时风险确认，不照搬现存解释段落；输入 target 的 wire 字段仍为 `quota_project`。
2. 正常 UI→真实管理 TCP/SQLite→调用/刷新与 runtime 重建；本批 router oneshot + callback TCP +合成 coordinator 不冒充整条真实本地业务流程。
3. 公共 Chat/Messages/Responses JSON/聚合 SSE、Key/usage/结算、取消/refund 与独立完整 decoder/工具/签名/模态。
4. Vertex-specific Server/Worker 真实 Redis publication/restart/竞争、PG/Hosted、跨进程与断电/failover、旧数据/部署兼容；本批不使用已消费的 P34 容器授权。
5. Express 剩余 decoder 范围、原定其他渠道、Luma Agents/Web/媒体与 P50 全范围集成不删除；真实 client/账号、IAM、project/region/model、refresh 和额度在 P60/P70 集中录制。

未出包、部署、重启在线实例、迁移数据库、提交、推送或创建 Docker/Redis 资源。

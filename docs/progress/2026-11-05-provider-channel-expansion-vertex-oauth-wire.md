# 服务商扩展：Full Vertex 独立 desktop OAuth wire

日期：2026-11-05。对应总计划 R11/W07。

## 完成范围与当前边界

本批实现独立 `vertex_oauth` 协议 owner：显式 Google desktop client → PKCE 授权 URL → 单次 callback/code capability → code exchange → refresh。
使用已有 Rust HTTP client、序列化、时间与加密依赖，不读取 ADC、环境 client、Antigravity 配置、真实 `.ng` 或历史凭据，不修改已有 Gemini header/refresh 兼容语义。

**这只是可独立测试的 OAuth wire 子链，不是正常 Console 登录/保存、durable refresh、Full Vertex 调用或整卡完成。**
累计新增模板仍为14，不新增整卡 `offline_ready`，不标 R11/W07、P50 或总计划完成。

## 已核实的公开依据

本批同日联网只读核实以下官方页面，无授权/token/生成请求：

- [Google native app OAuth](https://developers.google.com/identity/protocols/oauth2/native-app)：desktop public client、loopback IP/随机端口、PKCE、code/refresh 表单、Bearer、`expires_in`、可选 `refresh_token_expires_in`；当前 code/refresh 表中 `client_secret` 标为 Optional。
- [Vertex authentication](https://docs.cloud.google.com/vertex-ai/docs/authentication)：认证材料不等于项目 IAM 或 Vertex 权益。
- [Full Vertex generateContent](https://docs.cloud.google.com/vertex-ai/generative-ai/docs/reference/rest/v1/projects.locations.publishers.models/generateContent)：Full Vertex 的 project/location 资源合同，不能用 Express 的固定全局路径替代。

原始抓取在 `C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-vertex-oauth-20261105/`：`public-sources.json`、完整 `native-app.html` 与提取后的 `native-app.txt`。
Exa 摘要有截断，optional client secret / time-based refresh 依据完整页面，不外推未抓取内容。
DPoP 是可选独立能力，本批未实现；不声称支持 DPoP-bound refresh token。ID token 被忽略，不当作已验证 Google 用户身份。

## 生产实现

1. client ID/可选 client secret 必须由调用者显式提供，材料有界且无控制字符，Debug 不输出材料。
   不内置或借用别的产品 client，不猜 client 后缀或从 token 格式认定产品。
2. listener owner 必须先 bind loopback，再把实际非零端口交给 `AuthorizationAttempt`。
   重定向固定 `http://127.0.0.1:{port}/`，使用 URL 规范化；48随机字节 verifier、32随机字节 state，OS entropy 失败明确报错。
   授权固定 Google endpoint、`cloud-platform` scope、S256、offline 与 consent；secret 不进入授权 URL。
3. attempt/code 非 Clone、非 Serialize，600秒 monotonic deadline，callback 消费 attempt，code exchange 消费 code。
   callback 有16KiB/16参数上限，绑定 origin/port/path/state；拒绝 fragment、重复参数、code/error 混合及错误 issuer。
   未验证 scope/authuser/未知元数据不成为账号身份，拒绝或错误不回显私有 query/prose。
4. token success 使用 typed serde：忽略未知字段，重复 known 字段、错误类型/显式 null、非法 token、缺失/无效 TTL、DPoP token_type 和多余 scope 均拒绝。
   `expires_at` 与 refresh deadline 从请求开始 UTC 时刻计算，不在收包时重置寿命；使用 checked add，拒绝已过期结果。
   初次授权必须得到新 refresh token；refresh 可以省略 RT，此时保留原 RT。已知 time-based refresh deadline 只取更早值，不因刷新或新 duration 延长。
5. 只有官方 Vertex feature 开启时才编译 HTTP owner；关闭时 exchange/refresh 在网络前固定 `FeatureDisabled`。
   生产 token origin 固定；仅 `cfg(test)` task-local 可以切到非零端口127.0.0.1 HTTP peer，不能通过用户配置改变 OAuth origin。
6. `.default_headers(false)`、禁 redirect、form encoding；不继承调用者默认 Bearer/Cookie/API Key。
   一个最多15秒的 send+read总预算，同时受 code/RT 剩余寿命限制；不自动重试，不启动后台 task 或第二套 scheduler。
   Content-Length 预检加累计解码64KiB上限，gzip/分块/截断有真实 loopback 测试；取消依赖拥有的 future/stream Drop。
   400/401 仅按固定 machine error 分类，不匹配 prose；错误、Debug 不携带 callback/token/上游诊断。

## 验证与失败记录

所有 Rust 验证 `--locked`、最多4个Cargo jobs、无 management-ui、默认线程栈；未设置 `RUST_MIN_STACK`，未加栈或新任务规避测试。

| 检查 | 结果 |
| --- | --- |
| scaffold 功能红测 | `red-wire-v2`：3 passed / 14 expected failed / 0 ignored，其中1项是已有 Express 身份隔离测试；不是编译失败代替红测 |
| wire 定向 | `green-wire-final`：24 passed / 0 failed / 0 ignored，其中23项本批新增，1项已有preset测试；`green-wire-v2`同样24项通过，之后只移除未用import并修正测试模块注释，最终源码已重新编译实测 |
| 代表性 feature-off | 关闭官方Vertex、保留已有Gemini Web core：14 passed / 0 failed / 0 ignored；13项纯类型/解析复核，加1项专属关闭测试，code/refresh均在网络前FeatureDisabled且listener零连接；不替代完整feature matrix |
| 相邻 OAuth | 同最终默认栈EXE：Accio25、Grok52、Kiro25，合计102 passed / 0 failed / 0 ignored；前后EXE与2943项输入未漂移，见`adjacent.json` |
| checker | 31 passed / 0 failed；最终ratchet通过，4423文件，14项immutable runtime，governed source >700为0 |
| 来源与差异 | 15项本批文件的严格UTF-8无BOM、有效行与scoped whitespace通过；2943项绿色输入无漂移、写前快照中无本批范围外Rust变化。当前全工作区`git diff --check`也通过；未由本批修剪先前3项前端末尾空行，不回写前批的历史失败记录 |

最终按测试名去重为 **127项不同Rust测试通过**，其中24项本批新增（23项官方功能开启、1项仅关闭时编译）；
不把14项关闭组合与24项默认组合里的13项重复纯测试再次累加。没有ignored算通过。

```powershell
cargo test --locked --no-default-features --features runtime-defaults --lib --jobs 4 vertex_oauth -- --test-threads=1
cargo test --locked --no-default-features --features line-gemini-web-reverse --lib --jobs 4 vertex_oauth -- --test-threads=1
```

两次编译分别对应`green-wire-final.json`和`feature-off.json`；三组相邻测试直接使用前者EXE，哈希见`adjacent.json`。
前批“仅官方Vertex、没有Web core”组合的15项共享Gemini符号失败未由本批修复或重跑，不能报完整feature矩阵绿。

工具/夹具失败保留，不计通过：

- 首次 `rustfmt` 对 Windows shell 的 literal wildcard 报文件不存在；后用明确模块入口格式化。
- `red-wire` 链接失败 `LNK1104`：临时 runner 的 TEMP 子目录未创建；只补建本批 linshi 子目录，之后取得实际功能红测。
- `green-wire` 编译失败 `E0599`：新增测试误用锁定 time 版本不存在的 `OffsetDateTime::MAX`；改用公开 `PrimitiveDateTime::MAX.assume_utc()`，不改生产到期语义。
- 首次 ratchet 的外部 absolute JSON 路径被 checker 拒绝；改用唯一 repository-relative `target/validation/vertex-oauth-20261105/ratchet.json`，未覆盖原有 dirty 报告或修改 checker。
- 临时审计初次把`git diff --no-index --check`的普通内容差异exit1误判失败；本批linshi正反证分别为合法文本exit1/空输出、尾随空白exit3/明确诊断后，修正临时审计判定。没有放宽仓库checker或忽略whitespace输出。

## 安全、资源与规模复核

新职责仅位于 `src/protocol/vertex_oauth.rs` 和同名子目录；共享 `src/protocol/mod.rs` 只注册模块。
Accio fixture 只通过测试 path 复用其 synthetic HTTP server；所有响应显式改成 Google schema，不借产品材料，不修改原 fixture。
没有数据库/凭据迁移、外部进程或独立后台定时任务owner，没有新增依赖、无界队列、阻塞网络/文件工作或重试。
解析、query 与响应保留均有界；code/refresh 是一次拥有的 async attempt，取消测试明确销毁整个 owned future 后等待服务端 body Drop，再 stop/join fixture server。
有效行与最终哈希见 `audit.json`；所有新增源码/测试均小于500，不新增例外或改baseline。

| 文件（均在`src/protocol/`下） | 有效行 |
| --- | ---: |
| `mod.rs` | 102→103，仅模块注册 |
| `vertex_oauth.rs` | 新增115 |
| `vertex_oauth/authorization.rs` | 新增136 |
| `vertex_oauth/material.rs` | 新增108 |
| `vertex_oauth/transport.rs` | 新增79 |
| `vertex_oauth/transport_http.rs` | 新增93 |
| `vertex_oauth/authorization_tests.rs` | 新增195 |
| `vertex_oauth/material_tests.rs` | 新增172 |
| `vertex_oauth/transport_tests.rs` | 新增195 |
| `vertex_oauth/transport_boundary_tests.rs` | 新增175 |
| `vertex_oauth/transport_test_support.rs` | 新增23 |

## 下一接力点：不允许被本批替代的工作

1. `vertex_oauth_context` 作为 credential-only typed identity，绑定明确 client/grant/scope 与 project/location/quota-project 等实际受众；不能从同邮箱推定权益或借用 Express Key。
2. 原声明/最终payload校验、DB payload stripping、nested SecretPatch/Keep 的 destination/consumer 比较与正常暂存受众；新增可选context保持旧 absent hash兼容。
3. 现有 refresh grant identity/material、reserve→Sending→Result→Published、revision publication、replay/cache owner与真正 scheduler接线；不建立平行持久化系统。
4. 独立最终 Bearer 构造，拒绝旧 `auth_token` 遮蔽新 access token；不能只沿 generic refresh 写 `api_key` 后就开放模板。
5. 正常管理授权/session/listener/create/status/cancel/import、稳定ID/普通保存/原revision-CAS、正常UI与重启恢复；wire owner本身不拥有管理登录或listener。
6. Full Vertex最终URL/method/resource/region/publisher/model验证、协议/usage/结算/取消与最终模板；真实client、账号、IAM、模型及额度留集中P60/P70验收。

Express 尚未闭合的多candidate、未支持Part/Preview/模态、signed Preview关联、实时参数流，以及其余原定渠道与P50/P60/P70继续保留。
本批未读取真实凭据、调用真实上游、建Redis容器、出包、发布/部署、提交或推送；不修改运行实例。

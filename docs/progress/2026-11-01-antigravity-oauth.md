# Antigravity OAuth 独立接入与本地验收

## 范围与当前结论

新增独立 `antigravity-oauth` 服务商模板、专用 OAuth 导入入口、native credential
保活接线、Cloud Code 调用适配器和 `antigravity` 指纹适配器。采用 Gateway 自有代码和
既有 credential／route runtime，不运行参考项目的服务或可执行文件。

本轮是代码接入和离线合同验证；未执行真实 Google OAuth、真实 refresh 或真实生成。
模板保持未完成、不标蓝，不能把本地合同、用户信息读取或模型目录读取当成生成验收。
没有修改当前运行实例、真实账号和数据库，没有打桌面 EXE 包、提交或推送。

## 三个职责

### 凭据导入与保活

- Google 官方 OAuth，PKCE S256、随机单次 state、offline refresh token；
  loopback callback 为 `http://localhost:51121/oauth-callback`。
- 浏览器只供用户正常手动授权；可粘贴同一次授权的完整回调 URL，不导出浏览器状态。
- 管理身份、Host、IP 绑定；创建和导入要求 secret grant。状态回执不包含 token。
- 从官方 userinfo 获取稳定 Google ID，按该 ID 去重；重新授权保留 credential ID、
  disabled 状态及其他操作员策略。不从未验证 JWT 或调用者字段决定账号身份。
- 读取 `loadCodeAssist` 的真实 project，再读取 `fetchAvailableModels` 的真实目录；
  不伪造项目、自动 onboarding 或代用户接受条款。没有真实目录时不预填模型。
- 复用既有 token refresh scheduler 的提前刷新、single-flight、generation 防覆盖、
  失败退避及 refresh token 轮换持久化，不增加每账号浏览器或第二个刷新循环。
- 原生凭据沿正常 `RouteConfigRuntime::commit_document` 增量提交，无存储版本变更。
- 授权会话十分钟；已开始的提交跨过 TTL 后继续保留查询能力；成功后只保留五分钟
  不含 token 的回执，不重新交换授权码或重放导入。

### 独立调用适配器

- `antigravity_oauth_compatible`／`special_adapter=antigravity_oauth`，不是普通 Gemini API。
- 使用 Cloud Code `v1internal` 外层 project/model/requestId/requestType/userAgent，以及
  账号和项目隔离的 sessionId；Canonical ↔ Gemini 内层格式复用公开转换边界。
- 接入文本 Chat Completions、Completions、Messages、Responses；支持基础工具结构和
  显式 tool choice。多轮工具、Claude thinking/signature 等行为未做真实兼容性验收。
- JSON 与 SSE 解开 `response` envelope 后转换为 Canonical；SSE 采用有界聚合，
  不是实时逐 token 透传。不把空、partial、blocked 或未完整终止的输出登记为成功。
- 仅向官方 HTTPS Cloud Code endpoints 发送凭据，禁止重定向；生成失败不自动跨账号、
  跨端点或重放。请求 16 MiB、生成响应 32 MiB、目录 2 MiB，网络操作有 deadline。
- 不支持独立图片／音乐／视频生成端点和 stored Responses ID 续接。

### 专用指纹适配器

- `fingerprint_adapter=antigravity` 由独立 executor 拥有，不能供普通 API 或其他 executor 使用。
- 最终发送明确关闭共享 Chrome 默认 headers，应用独立 Antigravity UA、Bearer、
  JSON/SSE headers，使用独立连接池 group 与非浏览器 TLS baseline。
- 保留共享 transport 的代理及证书验证策略。不读取、重置或伪造本机 Antigravity 设备标识。
- 本地真实 HTTP capture 验证 browser default headers 不泄漏；这不等于真实客户端
  TLS ClientHello 逐字节复刻，也没有完成跨平台身份或 Google 风控接受度验收。

## 本地验证

使用 `--locked --offline --jobs 4`，保留缓存、依赖锁文件和既有 dirty edits。
测试临时材料及脱敏日志位于
`C:/Users/Public/nas_home/AI/GameEditor/linshi/antigravity-tests-20261101`。

- 初始新增合同测试先红：尚无 preset／fingerprint enum 和 UI 模板；接线后通过。
- 初轮新增库测试 14/14，通过；公共入口初次运行发生 Windows 调试测试线程
  `STATUS_STACK_OVERFLOW`，未记为成功。依照既有全入口合同设置独立 16 MiB 栈，
  再跑库及两套集成测试共 18/18 通过；没有因此修改产品运行栈或发送上游请求。
- 复核发现 SSE 的异常终止原因可能被后续 STOP 覆盖；新增回归实际先红
  （`sse-terminal-regression-before.log`）。改为每个终止帧立即验证，拒绝重复终止，
  不允许把 blocked／非法终止洗成成功。
- 最终 `cargo test --locked --offline --jobs 4 --lib --test antigravity_contract
  --test console_antigravity_oauth_contract antigravity`：19/19 通过（库 15、公共 API 3、
  管理 API 1），包含新增 SSE 冲突回归；没有执行上游 Google 请求。
- 最终既有 `token_refresh::` 16/16、`protocol::call_flow::` 5/5、
  `routing::config::refresh_state_tests` 5/5 通过，共 26 项；均非零用例过滤。
- 桌面 native `authorization_browser`：7/7 通过，包括精确 Google endpoint allowlist、
  shell-free 参数传递、超时回收和退出取消。测试不实际打开浏览器。
- UI 模板／专用 OAuth 面板／原 ChatGPT 面板／调用流程／指纹选择相关回归：
  7 文件 63/63 通过；最终 TypeScript typecheck 通过。
- 有效行 checker 自测：31/31 通过；最终 ratchet 与 `git diff --check` 通过。
  ratchet 为 3,846 文件、14 个精确归类的 immutable runtime artifacts，
  governed source 超过 700 行为零，没有改动 baseline 或新增豁免。
- 本轮 53 个相关文本通过严格 UTF-8 无 BOM 检查；50 个受管源码／测试已由权威
  lexer 测量。新增模块最高 270 有效行，全部受影响源码最高 433 行；无需尺寸豁免。
  共享非流式 dispatcher 为 423→433、stream dispatcher 为 332→341、native
  browser launcher 为 149→152；没有增长任何超过 700 行的历史债务文件。
- 本轮相关 Rust 文件定向官方 rustfmt check 通过。

源码 before 为 `C:/Users/Public/nas_home/AI/GameEditor/linshi/antigravity-before.json`；
最终测量及文件指纹为上述隔离目录内 `source-final.json`、`scope-final.json`。
编译保留现有无关 unused/dead-code 和 linker-message warnings，没有做无关修复。

## 实现与参考

实现说明：`docs/antigravity-oauth.md`。2026-11-01 联网核对的只读参考 revision：

- `anshulrules/Antigravity2Api`：`b373165050b32ce4b390bf3a952d73ececc979d4`。
- `su-kaka/agy2api`：`6cc45620f1ecbe12bda7792b2daf8940d8a4ffde`。
- `Draculabo/AntigravityManager`：`8b4b5eafb8f59c844ec985595bdec316fa0e9466`。

仅参考协议结构和共同公开的 installed-client 配置；未下载或运行 README 链接中的载荷。
真实 Google 授权、模型可用性、额度及生成仍需独立验收，不据参考项目的声明推定成功。

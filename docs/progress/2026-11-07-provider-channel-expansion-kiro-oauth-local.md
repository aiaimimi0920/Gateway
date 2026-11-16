# Kiro OAuth：正常 UI、公共 TCP 调用与 Local 持久化重建

日期：2026-11-07。工作区：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`。
继承 `main` / HEAD `7969f12c85efceacf2a5e384e143b5c39a550981` 的大量未提交改动；HEAD 不代表本子批源码。
承接[显式 profile runtime 子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-07-provider-channel-expansion-kiro-oauth-runtime.md)，不新增模板或生产发送语义。

## 改了哪些内容

新增 Kiro OAuth 的正常管理、真实入站 TCP、公共协议与 SQLite 结算测试，以及显式 opt-in 的正常浏览器宿主。
复用已有 CLI Key 测试 owner 的生产 `build_app_state`、Local 配置事务、TCP router 和有界关闭；仅增加可选 test-only OIDC scope、通用公共请求方法及 OAuth registry 的 awaited join。
模块接线均受 `cfg(test)` 和 Kiro feature 约束，不向普通程序开放任意目的地 override，不另建服务、数据库或凭据机制。

主要源码：

- [模块及最终 wire 断言](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/client/kiro_oauth_local_tests.rs)。
- [正常管理授权及公共 Key fixture](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/client/kiro_oauth_local_tests/fixture.rs)。
- [三协议、Keep、clear 与重建](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/client/kiro_oauth_local_tests/flow.rs)。
- [HTTP/流错误、退款与取消](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/client/kiro_oauth_local_tests/failures.rs)。
- [正常浏览器 opt-in 宿主](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/client/kiro_oauth_local_tests/browser_fixture.rs)。

## 实际证明的路径

自动 TCP 测试从空池开始，经正常管理 register/device/poll/import 保存完整 OAuth context，目录 Region 为 `us-east-1`，显式 profile 的业务 Region 为 `eu-central-1`。
Chat / Messages / Responses 各自 JSON 和 SSE 共六次，再在重建后调用一次 Responses SSE。
普通公共 Key 使用真实 Local SQLite，消息余额 10→4→3；audit 必须是同一 provider、credential 和原公共 alias→精确上游模型。
最终 sender 断言准确 Bearer、profile、`Exact.Model/Version` 与 AWS target，没有 Key-only `TokenType`、caller Cookie 或 caller UA。

metadata-only Keep 保留 access、refresh 和 nested client secret；同目录、同端口重建后完整 context、credential ID、revision 与余额保持。
旧 runtime Grant 在重建后返回 403，新 Grant 才能清除三项秘密；再次重建后材料仍为空，账号身份保留，未多发生成或重新授权。
这不是旧格式材料的自动迁移，也不是两个独立 Gateway 原生进程。

错误组覆盖三公共协议 × JSON/SSE × 401/302/native late error，共 18 个场景。
每次只发送一次，audit 为 failed，未知 Token 不补值，本地消息预占退回，不泄露任意上游正文或秘密，不合成正常终态。
三协议的实际 TCP SSE reader Drop 另证 cancelled audit、本地退款、上游 body Drop 以及模型健康 failure_count 不增加。
退款仅指 Gateway Local 预占，不证明供应商撤销、退还 subscription credits 或远端取消。

### 正常浏览器联合验证

使用正常 `main.tsx` 页面，静态产物仅写到 `linshi`，透明回环代理连接上述生产 bootstrap/TCP 服务。
代理不改 Host/Origin/auth/body、不伪造 Grant；正常页面管理登录并确认权限。
本批专用 IAB 页面完成：添加 `kiro-oauth` 空池、显式 Profile ARN、选择 Builder ID 与 scope/machine、单步检查合成授权、保存到凭证池，再通过正常账号编辑保存手动精确模型。
只填“用户模型”而未关联上游目标不会产生完整模型映射；本次没有用 API 偷补，最终以正常“手动模型列表”保存并在重建后重新打开确认。

模型 metadata 保存未重新授权，三个秘密 descriptor 仍 configured；正常管理 API 建普通公共 Key 后，Chat JSON 与 Responses SSE 分别调用成功，余额 4→3→2。
第一次重建完整 serialized credential 和 revision 保持，页面 reload 可见同账号与调用统计。
随后**通过正常管理 API**取得新 Grant、显式 clear `api_key`、`refresh_token`、`kiro_oauth/client_secret`；不是声称 typed OAuth UI 提供明文 client secret 编辑或完成 UI clear。
第二次重建同 ID/profile/模型保留、三个 descriptor 均未配置；公共调用返回 400，余额仍 2，合成业务上游严格只收到之前两次请求。
合成 OIDC 严格只收到 register/device/token 三次；未打开真实 AWS 授权链接。

![正常模板保存及两次公共调用](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-kiro-oauth-local-20261107/browser-ui-success.jpg)

浏览器测试只有正常 UI 录入/模型 metadata；公共调用、充值和 clear 由正常 API 驱动，不能扩大成全部业务都在 UI 操作。
fixture 完成后 awaited 关闭 HTTP、OAuth registry、SQLite 与合成上游才写 `completed.json`；宿主和专属 Node 服务 exit 0，本批两 PID/两个监听端口复核均已退出。

## 验证与来源

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-kiro-oauth-local-20261107`。

| 检查 | 实际结果 | 回执 |
| --- | --- | --- |
| 正常 `runtime-defaults` 的 Kiro 库测试 | 206 passed / 2 ignored，含本批三项自动测试 | `kiro-final-v1.json` |
| 新 OAuth 浏览器宿主显式 `--ignored` | 1 passed / 0 ignored | `browser-ui-v1.json` |
| 正常生产库 `cargo check --lib` | passed；locked/offline，关闭默认 features，显式 runtime-defaults，4 jobs | `production-final-v1.json` |
| 正常 main 的隔离 Rsbuild 构建 | passed，未运行 publishing/删除 hook | `ui-build-v1.json` |
| 官方 Rustfmt | passed | `rustfmt-final-v2.json` |
| checker 自测 | 31/31 passed | `checker-v1.json` |
| 官方有效行 ratchet | passed；4789 文件、14 immutable runtime artifacts、governed source >700=0 | `ratchet-final-v2.json` |

Rust 按完整名称去重 **207 passed**；默认运行跳过的两个 host 中，新 OAuth host 已显式通过，旧 CLI Key host 本批仍未执行，不计通过。
本批新增四项测试（其中一项必须显式启用浏览器宿主），不叠加前批 236 项或重复运行数。
首轮 `local-first-v1` 的 2 passed / 1 failed / 1 ignored 原样保留：native 错误很早发生时，HTTP 头尚未提交即可关闭连接，测试原来错误地 unwrap 了 `wreq_proto::Error(IncompleteMessage)`。
修正仅限测试 helper：只允许故意构造的 native late-error + public SSE 场景提前连接失败，仍须 audit failed、退款、一次发送与脱敏；普通 HTTP/redirect 不放宽，没有改生产语义来测绿。

五个终态后端/静态阶段各 5102 项所选输入在阶段内无漂移，且与最终源码 hash 相符。
UI 构建和上一批 `typecheck-final-v1` 的 desktop 消费子集、checker 的 scripts 子集另行核对未变；本批不无故重跑未修改的 UI 测试/类型。
扫描不覆盖全部 node_modules、外部 `@neuro/ui` file dependency 或动态输入，不能称所有 UI 依赖/全仓冻结。
收尾 audit 的一次脚本失败是误认为存在 `initial-source.json`；实际没有该文件，改用已存在的首轮测试 source snapshot 检查后续源码变化范围，不补造任务开始时的全仓快照。直接 preimage 仍逐项校验原 hash。
八项源码经官方 lexer 复算：`client.rs` 497→499、原 Key fixture 200→200、共享 runtime 146→179；五项新增分别 56/111/131/193/154，有效行均不超过 499。未改 baseline/exception。
UTF-8 无 BOM、范围 diff、本地链接、来源与 fixture/截图/临时脚本 hash 见 `audit.json`、`effective-lines.json` 与 `scoped-diff.patch`。
本批 ratchet 报告先归档，再从**本批直接 preimage**恢复共享报告，保留继承 dirty 内容；`git diff --check` 通过。既有 warnings 不当作零警告。

## 接力边界

本子批闭合的是显式 profile runtime 的正常 UI→真实本地管理 TCP→保存、公共调用/Local 结算及同进程重新 bootstrap 的限定路径。
没有启动通用 background scheduler，不以之前 scheduler 定向证据代替这次端到端刷新；没有独立原生 Gateway EXE、OS 重启、Server/PG/真实 Redis、跨应用 receipt 或整包。
当前最小 scope、profile 发现/订阅/客户端资格完整合同、旧 route/未变 source hash DB 自动兼容、完整业务参数/decoder 与全部取消阶段、真实模型/额度仍须继续。
未读真实 `.ng`/历史凭据、未调真实平台、未创建容器、未替换实例、未出包或提交推送；P34 历史授权不复用。
模板仍为 17，不增加整卡 `offline_ready`；R02/W02、P50/P60/P70 和总目标保持执行中。

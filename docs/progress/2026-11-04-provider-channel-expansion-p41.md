# P41：Accio OAuth 独立协议与发送前安全 gate

日期：2026-11-04（Asia/Shanghai）。状态：本批协议/安全 gate 的定向验证通过；扩大 feature-disabled 测试另有 4 项既有用例失败。R01/W01 和总目标仍在执行，不报告全仓或全矩阵通过。

## 1. 本批范围

继续原计划 R01/W01 的无凭据开发。固定参考已联网复取为 `MYCF711/magpie-accio-auth@62993d5d7dc872ea4f62ad50116b698c62ea513a` 的 `src/constants.mjs`、`src/login.mjs`、`src/credential.mjs` 与 `LICENSE`；这是 MIT 社区 wire 参考，不是官方第三方 client 许可或真实服务器轮换证据。独立 Rust 实现的参考说明与原始 notice 位于 `src/protocol/accio_oauth/`；未采用推广邀请、宽松 callback、原文错误、JWT 身份猜测或缺 expiry 默认寿命。

本批的最窄可验证结果是：建立固定受众的独立授权/换码/刷新协议 owner，并阻止已知 Accio 材料误入通用 form 刷新。**不是正常管理登录、模板开放、完整刷新持久化或 R01 完成。**

## 2. 实际行为

- 显式 client，无内置 `accio-work`；China/Global 仅选登录页面，不推定额度共享，也不自动增加两张卡。
- 48-byte PKCE verifier/S256、32-byte state、16-byte ttid；固定 loopback callback，严格 origin/path/port/query、重复字段、state、错误分支校验。消费式 attempt/code grant 共享 600 秒 monotonic TTL；没有新建 listener、后台 task 或私有 endpoint 配置入口。
- 固定 JSON code exchange 与 current access+refresh；一次 HTTP 尝试，最多 15 秒 send+body 总预算，禁止 redirect，禁 client 默认 headers，64 KiB 响应累计上限。无自动重试，错误只导出固定本地码。
- typed envelope 必须 `success=true`，拒绝重复关键字段，两个非空有界 token 与 absolute expiry 必须完整。原始数值十进制运算，不经 f64；只接受明确数值或带时区 RFC3339，不造默认 lifetime。有效旧 expiry 仍保留，不把迟到轮换结果变成新寿命。
- 新 sgcookie 替换，省略/null 保留旧秘密；不在刷新请求发送 Cookie，不从 token 推定账号/utdid/配额。
- scheduler 的 `Prepared::new` 在 reservation 前对 Accio adapter 或 phoenix-gw endpoint 可恢复拒绝，避免 generic form 误发；未永久标死账号。后续完整生命周期接线必须替换该临时 gate，不能把 gate 计作功能完成。

## 3. 验证与证据

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-provider-expansion-20261104-p41`。

已实测生产未改时的三项行为红测：正确 endpoint 被送进 Generic、外部 endpoint 接受 Accio 材料、generic adapter 伪装 Accio endpoint 均失败（`red-accio-guard-v2.log`，0 passed / 3 failed）。更早 v1 是测试命名冲突导致 `transport::RefreshError` 的 E0425，只修测试类型路径；不将该编译失败当行为红测。v2 排队等待前一 Cargo 的目录锁，未同时运行两份 Gateway rustc。

最终成功检查如下，`test-summary.json` 按完整测试名去重，不能直接相加重复执行：

| 日志 | 实际结果 |
| --- | --- |
| `green-accio-v1.log` | 68 passed；25 个新协议用例、3 个新 gate 用例、40 个既有 Accio 用例 |
| `regression-refresh.log` | 95 passed / 4 ignored；ignored 是要求显式隔离 Redis 的既有 opt-in，不是 Redis 通过证明 |
| `regression-grok.log` | 52 passed |
| `regression-routing.log` | 119 passed |
| `feature-disabled-protocol.log` | 23 passed；只关闭 Accio leaf，保留其余 runtime-defaults |
| `feature-disabled-guard.log` / `feature-disabled-planner.log` | 3 + 1 passed |
| `checker.log` | 31/31 passed；checker 源码未改，不因业务变更反复重跑 |
| `rustfmt-final.log` / `ratchet-final.log` | `cargo fmt --all -- --check` 与无报告写入的 ratchet 均 exit 0 |

上述成功日志共 361 次通过，**341 个不同 Rust 测试，其中 29 个本批新增用例**；未将下面失败 run 的 42 次通过额外累加。15 项非 Markdown 输入（14 个源码与 LICENSE）与测试前 `source-frozen-v1.json` hash 完全一致；最终全部 20 项范围记录为 `source-final.json`。本批 6 个既有文件均有逐字节 before 副本，不用整个 dirty 工作树的 diff 冒充本批增量；源码有效行最大 201，三项既有源码分别为 protocol/mod 97→98、grant 119→128、tests 200→201。UTF-8 无 BOM、尾空白、scoped diff 审核和 `git diff --check` 均通过，没有修改 baseline 或 exception。

### 3.1 扩大 feature-disabled 的失败保留

`feature-disabled-accio.log` 在同一源码上成功编译后实际为 **42 passed / 4 failed**（exit 101）。失败名称：

- `provider_credential_folder_sync::tests::normalization_web::normalize_accio_import_payload_converts_manager_account_file`
- `upstream::headers::tests::browser_sessions::accio_forwards_runtime_cookie_and_cna_headers`
- `upstream::headers::tests::browser_sessions::accio_sets_required_headers`
- `upstream::headers::tests::header_precedence::accio_custom_headers_override_defaults`

这些既有用例没有区分 feature 关闭：前者仍要求被明确 compiled-out 的导入成功，后三者仍要求已关闭实现的 headers。四项在 enabled 的 68 项回归内通过；失败文件和它们依赖的旧 compiled-out 实现本批均未修改。没有为让测试变绿而放宽关闭产品的行为，也没有修改旧测试或把失败标 ignored。后续同一二进制的本批协议/guard 与既有 compiled-out planner 合同 23+3+1 全部通过；该定向结果不覆盖这四项失败，更不证明全部 feature 组合。旧用例的 feature 分流转后续 P50 矩阵治理。

所有认证材料为合成值，HTTP peer 仅随机回环。没有读取 `.ng`/历史凭据，没有公开站点认证/生成请求，没有新建 Redis/Docker 对象，没有启用 management-ui/工作区发布 hook，没有出包、替换运行实例、提交或推送。

只读复审未发现本批范围内可证实的协议缺陷；提示的压缩响应边界已补真实 loopback 用例并通过，97 bytes gzip 解压后的 65,537 bytes 被上限拒绝。当前实际链接的 `wreq@0.16.1` fingerprint 启用 gzip/br/zstd/deflate，未启用 cookies（`http-dependency-features.json`）；只实际测试了 gzip，不外推其他压缩格式，默认 header 隔离测试也不泛化为未来启用 CookieStore 后已获证明。首轮 checker 命令未显式改写 TEMP/TMP，测试使用系统临时目录创建、清理其自有隔离夹具；后续验证脚本已显式设置到本证据目录，不借此清理任何既有状态。

## 4. 后续责任与停止边界

1. 共享 scheduler/永久 grant Result 的绝对 expiry 与 sgcookie 必须先具备精确绑定、持久化、replay、撤销/迟到结果及重启恢复，再接专用 Accio Prepared；不复制一套调度器。
2. 已静态确认旧副本风险：Accio planner 目前不从 `payload.api_key` 固定 body 认证（`src/upstream/accio/request_plan.rs:16-48`），随后 generic `extra_body` 的 `or_insert` 会留下旧 `token/accessToken`（`src/upstream/client/request_plan_part_1.rs:156-178`）；当前 runtime override/持久投影只改 api_key。下一批须先用真实 planner 红测固定，并在 Accio 专用 planner 绑定当前凭据，不能只同步 importer 副本。额度请求明确使用 `payload.api_key`；当前精确 Console probe 对 Accio 为 Unsupported，没有发送旧 token 的 probe 运行证据。
3. 接正常管理身份、Secret Grant、已保存 preset/精确受众、原 revision、显式稳定 credential ID、utdid 录入/保留与普通 SecretPatch/CAS；不要把 OIDC/JWKS/Grok principal 类型借给 Accio。
4. 然后完成正常模板/UI、精确业务/额度负例与取消等合同；官方 client 使用边界和与旧导入是否同一产品/额度继续核实。完成所有可无凭据开发后统一录制和真实验收。

新增前端模板仍为 11 个，没有新增完整 offline_ready 的 W 卡。P50/P60/P70、其他原定渠道和总目标仍未完成。

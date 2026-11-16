# P49 后续增量：Kiro 旧 route 账号显式重新授权

日期：2026-11-04（Asia/Shanghai）。本批定向离线验证通过，承接已有
[管理后端](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-04-provider-channel-expansion-p49-console.md)及
[UI/API client](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-04-provider-channel-expansion-p49-ui.md)。
不是总计划 P50，不把 R02/W02 标为整卡 `offline_ready`，不新增模板或执行真实登录。
分支 `main`，HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`；保留共享工作区其他脏改。

## 1. 实际缺口与本批改动

原 `resolve_machine` 和 target admission 要求已有 typed `kiro_oauth`；旧 route credential
即使能够读取及使用原 bearer，也无法从现有账号入口重新授权。真实管理 router 红测复现了
`400 / kiro_login_rejected`，而不是缺模块或编译错误。

- 新增 `console::kiro_oauth::legacy`，仅服务显式重新授权的材料准入与设备保真。
  不推断旧注册、directory、scope、client ID 或注册 expiry；仍通过原 public registration
  和操作员明确选择的 directory/scopes 取得全新材料，不使用旧认证副本发起新 grant。
- 精确 `kiro` preset、稳定 credential ID、管理身份/Origin/Secret Grant、原 revision/CAS、
  accepted-save 的 20 秒预算及未知结果不重投等合同不变。稳定 ID 仍不能由 supplied machine 改设备。
- 设备来自真实编译后 payload：显式 UUID/32/64 hex 沿旧算法规范化；否则保留旧 extra RT
  派生值；只有根 `refresh_token` 时保持旧 header owner 实际使用的全零值，不新造设备。
  同值的继承/账号别名规范化为同一设备；冲突、非法显式值及不一致的业务 Region 在网络前拒绝。
- 不替换 provider 共享认证，避免影响 sibling。provider 级旧认证副本、外族 context、
  非 DirectHttp、认证/身份 header override 等继续拒绝；业务发送目标的既有 allowlist 未扩大。
- 保存仅替换指定账号的认证。旧 scalar aliases 和 `rawSource` 的嵌套/数组秘密先通过原
  SecretPatch `clear` 清理，再退役该账号的旧认证容器和注册 metadata；不将旧 secret `keep`
  到新的授权上下文。未知额外秘密仍受原 Keep 受众约束，不新增迁移特权或宽松兜底。
- 新授权缺 refresh/expiry 时仍明确清旧值；新 context 固定旧机器身份。原账号 ID、标签、
  enabled、模型设置、非认证 metadata、sibling、其他 provider 及已归档旧文档均保持。
  取消、尚未显式保存时不修改旧材料。

本批只处理正常 Console 的 route credential。未修改 folder source、未变 `source_hash` 的
PostgreSQL 历史行、启动迁移 owner 或 storage version；**显式重新授权不是自动存储升级**。
原生 Kiro Key 没有被接成 OAuth，也没有新增 `kiro-api` 模板。

## 2. 验证与失败保留

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-kiro-legacy-reauth-20261104`。
Cargo 固定 `--locked --jobs 4 --no-default-features`，主配置为 `--features runtime-defaults`；
只为测试进程设置 `RUST_MIN_STACK=16777216`，TEMP/TMP 指向上述 linshi 目录。

| 检查 | 实际结果 |
| --- | --- |
| 生产未改红测 | `red-legacy.log`：1 passed / 3 failed；三项 create 都实际返回 400 而非预期 200 |
| 首次绿测 | `green-kiro-v2.log`：45 passed / 1 failed；新保存、认证清理、设备和 sibling 断言已通过，最后读取旧 bootstrap archive 得到 None |
| 失败归因及修正 | `ConsoleStateFixture::new` 只建立内存 bootstrap；仅让夹具先正常提交建立真实历史 revision，保留归档内容保护断言，未修改生产语义 |
| 最终 Kiro 协议/管理 | `green-kiro-v3.log`：46 passed，包含 5 项新增旧账号用例 |
| 刷新 | `refresh.log`：13 passed |
| 真实管理准入 | `admission.log`：4 passed |
| 正常文档与 typed context | `document-context.log`：70 + 15 passed，另 1 个合成前端 fixture opt-in ignored，不是 Redis 测试 |
| 主配置去重 | `verification-summary.json` 按 suite + test name 统计：**148 passed / 0 failed / 1 ignored**，不叠加前批重复运行 |
| 代表性 Kiro 关闭配置 | `feature-off-admission.log`：5 passed；关闭模式继续在 target lookup/网络前拒绝 |
| 静态与来源 | Rustfmt、checker 31/31、ratchet、diff 和 6 项 Rust 摘要/UTF-8 无 BOM 检查通过 |

`source-green-input-v3.json` 与最终 6 项源码一致；相对 v2 只有归档夹具变化。
`carry-forward-audit.json` 确认前批 18 项 UI、16 项未变 Rust 摘要保持；另外 4 项 Rust
为本批声明的接线变更。前端未改，复用前批 228 项测试、类型、隔离构建与合成浏览器证据，
不把它们冒充本批重新执行。新增最大 242 有效行；接线为 67→68、103→135、104→126、306→307，
不改 effective-line baseline 或 exception。线性 metadata 扫描仅发生在管理准入/保存；
无新后台轮询、重试器、子进程或独立存储，资源生命周期沿既有 registry/save owner。

`final-scope-audit.json` 的 6 项源码摘要及 4 份文档编码/空白检查通过，但全历史链接扫描另发现
总计划第 10.3 节引用的 `gateway-provider-plan-20261102-ea3f870d` 下两份旧临时日志当前不可读取；
因此该合并审计 exit 1，未报告为全绿。旧引用保留，未删除或重造历史证据，不用新日志替代旧日志。
本批 checker/ratchet 有独立实际运行记录。最终共享工作区出现其他变更和构建进程，本批命令均已正常退出，
不将其他 Cargo/rustc 进程自动归属本批或停止；不声称整个工作区来源、构建与验证已收口。

本批仅使用合成材料、随机回环 OIDC peer 和既有 Console fake Redis；未读取 `.ng` 或历史凭据，
未请求真实上游、创建 Redis 容器、修改在线实例、出包、提交或推送。

## 3. 接力边界

- 下一步继续旧 route/未变 source hash 的 DB 兼容升级；不能用本批显式重新授权抵销自动升级要求。
- 当前最小 scope、真实 Builder verification URI、profile 发现/选择、ARN/业务 Region 与
  订阅/模型权益的 exact HTTP，以及当前 client/产品/额度，继续按 Kiro 合同调查与实施。
- 原生 Key direct HTTP 的固定许可参考仍未采用；跨应用/进程重启的管理 receipt 恢复仍缺。
- 本计划累计新增模板记录仍为 12；Vertex OAuth/Express、Luma Agents、各 Web/媒体完整生命周期、
  Grok 余项、Qwen OAuth 受限待决及 P50/P60/P70 保留原范围，不要求现在录制凭据。

# Accio 新导入 OAuth 材料归一化：限定离线子链

日期：2026-11-08 开始，2026-11-09 验证。R01 接力子批，已取得真实行为红测并实现新导入修复；新增10项与相邻回归已通过，不表示旧 DB 兼容、DB 刷新或整卡已完成。

## 已核实的责任边界

旧 folder normalizer 会保存 access/refresh 字符串，却丢弃显式 SG、refresh endpoint/client，只保留字符串 expiry，并在业务 extraBody 中重复复制 access/device。新 OAuth 管理登录已具有另一套正确的 credential-only SG 与绝对 expiry 保存合同，不能削弱该合同来兼容旧导入。

真实 folder→DB 链在 normalize 前比较原文件 bytes 的 SHA256；未变化即跳过。DB reader 不重新调用 folder normalizer，rawSource 仅是最多32层去嵌套归档，没有自动迁移 owner。**仅修 normalizer 不会自动修复已有同 hash 的 DB 行。**

当前实际 token refresh scheduler 仅消费 route-config compiled credentials；另一 PG refresh steward 仅处理 ChatGPT Web。cache repair 虽能读取 DB camel refresh/SG 字段，也不是从 DB 建刷新任务。因此补齐导入字段只能先证明材料完整保存，不能冒称 Accio DB 自动刷新已接通。未设计或执行绕过旧 source hash、自动覆盖已轮换材料或强制重写数据库的方案。

本子批先完成新导入的完整材料闭环：保留 access/refresh/SG/显式 client、复用协议 owner 的绝对 expiry 精确解析、固定有效业务与 refresh 受众、移除业务 body 的重复认证副本，验证原普通 planner 仍从 canonical access/device 绑定认证。旧 DB 同 hash 兼容和 DB 刷新 owner 继续保留为总计划必做项，不因本子批结束而退役。

## 改了哪些内容

新增 `normalization/accio_material.rs` 负责新导入的授权材料边界，原 Accio normalizer 只作接线。root/extraBody/extra_body 的同义材料冲突拒绝，SG 保存为 credential 顶层 `accioSgcookie`；显式 refresh client 保留，不制造第一方身份。只有已有 refresh 与显式 client 且未提供 endpoint 才补固定 Accio endpoint；外来 endpoint、非固定有效业务根、坏 token/选定 device 均拒绝，诊断不回显材料。

expiry 直接复用协议 owner 的精确绝对时间解析，不复制算法或构造 `now + ttl`；整数秒/ms 和字符串保留，已经由 JSON Value 变为 f64 的小数数字拒绝，避免假称保留原纳秒或延长期限。合法已过期材料仍可保存，不表示 access 可用；没有 expiry/client/refresh 的旧材料不凭空增加这些字段或刷新权限。

业务 extraBody 不再复制 `token/accessToken/utdid`。正常 native planner 已实测仍从 canonical `apiKey` 与 device header 绑定正确 token/utdid/精确模型，不发送 refresh、SG、client 或 rawSource。原账号 identity、显式 Cookie/cna、device header 和其他业务元数据维持原逻辑；本批不宣称清理 Cookie 内未知 SG 语义、全 metadata 幂等或改变所有旧字段的选择策略。缺 access 的原 passthrough 语义未变。

新增9项 normalizer 合同与1项正常管理脱敏回归；旧 manager account 用例只将 body 认证副本断言改为不存在，其余 identity/Cookie/cna 等断言保留。canonical SG、归档 rawSource 和其嵌套秘密均复用原正常脱敏，不修改脱敏生产逻辑。协议 owner 仅开放原 expiry/atom 校验的 crate 内可见性，解析行为未变。

## 已运行的验证

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-accio-import-20261108`，保留直接修改前 bytes/hash、完整日志、各阶段输入清单与终态 JSON。首轮7项 normalizer 合同实际得到 **1 passed / 6 failed**，不是编译错误；随后实现修复并补3项回归，原7个测试身份仍保留并全部通过。

当前冻结 lib 测试程序为 `rust-green.exe`，SHA256：`57f12152d18156ffa59031b98bc3ab7545b2c666c7aab4a29a23985df7736bd5`。来自本次 locked/offline、`runtime-defaults`、4 jobs 的真实 Cargo no-run 构建；仅根包 test debug=1 区分既有被占用产物，未删除缓存、杀外部进程或改仓库 profile。

| 同一冻结 lib 的选择范围 | Passed | Failed | Ignored |
| --- | ---: | ---: | ---: |
| `accio_import`：10新增及1旧导入用例 | 11 | 0 | 0 |
| `accio` | 120 | 0 | 0 |
| `provider_credential_folder_sync` | 195 | 0 | 14 |
| `token_refresh::` | 145 | 0 | 4 |
| `credential_payload::tests` | 3 | 0 | 0 |
| **按完整测试名去重** | **434** | **0** | **18** |

18项 ignored 为既有真实 PG/Redis opt-in 测试，不计通过，本批没有启动这些服务或重跑 P34 Lua。新导入→普通 planner 和正常脱敏已验，不将它们冒称实际 PG folder 导入、旧 DB 修复或 DB scheduler 验收。

开启 `runtime-defaults` 及从其只移除 `line-accio-web-reverse-api` 的代表性关闭生产 lib 均通过 locked/offline、4 jobs 的真实 `cargo check`；关闭检查等待外部 Nous 编译释放共享 build lock，沿同一 handle 等到成功，没有另起重复编译、杀其他 owner 或重跑全仓。未执行关线单测或完整 feature matrix。

9份 Rust 官方格式检查、checker 31项和有效行 ratchet 通过；ratchet 扫描5144文件、14个 immutable runtime artifacts、governed source >700=0。9份源码/测试均≤500有效行：旧 normalizer 289→251，旧 web 测试460→447，新增材料 owner133、新增合同254；未修改 policy/baseline/exceptions。原有效行报告在本批执行前备份到证据目录，成功后按直接 preimage 恢复，不覆盖其原有 dirty 内容。

构建期间外部 Nous 与共享 refresh receipt/runtime/Redis owner 的20项输入发生变化，完整列表见 `rust-green.json`；生产开启期间另有 `src/console/runtime/pooled_backend.rs` 变化，关闭期间有 `tests/console_refresh_grant_contract/receipt.rs` 变化。本批9份 Rust 在全部阶段前后保持同 hash，并由冻结 EXE 运行上述测试；不把这次结果宣称为最新全仓单快照联合绿，也不接管外部 owner。最终完整名去重、阶段终态、范围源码/UTF-8/无BOM、本地链接与直接 preimage diff 审计分别保存为 `final-proof.json`、`scope-audit-*.json` 和 `scope.diff`，全仓 `git diff --check` 另见 `diff.json`；不叠加中间或历史通过数。

首次临时 proof 因真实红测 stdout 与 panic stderr 交错而漏读一行测试身份，明确失败；只修证据脚本为完整名去重并独立核对原1/6终态，未改生产、测试或原日志。原因与原断言见 `audit-proof-first-failure.txt`。一次文档补丁上下文不匹配未应用，随后按实际短上下文补入；这两项工具失败不计业务测试失败或通过。

## 仍需继续与安全边界

- 已有同 source hash DB 行仍在 normalize 前跳过；需要另行实现兼容 owner，保护稳定 ID、轮换后材料及 snapshot CAS，不能用 hash 前缀强制重导覆盖。
- DB Accio refresh task 尚未接：当前 token scheduler 只消费 route-config compiled credentials，另一个 PG steward 只处理 ChatGPT Web。能读 camel 字段的 cache repair 不是任务入口。
- Accio 专属完整取消 audit、真实获准 client、当前业务终态/wire、模型/额度关系及完整运行模式/进程/包仍未完成。
- P50 全范围无凭据集成与 devtest、P60 集中录制、P70 真实逐渠道验收继续；不在此时要求录真实凭据。

本轮另外匿名联网重新读取 Udio 两份官方文档，均返回200：public API仍未提供，audio/video/stems下载仍停用。原始页面、文本与SHA256见 `C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-udio-20261108-readiness/public-sources.json`。不因旧song URL增加绕行下载、转存播放流或假音乐验证；Udio有效播放/精确probe仍未完成。

无真实 `.ng`/凭据/供应商调用、Redis/Docker/PG服务、现有实例修改、出包/提交/推送。P34授权已消费，不可复用。模板仍17，无新增整卡offline_ready，P50/P60/P70与总目标继续。

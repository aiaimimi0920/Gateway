# AIGateway fork 能力采纳：第五批

日期：2026-10-20（Asia/Shanghai）。本批落实 A04 的一个安全切片：route-config provider pool 显式 `oauth_only` 硬资格约束，使用 Gateway 的 typed document、编译快照、轮询、模型映射与 refresh owner 重新实现。**不是完整 OAuth-first / prefer-fallback，也不表示全部候选已采纳完成。**

状态：本切片源码与下列本地回归已完成，Rust全量、all-targets、formatter及行数门禁通过。Desktop仍有12项同名既有失败，未进行打包或真实运行验收，因此全仓发布门禁不是通过状态。

## 已实施

- 新增 provider 字段 `credential_pool_auth_policy: any | oauth_only`。默认 any 省略序列化，保留旧文档、普通轮询与 unsupported-model legacy fallback 的语义；未知枚举值拒绝读取。
- 严格模式只接受仍 active 且 refresh token / endpoint 非空白的 typed refresh owner。不根据 token 形状、bearer、session、profile、套餐或价格判断 OAuth，不发起鉴真或预热调用。
- 普通选择、单目标映射、多目标映射与模型列表应用同一业务池资格约束。缺项、pending discovery、禁用、组外、模型不支持及默认 base payload 均不旁路选择到 API key。
- 显式管理 probe 保留 exact credential 与 mapped model 语义，可测试被业务策略排除的 API credential；不偷换为合格 OAuth 账号，不以 allowed IDs 是否存在猜测调用者。
- 字段参与 provider fingerprint；无关配置变更复用 owner，policy 变更使旧 owner retirement。旧快照之后的严格选择失败关闭，不承诺撤回已选 payload。

功能与边界见 [凭据池认证资格约束](../credential-pool-auth-policy.md)。本批未增加专用 UI 开关、数据库字段、storage version、迁移、预算 owner、后台任务或新网络请求。

## 取舍与范围

原框架会在健康筛选之前，从每个 route-config provider 的池中先选一个 credential。仅给后续队列加 OAuth 权重，无法看到已经被池选择排除的 OAuth；SQL 候选还可能先过滤冷却账号。若据此实现 prefer，很容易把“暂不可用”误判为“未配置 OAuth”，或在同 provider 内绕过 fallback 限制。因此先做可证明的硬资格，不制造名不副实的全局 OAuth-first。

此字段不约束 DB / Redis / access-catalog 的独立候选，不阻止其他 provider 接手，也不替代 pin、授权、健康、quota、预算或发送时撤销合同。完整 A04、完整 D01 reasoning 历史往返、B01/B02 共享预算与单次恢复、B03/B04/B05 外部 advisory 仍需各自独立合同；不是本批已完成项。不采纳无限恢复、指纹鉴真、请求外 ticket ping、DOM 补丁或周期重启的结论不变。

## 工程与保全

开始快照有155项 dirty 内容及1766个 tracked Rust 文件的 hash / 有效行数。本批变化12个源码 / 测试文件，最大471有效行；新测试模块261行，专门覆盖同一 policy 合同。没有新超限文件、501+例外或 baseline 修改，没有接管 S06。

`src/routing/config.rs` 与 `candidates.rs` 是两个原已dirty文件的预期窄增量；已与 before-files 逐项核对，保留第四批 probe identity wiring。另外窄修既有 `maintenance/blocking_tests.rs` 测试夹具，47→49有效行，原文件已完整备份。其余152项原dirty内容未变。冻结清单覆盖1852个源码 / 现场 / 功能文档文件，最终hash全部一致；测试修正前快照与日志独立保留，没有以重冻覆盖首次失败记录。

原始16项现场、参考 fork 的219个源码文件、HEAD、branch、index 保全检查通过。没有 fetch、整理或修改参考仓库，不称本地 fork 为最新上游。未读取或改写 live `.ng` credentials，未改变本地数据 owner、端口8317、计划任务、部署实例或其他 Neuro 项目。

安全 / 资源复核覆盖精确账号与授权、不记录或改写 secret、锁序、owner retirement、临时资格与发送间隙、线性扫描及分配。新增 strict 锁序与现有 refresh 一致；Any 短路不获取该锁。不新增 SQL、文件路径、队列、任务、句柄或进程生命周期。独立只读评审未发现本切片阻塞项，mapped catalog 断言与注释精度建议已落实。

## 验证回执

先用4项真实红测试确认缺口：普通轮询选API、映射选API目标、strict仍走unsupported fallback、新字段未保留。实现后第一轮10项中9项通过；另1项是空白endpoint夹具被既有URL验证提前拒绝。保留该日志并修正期望，没有放宽产品校验；补强 bearer/profile、mapped catalog 与 default probe 反例后最终10项聚焦通过。

首次全量lib为3478 passed / 1 failed / 36 ignored，失败是既有取消测试：`Weak::upgrade()`返回None后，文件锁仍可能在同一Arc的字段析构中，立即拿锁得到WouldBlock。独立std程序用受控Drop barrier确定性重现了“弱引用不是析构完成fence”。修正仅等待实际文件admission释放，保留原5秒deadline、未完成时两个锁均须保留的断言，并使非WouldBlock错误立即失败；没有修改产品实现、忽略测试或靠重跑覆盖失败。首轮全量、all-targets和fmt回执保存在`first-full-run/`，修正后的聚焦/重复与完整回归另行记录。

| 检查 | 本轮实际结果 |
| --- | --- |
| 新 policy 聚焦测试 | 10 passed / 0 failed |
| Rust 全量 `cargo test --locked --offline -- --test-threads=1` | 98 suites；4082 passed / 0 failed / 104 ignored |
| `cargo check --locked --offline --all-targets` | exit 0；仍有unused / dead-code等warnings |
| `cargo fmt --all -- --check` | exit 0 |
| 取消测试修正后聚焦与重复 | 聚焦1 passed；独立连续100次全部通过，完整Rust回归再次覆盖 |
| Desktop TypeScript | exit 0 |
| Desktop 全量 `src`，`--no-cache --maxWorkers=1` | 695 passed / 12 failed；不是全绿 |
| 与第四批比较 | 12个失败名称完全相同，无新增失败名称；本批未重跑隔离 baseline 副本 |
| Node scripts | 2122 passed / 0 failed / 2 skipped |
| Python | 358 passed / 0 failed / 4 skipped（总362） |
| line manifests | 44 checked / 0 errors |
| checker tests / ratchet | 31 passed；ratchet exit 0，未运行 strict |
| scoped diff check / 新文本编码 | exit 0；最终文档与新增文件另有UTF-8无BOM及空白检查回执 |

所有 Cargo 操作在本仓库内串行，使用默认线程栈、清除 `RUST_MIN_STACK`、`--test-threads=1`，并设置 `GATEWAY_PREBUILT_WEB_UI=1` 复用既有ready marker，未触发checkout Web发布或输出清空。Node测试进程的 TEMP / TMP 预先使用规范化路径。本批没有改变前端源码、放宽 timeout、筛掉原12项失败或修改断言以制造全绿。

取消测试夹具修正后重新执行Rust聚焦、100次重复、完整Rust回归、all-targets、formatter、checker tests与ratchet；Desktop / Node / Python / manifests复用同轮已结束的静态回执，其实现和测试源码没有随后修改。本批没有重跑前端隔离baseline，也没有把同名失败比较当作全绿证明。

未运行真实供应商、独立 PostgreSQL fixture、Server Redis live、44条独立 feature-line 矩阵、生产Web重建、真实浏览器交互、native EXE、发行或部署验收。不得据本批合成测试或复用Web标记声称全仓门禁通过或可直接发布。

证据：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-fork-adoption-phase5-20261020/`，包含 before / frozen scope、源码manifest、设计与复核、红绿日志、首次失败与修正后完整回归、确定性析构证明、100次重复、静态检查、前端失败比较、现场保全及`verification-summary.json`。其中`fullRepositoryGatePassed=false`刻意保留真实边界。本批未 commit、push、打包EXE或部署，Gateway HEAD 仍为 `08634912809fea48b45fd888936cd688356fdfac`。

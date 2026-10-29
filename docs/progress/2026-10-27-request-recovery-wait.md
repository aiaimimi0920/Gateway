# 2026-10-27 请求恢复等待

## 范围

已获用户开发授权；本轮不生成交付包、不替换或重启当前 4200 实例，不操作真实账号数据。
实现合同见 [请求恢复等待](../request-recovery-wait.md)。范围以桌面／Local 的
route-document 服务商为主；PostgreSQL／Redis 动态账号的完整初始 cooldown、轮换和
SQL reservation 生命周期不宣称已完成新增验收。

## 改动

- `src/request_wait/`：可信 provider 模式、临时拒绝白名单、有界进程内等待队列、FIFO 恢复准入。
- `src/pipeline/request_budget*`：唯一原始起点；默认 300 秒，可选 600／900 秒；官方模板
  长等待可去除默认总截止，但不能放宽显式预算或复活终止 owner。
- `src/pipeline/stage_send/recovery_wait.rs`：锁定同一已授权账号／模型；不重跑路由、预扣或
  reservation；等待后重取 token override、分组和本地 Key 权限。明确容量拒绝一事一许可，
  实际发送／审计计数不回退。未移交流前才能重新准入。
- `src/routing/admission.rs`：临时 circuit 候选可以保留等待，真实发送 gate 仍执行；
  余额、手工禁用、权限、站点规则不借等待绕过。余额拒绝仍可转向既有授权备用账号。
- Native HTTP、通用 HTTP、CPA Rust transport 使用独立 managed 生命周期，避免隐藏的
  transport 总 timeout 先截断长等待；连接建立保持有限超时。
- 桌面模板创建设置新增“恢复等待”选择，不把配置写入上游 extra_body。

## 验证过程与真实问题

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-recovery-wait-20261027`。
日志可能包含合成 fixture token，不包含真实账号凭据。

1. `before.log`：先跑预算合同红测，2 项失败，证实原代码拒绝超过 300 秒。
2. `check.log`：首次共享 Cargo 检查等待锁，随后遇到另一修改中的 Anthropic stream
   match 类型错误；该处已由其原修改方修复，本任务没有接管其代码。
3. `focused-runtime-r2.log`：发现 Windows 默认测试线程栈溢出。发送 future 的取消边界改为
   指针大小的 heap frame，不以增大 `RUST_MIN_STACK` 作为最终验收方式。
4. `diagnostic-large-stack-only.log`：仅用于定位业务逻辑，不作为通过凭据。真实 TCP 断开、
   连续 7 次拒绝、SSE/tools、权限撤销路径已能运行；发现虚拟时钟先于真实 SQLite/IO
   完成的测试竞态，改为先取得真实拒绝再推进短 deadline 的虚拟时间。
5. `focused-runtime-r3.log`：默认线程栈下新恢复路径通过；原 IP-block 测试的“立即失败”
   预期与新默认长等待冲突。旧 owner 合同显式使用 `disabled`，新等待另有独立回归，
   不删除原授权、封禁和流式安全断言。
6. `focused-runtime-r4.log`：账号分组套件 14 项通过；发送套件 51／53 通过，两项只在
   临时 SQLite 目录清理时报 `os error 32`，精确单测独立重跑通过。fixture 仍核对唯一
   自建路径，只对 Windows sharing/lock violation 做最多 5 秒的有界等待，其他错误不吞掉。
7. `final-core.log`：140 项中 138 项通过，新增 OAuth 夹具因缺少必填 `base_url`
   在反序列化阶段失败；补齐合成 endpoint 后，`final-core-r2.log` 默认测试线程栈下
   140 项全部通过（退出码 0），包含过期等待、新 token override 和永久刷新错误负对照。
8. CPA 是根项目的 path dependency，不是 workspace member，根目录 `-p` 测试入口被
   Cargo 拒绝；其独立旧锁文件也不能在 `--locked` 下直接使用。没有修改这两个锁文件。
   在证据目录建立私有 harness，直接引用原始 `src/lib.rs`，校验 manifest 依赖与原包一致，
   私有锁文件中 186 个包的版本／checksum 均已存在于根锁文件，然后用 `--locked --offline`
   执行原 crate 的完整 20 项单元测试。`cpa-harness-lock-check.json` 保存一致性证据，
   `cpa-transport-final.log` 保存实际通过记录；前面的入口失败日志保留，不冒充测试通过。

## 最终验收记录

| 验证 | 结果 | 回执 |
| --- | --- | --- |
| 核心等待／预算／容量／OAuth 等定向单元测试 | 140 passed，退出码 0 | `final-core-r2.log` |
| 完整账号分组运行合同 | 14 passed | `final-runtime.log` |
| 完整发送链路运行合同 | 53 passed，包括此前两项 Windows 清理失败的用例 | `final-runtime.log` |
| 10／15 分钟配置与实际预算延长 | 2 passed | `final-runtime.log` |
| 共享预算、明确拒绝、累计 retry wait 相邻合同 | 12 + 8 + 7 + 2 passed，退出码 0 | `adjacent-budget.log` |
| CPA 原始 transport 源码的独立完整单元测试 | 20 passed，退出码 0 | `cpa-transport-final.log` |
| 前端恢复等待写入正常创建流程 | 5 passed | `final-static.log` |
| TypeScript typecheck | passed，退出码 0 | `final-static.log` |
| effective-lines checker 与 ratchet | 31 passed；ratchet 退出码 0 | `final-static.log` |
| 官方 Rust formatter 与差异空白检查 | passed | `final-format-diff.log` |

以上共 258 项 Rust 定向测试，不是全仓库回归。最终默认测试线程栈运行，没有使用
`RUST_MIN_STACK` 放大栈空间来掩盖问题；构建仍有既有 warnings，不宣称零警告。
`effective-lines-final.json` 实测本轮 49 个源码／测试范围均不超过 500 effective lines，
并校验 UTF-8 无 BOM；发送主文件从本轮初始 493 行降至 488 行。等待职责单独放在
cohesive module，没有更新 baseline 或申请例外来放过增长。
`scoped-diff-review.log` 保存定向差异，`final-verification.json` 汇总通过回执、源码哈希和锁文件未变校验。

运行场景已覆盖：连续 7 次明确容量拒绝后第 8 次成功（JSON 与 SSE/tools）；真实 TCP
客户端断开后不再后台发送；原始 deadline 到期后终止；等待期间账号停用或分组权限撤回；
初始临时 open circuit 不被丢成零候选，但也不能提前发送；SSE 移交／部分输出后不重放。
队列单元测试覆盖总量、每 Key、每 provider、保留量上限，以及取消后的计数／FIFO 释放。

未执行：真实付费上游调用、生产数据迁移、EXE 包装和原生手测、全仓库回归／正式发布门禁。

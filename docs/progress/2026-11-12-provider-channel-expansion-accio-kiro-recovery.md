# Accio / Kiro 授权会话失效恢复

日期：2026-11-12。R01/R02 正常授权 UI 的恢复缺口修复；不表示真实登录或整卡完成。

## 问题与修改

真实后端 `console/accio_oauth/mod.rs` 与 `console/kiro_oauth/mod.rs` 均将缺失会话映射为本产品的 `*_login_rejected` code 与 `*_login_unavailable` message。两个前端 hook 未提供共享 `useManagedOAuth` 已有的 unavailable 策略，导致确定不存在的会话仍作为网络失败重试，active 状态不释放，阻止原界面重新发起授权。

只修改两个产品 hook：精确匹配本产品错误码与消息，复用共享恢复路径。未提交保存的失效会话释放，但不自动新建登录；保存已发出、结果可能丢失时保留 uncertain 与重复保存栅栏，关闭重开不重复 create/import。容量、revision 或其他错误不误判为会话已消失。Kiro 仍只在用户明确操作时推进 device poll，定时检查仍为 GET。

没有修改共享 hook、后端协议、授权/保存权限、凭据或持久化逻辑，也没有新建重试/恢复框架。

## 验证

- 每个产品新增4项回归：未保存会话释放与明确重新授权；提交结果不明后回执失效及重开防重；错误消息不匹配、错误code不匹配两项负向检查。
- 修复前两个 lifecycle 文件：4 failed / 23 passed，实际失败是session未释放和确定失效回执未停止轮询；不是修改测试放宽要求。
- 修复后10文件（Accio/Kiro/Grok生命周期、共享unavailable、四产品相关面板/控制）：**103 passed / 0 failed**。包含本批8项新增，不把前批数量累加。
- `npm run typecheck` 通过；checker31/31、ratchet（5485文件，14 immutable runtime artifacts，governed source >700=0）、`git diff --check`通过。
- 临时行数统计首次误传 lexer 语言名 `typescript` 报错；核对官方映射后用 `c-like` 重新统计通过。此调查脚本错误不影响先前已通过的正式ratchet。前值由精确移除新增段重建：Accio hook9→11、测试95→137；Kiro hook10→12、测试67→109。未修改policy/baseline/exceptions。
- 沿用现有前端格式；package scripts 无独立前端formatter，不引入新依赖。没有修改组件渲染、effect生命周期或轮询频率。

测试、类型和checker日志：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-oauth-recovery-20261112`。本批仅前端修复，不重复编译Rust或出包；mock管理API证明UI恢复合同，不冒称真实后端重启、真实OAuth或原生包业务验收。

## 交付边界

上一轮devtest包不包含这些后续源码修改。没有读取真实凭据、调用供应商、产生费用、修改运行实例/现用数据、提交或推送。新增模板仍17，无新增整卡offline_ready；P50/P60/P70及其他核心功能继续。

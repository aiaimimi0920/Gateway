# 路由失败归属与授权备用账号修复

## 完成范围

已完成源码修复与定向验证，不代表正在运行的 4200 已更新，也不代表真实服务商验收。
本次续接 Session `01a1ebff-1922-7d81-a91f-6a2779a5d338` 的两项要求：
失败日志显示供应商及具体账号；首选账号不可用时使用已授权、支持同一模型的备用账号。

## 现场结论

只读复核 `gw-audit-ff56be09-c3c9-4c27-8486-b0be055ae916`：
`gpt-6.1-sol` 请求耗时 9ms，`routeAttemptCount=0`、`candidateCount=0`，
`providerAccountId`、`realCredentialRef` 和 `upstreamStatus` 均为空。
这是发送前拒绝，不能归因为某个实际被调用的上游。
历史配置 `r495-7da6adc87d28` 的该模型规则列出 `chatgpt`、`openai-api`、
`zzz-api`；对应账号均为 `enabled=false`。当前配置不用于反推历史。

## 改了哪些内容

- 候选解析与发送前准入保存受限的 `rejectedCandidates`，包含供应商、账号及原因；
  上限 100 条，不记录凭据内容，不暴露请求授权分组外的账号。
- 失败没有成功候选时，审计沿实际发送起点记录的最后一次身份显示供应商标签与账号；
  零发送不生成供应商故障分类，拒绝候选不增加实际发送次数。
- 账号明确配置的模型目录也可证明服务商支持该模型，解决供应商汇总目录未同步时
  授权备用账号被遗漏的问题。启停、分组、模型、认证、发送预算及回退授权仍执行。
- 运维表格区分“未发送到上游”和实际上游失败，展示发送前跳过的账号及原因；
  保持历史记录兼容，不重写历史审计。
- 修复诊断接线的 Rust 借用冲突，按准入诊断职责提取代码；本轮入口时
  `stage_route.rs` 为 510 有效行，完成后为 489；`stage_send.rs` 从 508 降至 500。
  16 个相关源码文件均不超过 500 行，UTF-8 无 BOM。

## 验证结果

| 检查 | 结果 |
| --- | --- |
| `access_key_groups_contract` 与 `search_tool_audit_attribution_contract` | 18 passed |
| 候选回退、认证策略及实际发送归属定向 lib tests | 13 passed |
| 运维归属、统一运维与工作区前端测试 | 12 passed / 3 files |
| TypeScript typecheck | passed |
| effective-line checker tests | 31 passed |
| effective-line ratchet | passed，governed source >700 为 0 |
| `cargo fmt --all -- --check`、`git diff --check` | passed |
| 验证前后相关源码哈希比较 | 16 个文件一致 |

隔离 loopback 回归证明：停用首选账号且无授权备用时零发送并保留拒绝身份；
仅具体账号声明模型时仍可调用授权备用；合成 HTTP 400 失败记录实际备用供应商与账号；
组外账号、其他模型、禁用规则不被当成备用。原有恢复等待、断开取消和权限撤回回归通过。

完整命令、日志、UTF-8/行数/哈希及最终回执位于
`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-routing-attribution-20261118/final-verification.json`。
先前编译失败日志保留，最终通过回执分别为 `rust-contracts-resume-r2.json` 和
`rust-lib-focused.json`，不以早期结果替代本次验证。

## 未执行的工作

未调用真实或付费上游，未修改真实凭据、账号启停、运行数据库或 4200 实例；
未创建新手测包、执行正式发布、提交或推送。原生界面手测与上线验证另行进行。
该条历史零发送日志没有实际调用身份，不能通过补写历史来制造一个失败供应商。

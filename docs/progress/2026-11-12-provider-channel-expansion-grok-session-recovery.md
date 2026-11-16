# Grok OAuth 会话回收后的恢复

日期：2026-11-12。范围：R05 正常授权 UI；不是整卡、P50 或真实 OAuth 验收完成。

## 已修复

后端 `src/console/grok_oauth/mod.rs` 将失效会话作为 `grok_login_rejected / grok_login_unavailable` 返回。Grok 前端没有连接共享 `useManagedOAuth` 已有的 unavailable 策略，因此即使服务端已明确会话不存在，仍按暂时性网络错误重试，并保留 active 状态，阻止在原界面重新发起登录。

`useGrokOAuth.ts` 现在精确匹配上述错误组合，复用已有恢复机制：未提交保存的会话释放，用户可以明确重新授权；已提交保存但回执不可用的会话保留 uncertain 与重复保存栅栏，关闭重开也不得重复 create/import。不改变服务端授权、revision、Grant 或凭据保存逻辑，不将普通网络错误归为失效会话。

## 验证

- 新增两项真实 wrapper 回归：失效未保存会话可明确重新授权；保存响应丢失后回执消失仍防止重复提交及重开误保存。
- 修复前 `useGrokOAuth.lifecycle.test.tsx`：2 failed / 9 passed。失败分别为 session 未释放、确定失效回执未立即停止轮询。
- 修复后 lifecycle、普通 Grok 面板、设备授权面板及共享 unavailable 四文件：31 passed / 0 failed。
- `npm run typecheck` 通过。
- 有效行 checker 31/31，ratchet 通过，5478 文件、14 immutable runtime artifacts、governed source >700=0；`git diff --check` 通过。
- 使用官方 lexer 对本轮增量重建前值：hook 9→11，测试 122→155。前值是修改后按精确新增段移除得到的计数，不冒称事前运行记录。项目 package scripts 无单独前端 formatter；保持相邻格式，不引入新格式工具或依赖。

本轮不修改 Rust，无需重编后端或重复其回归；没有真实凭据、收费调用、数据迁移、运行实例变更、提交或推送。已发布 devtest 包保持冻结，**不包含本轮源码修复**。

## 剩余核心项核验

只读交叉核验确认 Perplexity `snapshot.rs` 仍明确拒绝非空 `diff_block`；`request.rs` 仍限单轮 turbo，无会话续接。11 月 8 日专属回执也明确 op/path 与上下文合同尚未冻结，因此不能删 guard 或猜成 JSON Patch 来伪造完成。Luma OAuth/MCP 的 discovery 不等于 Gateway 客户端注册许可或业务工具合同。后续应补这些精确协议证据，而不是重复 Grok/Qwen 余额测试或打包。

旧身份表部分缺口已过时：实际源码已有 Grok device 管理入口与 UI，不能继续按旧表重复实现。上述核验是当前源码/回执审计，不是重新联网确认厂商现行协议。P50/P60/P70 继续，不新增模板或整卡 offline_ready。

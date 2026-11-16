# DIM 授权会话失效恢复

## 本批解决的问题

DIM 后端 registry 回收登录会话后，通过 `dim_login_rejected` code 和 `dim_login_unavailable` message 明确返回不可用。DIM 的 `useDimOAuth` 之前没有接入共享 hook 已有的 unavailable 分类，因而未保存会话仍可能停在 active 状态，登录按钮和注册字段继续被锁定；已提交保存的回执丢失也仅按一般读取失败处理。

本批只给 DIM policy 接上精确 code/message 分类，复用 `useManagedOAuth` 既有行为：

- 未提交保存：释放失效会话，清除待续接操作，提示明确重新授权；不自动创建登录。
- 已尝试保存且结果未确认：停止自动轮询，保留非秘密回执定位和 duplicate-save fence；关闭/重开面板也不允许重复 create/import。
- 容量不足、版本冲突等非精确匹配错误不释放有效会话。

正常 DIM 面板沿现有状态显示提示、解锁或保留限制，不增加旁路按钮，不改变两站隔离、注册 client、Grant、revision/CAS 或明确保存合同。

## 修改与验证

生产文件仅 `apps/desktop/src/features/console/useDimOAuth.ts`：9→11 有效行。测试为 `useDimOAuth.test.tsx`：59→101，`DimOAuthPanel.test.tsx`：67 有效行；没有修改共享 hook、API、Rust、依赖或有效行基线。

新增 6 个用例：hook 的未保存恢复、未知保存回执重开防重、两种非匹配错误，以及正常面板的重新登录解锁/未知保存保持锁定。

- 红测：DIM 两文件 14 项，4 failed / 10 passed；失败对应两条缺失恢复路径及面板表现。
- 修复后：DIM hook、面板、API、共享 unavailable 和 Accio/Kiro/Grok 生命周期，共 7 文件 **70 passed / 0 failed**。
- `npm run typecheck` 通过。
- `npm run test:effective-lines --prefix scripts`：31 passed；ratchet 通过。
- 包含未跟踪文件的独立 whitespace 检查通过；没有引入新 formatter 或依赖。

日志位于 `C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-dim-session-recovery-20261112`。本批为前端定向修复，不以旧后端测试冒充本轮运行；未重复 Rust 编译或正式发布检查。

## 安全和完成边界

错误分类同时匹配类型、固定 code 与 message，不能通过普通网络错误释放防重保护；不读取或保存任何 callback/token，不自动重试变更请求，不改变 accepted save 的所有权。浏览器组件测试使用模拟 API，不是真实 DIM 登录验收。

没有真实平台调用、账号注册、凭据读取、数据改写、运行实例变更、打包、提交或推送。旧手测包不包含此修复。Suno MCP 账号资格阻碍保持不变；Perplexity 多轮尚缺续接 wire 与私有状态所有权合同，未通过删除单轮限制伪装完成。本批不改变模板数量、整卡 offline_ready 或总计划完成状态。

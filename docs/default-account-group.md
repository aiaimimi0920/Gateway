# 默认账号分组

`default` 是未选择其他分组的账号的默认分组。管理界面的账号目录、分组筛选、访问 Key 的分组授权和分组计费使用相同的有效成员集合。

- 所有未加入其他分组的显式账号以及服务商继承账号自动归入 `default`，包括停用账号；停用账号不会因此重新参与调度。
- 新增账号不必手动选择分组。选择其他分组后退出默认成员集合；移除全部其他分组后自动回到 `default`。
- 已选择停用分组也算有明确分组，不会自动转入启用的 `default` 绕过限制。
- 默认分组首次出现时启用、计费倍率为 1。既有 `default` 的名称、启停和倍率设置保持不变，成员按上述规则计算。
- 没有指定分组的调用仍保持原来的授权语义，不因这一改动自动改成只能访问 `default`。访问 Key 的显式分组限制保持有效。
- 兼容旧配置时通过不可变快照派生成员，不重写历史修订、摘要或密钥，也不要求用户手工迁移数据库。默认成员关系不依赖旧配置中是否逐项写入 `default.provider_credential_ids`。

验证入口：`cargo test --locked --test suite_provider_contracts default_account_group_contract`，以及桌面端 `defaultAccountGroup.test.ts`。

## 管理端分组汇总

`GET /v1/internal/gateway/account-groups` 使用管理端认证，读取当前路由快照。响应的 `summary` 包含路由修订、配置来源、`accountGroups`（含有效倍率和成员关系）、`accounts` 和 `providers`；会脱敏返回的上游 URL。这是配置视图，不是实际费用或请求审计。分组授权和计费规则见[本地访问密钥](local-access-keys.md)。

# 凭据池认证资格约束

`credential_pool_auth_policy` 是 route-config 中单个 provider 的业务凭据池约束。适用于希望同一池内只使用具备 OAuth refresh 配置的账号，而不因轮询或模型映射选到普通 API key 的场景。它不是全局 OAuth-first，也不证明账号、token 或上游响应的真实性。

## 配置与兼容

可选值为 `any`（默认）和 `oauth_only`。省略或显式配置 `any` 均保持旧路由行为；规范化序列化时省略 `any`。未知值拒绝反序列化，不静默退回默认值。

以下只使用合成地址与凭据，展示字段所在层级，不可用作实际连接配置：

```yaml
providers:
  - id: example-pool
    adapter: openai_compatible
    base_url: https://example.invalid/v1
    supported_models: [example-model]
    credential_pool_auth_policy: oauth_only
    credentials:
      - id: example-api
        api_key: synthetic-api-key
        supported_models: [example-model]
      - id: example-oauth
        api_key: synthetic-access-token
        refresh_token: synthetic-refresh-token
        refresh_endpoint: https://example.invalid/token
        supported_models: [example-model]
```

该字段经过现有 typed route document、编译快照与 provider fingerprint owner。不新增数据库字段、storage version、迁移、后台任务或独立状态库。修改 policy 会使该 provider 的编译缓存失效；与该 provider 无关的配置变更仍可复用其 refresh owner。

本批没有新增专用 UI 开关。可通过现有 route document 编辑能力配置；管理 API 写入仍必须遵守原有鉴权、expected revision 与 secret keep/patch 合同，不能为修改普通字段而覆盖凭据。

## `oauth_only` 的精确定义

业务选择要求凭据具有当前进程创建且仍 active 的 refresh owner，并且 refresh token 与 refresh endpoint 都包含非空白内容。检查只读取，不复制、改写或记录秘密。它是在现有配置中的保守资格判断，不会发起 OAuth 鉴真请求。

- 不根据 token 字符形状、`bearer`、`session_auth`、protocol profile、套餐或价格推断 OAuth。
- 缺少任一 refresh 字段、空白值、无 typed owner 的 provider-level 默认账号、尚待 discovery 的凭据均不合格。
- 原有 enabled、账号组授权、模型支持等资格仍有效；策略不扩大权限、不启用账号、不恢复健康状态。
- 普通 round-robin、`model_map`、`model_map_targets` 的业务选择均应用约束；先排除不合格凭据再在剩余集合内轮询或选择映射目标。
- 没有合格账号时不退回该池的 API key、base payload 或 unsupported-model legacy fallback。模型列表也不将该 provider 中不合格的目标当作可用目标。
- 显式管理 probe 仍测试指定 credential，包括被业务池策略排除的 API key；它不会为了满足本策略更换为其他账号。这不等于管理 probe 已变成普通业务路由。

## 不涵盖的范围

该策略仅约束对应的 route-config provider pool，不约束数据库、Redis 或 access-catalog 提供的独立候选，也不阻止其他 provider 按既有路由规则接手请求。若整个请求都必须禁用 API key，不能只给其中一个 provider 设置本字段并假定已获得全局保证。

本批不提供“优先 OAuth、失败后允许 API key”的 `prefer` 模式，不调整跨 provider fallback、错误预算、健康筛选、pin 或重试许可。此类能力需要统一的可信候选类型与明确回退合同后再实施。

refresh owner retirement 会让旧快照的后续严格选择失败关闭；检查与实际发送不是一个原子事务，已经选出并克隆的 payload 不会被这一资格检查撤回。因此这不是 send-time revocation、跨实例吊销或供应商可用性保证。`any` 模式原有的 refresh 配置处理不在本批顺手修改。

## 验证入口

聚焦合同位于 `src/routing/config/pool_auth_tests.rs`，覆盖默认兼容、严格池轮询、模型映射、catalog、精确 probe、授权、缺项/空白材料、编译复用及 owner retirement。可在仓库根目录执行：

```powershell
$env:GATEWAY_PREBUILT_WEB_UI = '1'
Remove-Item Env:RUST_MIN_STACK -ErrorAction SilentlyContinue
rtk proxy cargo test --locked --offline --lib routing::config::pool_auth_tests -- --test-threads=1
```

`GATEWAY_PREBUILT_WEB_UI` 仅复用已存在的 Web 构建标记，不代替生产构建验证。真实供应商、数据库/Redis 独立候选、打包与运行验收的边界见本批进度回执；合成单元测试不计这些范围通过。

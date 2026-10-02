# Gateway SQLx 依赖边界

此内部 path library 只重导出官方 SQLx 0.8.6 的 core、PostgreSQL、SQLite 类型和
官方 `FromRow` derive，根 Gateway 使用 `sqlx` dependency alias 消费它。没有自制
derive、ORM、查询/解码实现、vendored fork，也不改变 SQL、schema 或数据库数据。

## 为什么不继续依赖 umbrella

SQLx umbrella 的 PostgreSQL/SQLite、JSON/time feature 转发会将未使用的 MySQL
和 RSA 纳入当前 Cargo 锁图。只关 default features 不足以排除它们。
直接组合官方 runtime crates，并仅给 `sqlx-macros` 启用 `derive`，经隔离官方
resolution 和实际编译验证，保留原 `FromRow` 实现而不引入 MySQL/RSA。

根 `Cargo.lock` 是此 crate 的构建和发布依赖 owner；它不是独立发布目标，不额外
建立独立 lock。从根目录运行 `cargo check --locked --all-targets` 和
`cargo test --locked --test sqlx_runtime_contract`。桌面 launcher 与
`gateway-local-data` 保持各自的 manifest/lock owner，不在此依赖图中。

## 保持的能力与升级边界

- Tokio runtime、rustls ring + webpki roots、PostgreSQL/SQLite 的 JSON/time；
- bundled SQLite；现有 runtime owner 继续配置 WAL 和 Full synchronous；
- 所有重导出均是官方原类型，`FromRow` trait 与宏保持同名不同 namespace；
- 普通 query/query_as/query_scalar/raw_sql、pool、Row、Transaction、QueryBuilder；
- 当前模型的官方 named mapping、泛型 trait bounds 和原始解码错误；
- 额外对照覆盖了 rename/default 属性、raw identifier 与 serde rename 不干涉列名。

不提供 MySQL、Any、query 系列编译期宏、迁移/test 宏或 `Type` derive。当前 Gateway
没有这些调用；新增需求应在此 owner 明确评审，不能悄悄重新引入 umbrella。
这不是整个 umbrella API 的等价替代。官方 `FromRow` 的 `try_from` 属性会引用
umbrella 的 `__spec_error!` helper，此处没有提供；当前模型无该属性。未测或依赖
额外 umbrella helper 的其他属性不作支持承诺，新增调用必须单独编译和验证。

四个官方 crate 必须一起评估、保持同一精确版本；`sqlx-core` 的内部 runtime/TLS
feature 不是稳定公共配置接口，升级时必须重新核对上游。不要通过打开宏的驱动或
JSON/time feature 来配置 runtime；这些开关属于两个 driver/core 的依赖声明。
Dependabot 已覆盖此 manifest；版本更新必须同时通过锁图、编译、SQLite 运行及
官方 umbrella 对照，不能仅凭当前行映射测试声称整个数据库产品验收通过。
`tests/fixtures/sqlx_error_snapshot.json` 保存本次独立官方对照的解码结果/错误快照；
正常根测试也会比较 kind、column/index、source 和 display，不能用候选自身的输出
直接刷新期望来使回归通过。

回滚不需要数据迁移：恢复根 `Cargo.toml` 与对应 `Cargo.lock` 的原 SQLx umbrella
依赖即可。正式升级或回滚都使用新的构建/发布证据，不修改已有发布包。

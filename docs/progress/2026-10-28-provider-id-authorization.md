# 服务商 ID 在线迁移：本地授权兼容阶段

承接 `2026-10-28-online-provider-id-migration.md` 的 SQLite 健康状态兼容层。
当前 4200 不停止、不 drain、不重启、不替换 EXE，不修改运行配置或真实数据库。
后续等待授权来源刷新见 `2026-10-28-provider-id-wait-authorization.md`；本文保留本阶段验收记录。

## 已接入的源码

- `src/provider_identity.rs`：只根据显式的一对一别名建立权限等价关系。双向扩展、
  去重并保留输入顺序；不会根据 preset、品牌、URL、协议或大小写推断身份。
- `src/local_runtime/provider_identity.rs`：在读取 key 的同一个 SQLite 事务中读取
  映射快照。最多取 257 条，超过 256 条拒绝，不能静默截断权限注册表。
- `access_keys/policy.rs`：模型发现和请求候选授权使用相同的 provider 限制扩展，
  返回给 fallback 的显式授权集合也包含同主体的新旧 ID。只过滤原候选，不创建
  候选、不复制账号、不重新排序服务商，不改写 key 的 metadata、hash 或 sealed secret。
- `None` 仍表示无 provider 限制，空数组仍表示不允许任何 provider；只扩展原来
  获授权 ID 的对应成员，不追加整张别名表。输入最多 128 项，扩展最多 256 个 ID。
- 拒绝自指、链、环、多对一合并、重复源及无效 ID。非空 provider 限制遇到畸形
  注册表时拒绝授权；无限制及空限制不依赖该表，保留原权限语义。

## 权限与激活边界

现有 `model_provider_aliases` 表现在也承载本地 key 的身份等价授权，而不再只是
健康状态实现细节。已更新迁移原语注释：未来协调器必须证明 old/new 是同一主体，
保留旧 ID 不被重新分配，并禁止两者作为独立服务商同时参与调度。

当前仍然没有 HTTP 迁移入口、启动自动迁移或配置保存触发器；生产调用方尚未调用
内部 rename 原语。本阶段不会令当前已保存服务商发生改名。

## 测试范围

- 纯别名规则：双向扩展、无关主体隔离、大小写、空列表、去重、非法图及容量边界。
- SQLite 集成：old-key/new-route、new-key/old-route，模型发现与候选/返回 fallback
  集合一致；加入另一条无关映射仍不能获得对应权限；两候选只保留一候选。
- 校验 key payload、token hash、sealed secret 在查询、授权和重开后保持不变。
- 错误模型、空 provider 列表、无关 provider、撤销继续拒绝；目录账号组交集、
  admission callback 继续生效；畸形映射拒绝非空限制 key 的目录和请求授权。
- 所有数据为 `linshi` 下的合成测试数据库，URL 只是测试配置，不调用上游。

## 仍需后续接齐

1. 映射快照与路由配置热发布尚未统一协调，不可据此宣称在线切换已完成。
2. 恢复等待路径会重新鉴权，但当前忽略返回的 fallback IDs；在等待过程中发布
   新映射时，旧上下文可能保守拒绝跨 provider 回退。需和候选身份刷新一起处理，
   不能简单追加所有新权限或复活已被其他约束过滤的候选。
3. 本轮测试不等于真实发送阶段的跨 provider 重试或长流式验收；账号组请求路径
   仍由现有 pipeline 负责，本轮新增账号组测试覆盖的是目录交集。
4. 审计、价格、压力统计、refill 引用，协调器恢复日志与部署/降级门禁仍待完成。

## 本轮验证结果

- 纯别名规则先红后绿，最终独立运行和 Cargo 运行均为 4/4 通过。
- 首轮本地回归 36/37：新测试误把静态路由的账号 ID 当成数据库账号字段。
  根据 `routing/config/candidates.rs:469-485` 修正为同时断言
  `provider_credential_id == None` 和 `payload.credential_id == account-one`，
  未为迎合测试改动生产候选生成逻辑。
- 对齐了迁移写入端与权限读取端的控制字符校验，并补充拒绝用例；最终最新源码
  `cargo test --locked --offline --lib local_runtime:: --jobs 2 -- --test-threads=2`
  37/37 通过。
- `provider_identity::tests` 4/4、`routing::config::admission::tests` 2/2、
  `routing::config::candidates::fallback_tests` 2/2、`routing::model_circuit::` 33/33
  通过。以上去重合计 78 项 Rust 测试通过。
- checker 测试 31/31、有效行数 ratchet、修改文件 rustfmt、git diff --check 通过。
- 新的纯规则模块 52 有效行、SQLite 读取模块 24 行、纯规则测试 61 行；
  授权文件 248→254 行，职责仍为本地 key 策略；新增集成测试 187 行。
- 编译存在工作区原有 unused 等 warnings，没有执行 cargo fix 或升级依赖。
- 独立只读复核确认权限集合只扩展到原来授权的主体、同事务读取及候选不复制；
  并指出了上文保留的等待中 fallback 刷新与完整发送验收边界。
- 在线原实例只做健康检查：healthz/readyz 均为 HTTP 200，4200 后端 PID 42912、
  LAN 转发 PID 4280 未因本任务改变。

当前不登记实际 ID 迁移完成，不交付新 EXE。

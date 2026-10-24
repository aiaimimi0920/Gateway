# PostgreSQL 公开模型目录的硬准入一致性

接续 [多方案与管理 UI](2026-10-23-site-health-profiles-phase2.md)。

## 已实现

公开 `GET /v1/models` 的 PostgreSQL access-key 与 project 两条路径已传入只读
`subject_allowed(provider, credential, actual_upstream_model)`，而不是用全局同名 YAML 模型代替授权依据。

- access-key 路径继续验证 access key 并从原授权 projection 派生；不扩大 bundle 权限，
  不将短期健康结果写回 Redis projection。实时检查 provider active/cooldown 与可用具体凭据。
- project 路径保留 active project 和 route policy，alias/capability/default 都保留 provider 归属，
  先检查 provider/model policy，再检查至少一个具体凭据支持该模型且通过硬准入。
- 健康判定使用实际上游模型，不拿公开别名作为熔断键。多凭据任一个可用即可保留；
  全部禁用/归档的池不能退回 provider-default；真正没有凭据的老账号使用 `None` 身份。
- 过滤有效 row 后再计算 provider-scoped 的 alias/native shadowing，
  不健康或未授权的别名不会继续遮蔽健康模型，不借另一个 provider 的同名模型挽救被拒 row。
- 目录读取不做健康 HTTP 探测、试放占位、随机模型选择或 round-robin 消耗。
  同名模型保留的依据必须是本次权限范围内至少一条可用 witness。

纯选择算法在 `src/db/routing/catalog_selection.rs`；凭据 witness 在
`src/db/model_catalog_admission.rs`。原路由测试的重复 collector 改为调用真实生产 collector，
避免测试副本通过但生产分支未被覆盖。

## 实际验证

新增 15 项离线测试首轮全部通过。收窄 project 的凭据读取范围到策略允许的 provider 后，
已冻结源码并重新编译、扩大相邻库回归：**43 项通过、0 失败，3 项真实 PostgreSQL/Redis 集成测试按原有声明跳过**。
其中包含新增 15 项、原授权/目录/模型策略合同、只读目录/人工禁用恢复合同和 11 项熔断状态机测试。

| 检查 | 实际结果 |
| --- | --- |
| 相关 Rust library 测试目标编译及定向回归 | 43 通过，3 个外部服务测试跳过 |
| 本阶段 13 个 Rust 文件的 Rustfmt check | 通过 |
| effective-line checker 自测 | 31/31 |
| effective-line ratchet | 通过，受治理源码 >700 为 0 |
| `git diff --check` | 通过 |

证据为 `linshi/gateway-health-v2-20261023/catalog-checks.json`、`catalog-rust.log`、
`catalog-rustfmt.log`、`catalog-checker-tests.log`、`catalog-ratchet.log` 和 `catalog-lines.json`。
未为这次目录改动重跑所有上游/发布矩阵；相邻健康/发送合同的先前 138 项成绩见第二阶段，
不与本次重叠测试简单累加成独立覆盖总数。编译保留了其他既有模块的 unused 等 warning，没有因此修改无关代码。

当前有效行：credential witness 51、测试 79；project 选择器 60、测试 148；
project catalog 117、access projection 336、access 新测试 58、HTTP 入口 71。
本阶段无新增或修改文件超过 500；未新增行数豁免，最终 ratchet 已通过。

## 安全与性能复核

不修改数据库 schema、存储版本或秘密格式；不写人工禁用开关。SQL 使用绑定参数。
健康闭包只检查内存状态，列表查询批量加载凭据，不为每个模型新增 SQL/HTTP。
只为本次请求保留临时集合，没有新增全局秘密缓存、后台任务或进程。
既有 active-credential reader 的过期 cooling 恢复行为保留，不恢复人工 disabled 状态。

尚未连接真实 PostgreSQL 服务执行数据库集成验收，也没有真实模型/付费上游调用。
没有改变运行中的网关、live `.ng`、凭据或数据库，没有提交/推送/部署或生成新 EXE。

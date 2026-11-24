# Command Code Server 内部模型探测审计

范围是 [上轮缺失的探测审计](2026-11-21-commandcode-server-scheduler.md)；非生产迁移、正式 Release 或真实供应商验收。未改 Luma、现有运行实例、账号或余额。

## 改动

- `007-gateway-internal-model-probe-audits.sql` 新增独立内部审计表和按部署／产品／Provider 查询的索引，纳入 standalone 发布载荷。现场升级必须由 schema owner 评审后先应用 SQL 再升级二进制；回退可保留表。
- Command Code Server 显式及自动文本探测先写 `running`，才允许向上游发送；沿用有界取消安全的审计生命周期。完成、失败和取消仅能从 `running` 进入终态；终态持久化失败不报告成功、不重放。内部探测不再依赖虚构的公共 Key，也不进入公共请求审计或用户计费。
- 仅记录必要的部署、Provider、实际凭据标识、模型、状态、时长和已观察用量；不写凭据秘密、请求／响应正文或上游错误原文。参见 [审计合同](../providers/commandcode-server-probe-audit-contract.md)。

## 实测回执

- 仅在隔离 PostgreSQL `gateway_fixture` 中事务性应用 `007` 两次，保留其既有 17 条公共审计、11 条 Provider 及三条公共审计约束。另建隔离 `gateway_fixture_unmigrated`，只执行 `001` 至 `006`；SQL 确认公共审计表存在、内部审计表不存在。未删表，也未清理测试数据。
- 编译退出 0 后直接运行本次测试 EXE。真实 PostgreSQL／Redis 联合回归 **4 passed / 0 failed**（121.55 秒）：两个生产调度循环在真实 60 秒间隔内合计发送两次，各有一条 `completed` 内部审计、精确凭据／模型和 2/3/5 tokens；零公共审计。第二 AppState 同时读到 Redis 结果，重建后的 AppState 保留结果、活跃调度槽和两条审计。普通 Hosted Messages 对照、失败与取消终态、关闭单个测试池前置零发送均通过。
- 同一测试 EXE：Command Code **82 passed / 0 failed**（4 条 Server ignored 单独执行），共享探测审计生命周期 **7 passed / 0 failed**。checker **31/31**，ratchet、定向 `rustfmt`、`git diff --check` 与打包布局合同 **1/1** 通过。布局夹具通过不代表打出了新 EXE。
- 补充编译退出 0，新增三项对照各 **1 passed / 0 failed**：真实未迁移库零发送；接到合成上游回复后关闭专用池，终态失败不暴露答案、不报告成功、不重放，第二 AppState 仍读到原 `running`；公共写入依然明确拒绝无调用者身份。测试后只停止本批启动的 PostgreSQL／Redis 夹具，保留两套库和所有数据。
- Command Code 独立 feature 的 `cargo check --locked --offline --lib --bins --no-default-features --features line-commandcode-provider-api -j 2` 退出 0。
- 当前桌面回归首轮 **22 passed / 1 failed**：旧 Command Code 目录用例把同会话 grant 字符串续期当成权限撤销。对照共享 hook、服务端回读 grant 与现有文档后，只调整测试，不改生产或放松鉴权：确实撤销权限仍丢弃迟到目录；同会话续期且原 grant 仍有效、服务端成功返回时应用一次，不重发查询。最终 Command Code 与共享发现、续期 UI 回归 **36/36**，TypeScript `typecheck` 通过。未宣称原 grant 已过期时官方目录仍返回成功。
- 桌面 devtest 首次尝试与测试编译同时占用内存，已主动中止这次打包的 `rustc`；没有交付该次构建，现切换为独立构建。

## 桌面手测包

- 独立重试打包退出 0；`desktop / devtest`，非正式 Release。交付目录：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/release/Gateway/gateway-devtest-20261122-commandcode-server-audit`，入口 `gateway-ui.exe`。包内包含 `007`，两个 EXE 的大小、SHA-256 与清单一致。
- `gateway.exe`：`16f3567914a6365d83819befa7b5bd367fbdf1f1e74cdc4c74e1799ba1631128`。
- `gateway-ui.exe`：`aeb1230c89f08bac165a5d622371db701605b0ce0ef88820adce816de232c855`。
- 隔离启动退出 0：SQLite 就绪、管理页 HTTP 200、零供应商调用、无本次启动的遗留进程。未执行原生业务表单点击、包内双 EXE 探测成功路径、实际部署或真实账号测试。

一次性原始回执位于 `C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-commandcode-audit-20261122`。长期入口见下文，不以临时脚本作为依赖。

## 复跑

在 Gateway 根目录设置 `COMMANDCODE_TEST_DATABASE_URL` 指向隔离 loopback `gateway_fixture`（`001` 至 `007`），`COMMANDCODE_TEST_REDIS_URL` 指向隔离 loopback Redis，`COMMANDCODE_TEST_EVIDENCE_ROOT` 指向 `linshi` 下新的证据目录。数据库禁止使用 5432，测试会写入唯一作用域的合成数据。

```powershell
cargo test --locked --offline --lib --no-default-features --features runtime-defaults --config profile.test.package.gateway.debug=0 -j 2 commandcode_server_ -- --ignored --test-threads=1 --nocapture --skip commandcode_server_probe_audit_missing_migration_stops_before_send
```

未迁移负对照另将 `COMMANDCODE_TEST_DATABASE_URL` 指向隔离 loopback `gateway_fixture_unmigrated`（仅 `001` 至 `006`），同一 Cargo 参数将测试名替换为 `commandcode_server_probe_audit_missing_migration_stops_before_send`，并去掉 `--skip` 及其值。不能通过删迁移后的表制造负对照。

两个 AppState 不是两个独立 EXE；本次合成成功也不等于真实账号权限、上游计费或实际部署验收。

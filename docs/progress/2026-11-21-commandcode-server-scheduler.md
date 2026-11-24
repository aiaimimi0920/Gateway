# Command Code Server 自动探测与审计复核

范围：接续 [首批收口工作板](../providers/first-three-closure-2026-11-13.md)，只复核 Command Code 的非账号 Server 缺口。未改生产代码、真实账号、余额、部署、现有实例或 Luma。

## 实施与实测

- 用修改前的现存测试 EXE `neuro_gateway-f4464fa8f2c318a8.exe` 复现 `overflowed its stack`，退出 `-1073741571`。仅调整 [测试夹具](../../src/protocol/commandcode/wire_tests/server_scheduler.rs)：使用项目已有做法的 16 MiB 测试线程栈、堆上固定 worker future，保留两个生产调度循环、真实 60 秒周期和 Redis 原子槽。按两次实际结果发布等待，不再用固定延迟推断第二轮已完成。
- 新源码按下文命令构建成功，Cargo 退出 0（10m12s），随后直接运行该次构建的测试 EXE。隔离 PostgreSQL/Redis 联合回归 **1 passed / 1 failed**，退出 101，执行 61.14 秒。旧脚本首次提取 Cargo 路径时未适配 Windows 的 `src\lib.rs`，该次包装启动失败；直接执行的结果为准。
- **周期及恢复断言通过**：同一 Redis namespace 下，两轮总计两次上游模拟发送，两次正确评估发布；不同 Server AppState 均读到最后结果，关闭重建后结果和未到期槽仍保留。此处不是两个独立 EXE 的业务验收。
- **自动探测 PostgreSQL 审计失败**：最终断言预期 2 条，实际为 0。只读 SQL 对本次唯一账号再次核对，账号存在、总审计行仍为 0。同批普通 Hosted Messages 对照通过：2 次原生发送、2 条完成审计各 5 tokens、Redis 槽仅一个胜者。
- 同一新 EXE 的 `protocol::commandcode::` 默认定向测试 **82 passed / 0 failed / 2 ignored**（35.51 秒）；两条 ignored Server 测试已在上文单独执行，不计入 82 项。有效行 139 → 179；checker 31 项、ratchet、定向格式及 diff 检查通过。

## 当时缺口：Server 探测审计主体

2026-11-22 已实现 `007` 和独立内部审计，新的联合回归及未验边界见 [Server 探测审计回执](2026-11-22-commandcode-server-probe-audit.md)。下文保留原失败原因。

[探测录入](../../src/provider_runtime/model_probe_recording.rs) 使用 `console-model-probe`，没有公共请求的 `api_key_id`、`user_credential_id` 或 `access_key_id`。[PostgreSQL 公共请求审计 writer](../../src/db/request_audits/persistence.rs) 在 SQL 写入前就拒绝三者为空；隔离库还验证存在正式身份检查及 project/provider 外键。此结果与 [共享 probe 的既有边界](2026-11-03-provider-channel-expansion-p19.md#21-postgresql-审计缺口明确保留) 一致，不是额度、Redis 或上游模型失败，不重新跑真实账号。

修复前先评审 Server 内部探测审计的主体、归属和读取权限。优先考虑独立的内部探测记录，不伪造调用者，不放宽公共审计约束，不把内部用量计入用户账单；复用既有有界审计生命周期，保留成功、失败和取消的终态，并让非终态不会假装成功。需要持久化 schema 变更时按项目要求走独立迁移评审。保留当前联合用例的失败断言，后续修复应提供正式探测审计的读回，同时复跑公共请求正常审计与无主体拒绝。Command Code 整项仍未到仅待真实账号验收。

## 当时的重跑入口

当前版本引入了迁移前后的独立负对照，重跑请改用 [新说明](2026-11-22-commandcode-server-probe-audit.md#复跑)。历史执行环境如下。

在 Gateway 根目录，设置 `COMMANDCODE_TEST_DATABASE_URL`（隔离 loopback `gateway_fixture`，不可用 5432）、`COMMANDCODE_TEST_REDIS_URL`（隔离 loopback 实例）和 `COMMANDCODE_TEST_EVIDENCE_ROOT`（`linshi` 下本次目录）。保留真实身份及 provider 外键，所有运行使用唯一 namespace，不删除既有数据。执行：

```powershell
cargo test --locked --offline --lib --no-default-features --features runtime-defaults --config profile.test.package.gateway.debug=0 -j 2 commandcode_server_ -- --ignored --test-threads=1 --nocapture
```

本次可丢弃日志位于 `C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-commandcode-next-20261121`；复跑不依赖该目录中旧日志或脚本。

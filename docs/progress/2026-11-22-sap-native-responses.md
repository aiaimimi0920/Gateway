# SAP AI Core：原生无状态 Responses

## 实现与范围

- 在已确认支持 Responses 的现有 SAP 部署上，可显式启用 `sap_native_responses`。只有公共 `/v1/responses` 改走上游原生 `/responses`；未勾选的旧池及 Chat／Messages 不变。
- 新路径复用 SAP 的短期 OAuth Token、资源组、部署身份和不重放机制。线协议显式发送 `store:false`，不发送 Chat 日期版本、公开模型别名或部署 ID。支持文本、函数工具与结果及无状态加密推理历史；远端会话／引用、后台任务、文件／媒体和内置工具不在本切片内。
- 公共 JSON 与 SSE 保留上游扩展，把模型字段映射回调用者可见别名；验证终态，并仅从合法终态提取用量。Hosted 凭据不能启用、取消或把声明转移到其他部署；控制台修改绑定会清除勾选。详见 [配置合同](../providers/sap-aicore-chat-contract-2026-11-10.md#显式开启原生无状态-responses)。

## 验证

- 先执行了公共 Rust 红测，原配置不识别 `sap_native_responses`；控制台红测缺少复选框。实施后控制台 SAP 定向 **10/10**，TypeScript 类型检查通过。
- 新增 Hosted 能力归属的测试最初使用闭包别名绑定了同一组借用生命周期，导致 E0502／E0597／E0716，已改为函数别名，不改生产合并行为。首次 ratchet 报 `stage_send.rs` 有 501 有效行，已合并相邻标准库导入，复跑通过。
- 两次可运行的 SAP 定向回归均为 51 passed／1 failed。原生 JSON／SSE 两回合、畸形／截断、取消退款及 Local／Hosted 部署绑定均通过；负向用例在等待审计和请求结束时超时。临时 SQLite 已有一条失败审计。检查发现测试把读取错误响应体放在断言的失败分支中，正常拒绝路径反而没有读取；已改为先读完响应，再逐项检查拒绝、零 OAuth、零生成及预付次数不变。`store:true` 仍要求记录失败，现有审计不能有完成项。
- Hosted 新增用例 **1/1**：写入与读取使用两个独立 AppState，共用隔离 PostgreSQL／Redis。仅在凭据设置开关的账号零 OAuth／零生成；账号层显式启用后 JSON／SSE 各一次，共一次 Token 交换。PostgreSQL 读回两条归属账号与凭据的完成审计，各 5 tokens、各一次路由。专用容器已停止，数据保留。
- 共享回归：SenseNova **32**、Command Code **104**、Responses **60**、send pipeline **124**、底层 stream **53** 项通过；各产品需要专用环境的忽略项未在共享回归中运行。独立 `line-sap-aicore-chat` lib／bins 已通过。
- 定向 Rust 格式、有效行 checker **31/31**、ratchet、manifest **55/55**、UTF-8 无 BOM、`git diff --check` 通过。修正后 SAP 定向回归 **52 passed／0 failed**，3 项隔离 Hosted 用例默认忽略；本轮新增 Hosted 用例已单独通过。本阶段未访问真实 SAP 租户、消耗额度或修改现有实例。

## 复跑

常规回归只使用内存／临时 SQLite 和不转发的 loopback SAP 夹具。Hosted 复用 [专用数据库与 Redis 说明](2026-11-22-sap-hosted-x509.md#复跑)，需要 `SAP_AICORE_TEST_DATABASE_URL` 和 `SAP_AICORE_TEST_REDIS_URL`。测试创建唯一数据作用域，数据库与 Redis 不使用默认宿主端口，不清除既有数据。

```powershell
cargo test --locked --offline --lib --no-default-features --features runtime-defaults --config profile.test.package.gateway.debug=0 -j 2 sap_aicore -- --test-threads=1
cargo test --locked --offline --lib --no-default-features --features runtime-defaults --config profile.test.package.gateway.debug=0 -j 2 sap_aicore_server_hosted_native_responses -- --ignored --test-threads=1
cargo check --locked --offline --lib --bins --no-default-features --features line-sap-aicore-chat -j 2
npm.cmd run test --prefix apps/desktop -- --run src/features/console/SapAiCoreCatalogTemplate.test.tsx
npm.cmd run typecheck --prefix apps/desktop
```

## 桌面手测包

- `desktop / devtest`，不是正式 Release。包目录 `C:/Users/Public/nas_home/AI/GameEditor/Neuro/release/Gateway/gateway-devtest-20261122-sap-native-responses-v1`。来源指纹 `35688e555a29054a381aa787c47bdc270857d9b6b1e4876714f4db49c6581601`（写入本节前的工作树），共 6159 个清单载荷；未包含 `.ng`。
- staging 与交付目录的完整性检查均通过。`gateway.exe` SHA-256：`2ab39cd15e79ceb39253b3e67fc3f2c1dce875f6524db6ee805343483e3ec664`；`gateway-ui.exe` SHA-256：`b312c449170da00b5dee9008f597d82bb41fdb144266e6eb3dc1a4fb086d8428`。
- 用包内后端在随机 loopback 端口验证了隔离 SQLite 就绪、耗尽拒绝、重启读回、路由失败退款、Key 轮换及正常 drain；零 PostgreSQL／Redis 连接，未作上游生成。未验证 SAP 账号的原生桌面提交／重启读回、打包双后端成功推理或真实租户。既有实例及旧包未操作。
- 复跑使用 `tools/package-gateway-devtest.ps1 -Edition desktop -Jobs 2` 的新版本目录及 `tools/smoke-gateway-local-storage.ps1`，详见[手测打包规则](../devtest-build.md)。本轮首个 Windows PowerShell 子进程未发现 `Get-FileHash`；在 PowerShell 7 显式加载 `Microsoft.PowerShell.Utility` 后发布、校验及隔离运行均通过，未修改打包脚本。

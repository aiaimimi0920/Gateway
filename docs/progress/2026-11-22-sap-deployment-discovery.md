# SAP AI Core：只读部署发现与显式选择

- 管理入口要求控制台账号与有效敏感信息授权。合成回归通过真实 Gateway router 连接只监听 loopback 的 SAP 模拟端，核对 OAuth Basic、Bearer、资源组和分页参数。未授权或无效输入不外发；发现不修改路由 revision，不新增账号，不调用推理。返回列表经过 Chat 模型、运行状态、方案、可执行单元及同源部署 ID 校验。
- SAP 回归 **28/28**，含新发现入口的重定向、超时、错误脱敏、大小与页数限制，以及既有推理 Token 缓存与 Keep 绑定。首次线级回归发现当前 HTTP 客户端连续两次设置 query 会覆盖前一组，现已在一次请求构造中发送完整参数。
- 桌面定向回归 **33/33**，TypeScript typecheck、Web 资源构建、定向 Rust 格式、有效行 checker **31/31**、ratchet 及 `git diff --check` 通过。界面等待操作者显式选择，资源组切换取消待完成查询，旧结果不回填。查询用的密钥不进配置；另行保存账号仍使用现有敏感信息补丁和 CAS。
- 不开通 SAP 租户、不导入真实 Service Key、不调用实际 SAP 列表或推理。未以目录通过替代原生客户端、Server Hosted、真实推理或计费验收。

## 复跑

在 Gateway 根目录执行：

```powershell
cargo test --locked --offline --lib --no-default-features --features runtime-defaults --config profile.test.package.gateway.debug=0 -j 2 sap_aicore -- --test-threads=1
cargo check --locked --offline --lib --no-default-features --features line-sap-aicore-chat -j 2
npm run test --prefix apps/desktop -- --run src/features/console/SapAiCoreCatalogTemplate.test.tsx src/features/console/useCredentialDialogEditor.sap.test.tsx src/features/console/ProviderCatalogDialog.test.tsx src/features/console/ConsoleGlobalDialogs.luma-agents.test.tsx
npm run typecheck --prefix apps/desktop
```

## 桌面 devtest 与本地运行

- SAP 独立 feature 编译通过。桌面手测包（非正式 Release）：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/release/Gateway/gateway-devtest-20261122-sap-deployment-discovery-v1`。构建时源码指纹为 `42b90f823b349a73afc88282047b436d2015dce448cdb932d2bfcf1d9f5ed285`；记录的是构建时工作树，之后仅补本段验收回执。
- 完整清单与双 EXE 哈希已校验。`gateway.exe`：`d6dd1f176921e086e1a04bcbfad18bd8ed5f9fedc646db751f4bb15997a642ab`；`gateway-ui.exe`：`4f0f80cced9ff94b932c61a352e7cf60893484e9243883e97b44a432173b2c14`。
- 包内后端在隔离 SQLite 通过启动、重启与正常 drain，并验证 SAP 入口缺敏感信息授权返回 `403`；授予测试会话权限后，畸形 Key 在本地返回 `400 invalid_sap_aicore`、`Cache-Control: no-store`，查询前后的路由文件哈希一致。确认测试进程正常退出；没有连接 SAP，也没有修改其他实例。
- 未实际点击原生桌面窗口或使用真实租户验证列表。Server Hosted、其他云端点和 x.509、其他协议／能力仍是独立开发或验收项。

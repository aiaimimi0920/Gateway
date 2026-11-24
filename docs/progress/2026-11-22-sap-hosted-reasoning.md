# SAP AI Core：Hosted 的部署能力与 o3-mini 推理

## 实现

- 现有 Hosted 账号和凭据递归合并会覆盖 `sap_o3_mini`。本轮在仍能识别字段来源的合并层保留部署声明：`sap_o3_mini` 和 `sap_image_input` 仅由服务商账号声明，且只在最终部署地址、资源组和 API 版本与账号声明一致时生效。凭据不能新增、关闭或转移这些能力；空设置继承，其他服务商及普通 SAP Chat 路由的原有覆盖规则不变。
- 不改数据库结构、OAuth、TLS、域名限制、跳转或不重放策略。Hosted 和 Local 的相应准入复用现有 Chat sender；没有接入原生 Responses。

## 验证

- 修复前两项 `sap_aicore_hosted_merge` 回归均因凭据能启用推理能力而失败。修复后 SAP 46 项通过、2 项隔离用例单列；共享凭据模块 39 项通过、19 项既有专用环境用例未运行。
- 两个独立 Hosted AppState 使用专用 PostgreSQL／Redis。写入方通过真实管理路由持久化账号、凭据与项目别名；读取方走公共 Chat 路由。普通账号的凭据覆盖未能开启推理，零 OAuth、零生成。显式开启的部署不被凭据关闭，JSON／SSE 各一次成功，共一次 Token 交换。已核对 `reasoning_effort`、`max_completion_tokens`、实际部署 URL／版本与资源组，两条完成审计各 5 tokens，归属账号和凭据正确、各一次发送、没有重放。既有 X.509 Hosted JSON／SSE 对照也通过，合计 2/2。
- 独立 `line-sap-aicore-chat` lib／bins、定向 Rust 格式、31 项有效行 checker、ratchet、55 项 manifest 与 `git diff --check` 通过。合并入口有效行 206 → 221，新增部署合并模块 62 行。
- 上游为不转发的 loopback 测试端点；没有真实 SAP 租户、额度消耗或生产部署。两个 AppState 不等于两个打包的后端进程；包内 Hosted 成功生成、桌面原生账号提交与真实租户仍未验。

## 复跑

按 [Hosted X.509 夹具说明](2026-11-22-sap-hosted-x509.md#复跑)配置仅监听 loopback 的非默认端口专用 PostgreSQL／Redis。下列隔离筛选一次运行 Hosted 推理及 X.509 对照：

```powershell
cargo test --locked --offline --lib --no-default-features --features runtime-defaults --config profile.test.package.gateway.debug=0 -j 2 sap_aicore_server_hosted -- --ignored --test-threads=1
cargo test --locked --offline --lib --no-default-features --features runtime-defaults --config profile.test.package.gateway.debug=0 -j 2 sap_aicore -- --test-threads=1
cargo check --locked --offline --lib --bins --no-default-features --features line-sap-aicore-chat -j 2
```

## 桌面手测包

- `desktop / devtest`：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/release/Gateway/gateway-devtest-20261122-sap-reasoning-v1`，不是正式 Release。构建当时来源指纹 `ce73c3de87b2a33922009004000334381efead8244f8eae2b1d1e5e1b8ba96e7`（写入此段验收回执前的工作树），共 6159 个载荷文件。
- `gateway.exe` SHA-256 `c2ec652db7274f432cffdde5422fd04f98c2dca3729d370bbc87abbb69fd2423`；`gateway-ui.exe` SHA-256 `68c6c59957bfac61921a6681b1298ddb8ea2373de48852b32a3ff334a9ae8748`。分发目录与 staging 的 EXE 字节数和 SHA-256 均与清单一致。
- 包内后端在随机 loopback 端口完成隔离 SQLite 就绪、额度耗尽拒绝、重启读回、路由失败退款、Key 轮换、正常 drain，零生成、零 Redis／PostgreSQL 连接。没有启动新包的原生窗口进行账号业务点击，也未做包内双 EXE 的 SAP 成功推理。既有 4200 实例和前一包隔离桌面均未操作。专用 PostgreSQL／Redis 已停止，保留数据。

# SAP AI Core：Bedrock Converse 内嵌图片

- 公共 `/v1/chat/completions` 在部署显式开启 `sap_bedrock_image_input` 后，按原顺序发送用户消息中的文本和内嵌 PNG／JPEG。使用 [AWS ImageSource](https://docs.aws.amazon.com/bedrock/latest/APIReference/API_runtime_ImageSource.html) 的 `image.source.bytes`，不抓取 URL 或 S3。默认关闭，目录不推断能力；限制与拒绝语义见 [SAP 接入合同](../providers/sap-aicore-chat-contract-2026-11-10.md#sap-托管的-bedrock-converse)。
- 修复 Hosted 配置合并：Bedrock 没有 Azure API 版本，不能按 Azure 的版本相等判定图片设置归属。图片开关只继承自使用 Converse 的同一部署和资源组，凭据不能新增或覆盖，也不能通过带入 Azure 版本继续合并。
- 更换为可解码的合成 PNG／JPEG 夹具，并补齐现有纯文本工具用例的新参数。公共测试观察到两次真实本机 HTTP 发送，精确匹配 `/converse`、`/converse-stream` 的图片与文本顺序；各记录 5 tokens，并验证未启用、无效或有损输入的 OAuth、推理发送次数均为零。

**验证：** SAP 定向 Rust 回归 79 通过、0 失败、3 个隔离 Hosted 项未运行；SAP 独立 feature lib／bins 检查通过。控制台定向 15/15、TypeScript 类型检查、56 项 manifest、有效行 checker 31/31 与 ratchet、定向 Rust 格式及 `git diff --check` 通过。

首轮只验证了 Hosted 生产合并函数的配置归属；后续 PostgreSQL／Redis 实测见下节。本轮未调用真实 SAP、未消耗额度、未打包或部署。

```powershell
cargo check --locked --offline --lib --bins --no-default-features --features line-sap-aicore-chat -j 2
cargo test --locked --offline --lib --no-default-features --features runtime-defaults --config profile.test.package.gateway.debug=0 -j 2 sap_aicore -- --test-threads=1
```

## Server Hosted 数据库验收

两个独立 Server AppState 使用空路由文件和同一隔离 PostgreSQL／Redis。写入实例经管理 API 保存两组 Provider Account、Credential 与项目别名，读取实例经公共 Key 发出图片请求。账号池开启、凭据尝试关闭时，非流式与流式均成功；仅凭据尝试开启时，失败且 OAuth、推理发送均为零，没有完成审计。

模拟上游仅收到两次推理和一次 Token 交换；分别命中 `/converse`／`/converse-stream`，PNG 字节与文本顺序、短期 Bearer、资源组均匹配。读取实例的两条完成审计均由写入实例从 PostgreSQL 读回，每条精确归属公共调用者、项目、账号和凭据，`route_attempt_count=1`、用量为 2／3／5 tokens。这是用量归属，不是现金扣费验证。

Hosted 回归 **4/4**，涵盖 Bedrock 图片、X.509、原生 Responses 和 reasoning。同一个测试二进制的常规 SAP 过滤回归为 **79 通过、0 失败**（4 个 Hosted 项已单独执行）。隔离容器已停止，未清理数据库。没有接触真实 SAP、出包或部署；双 AppState 不是双 EXE。

复跑使用非默认端口的 loopback PostgreSQL `gateway_fixture`（应用仓库 `deploy/postgres/initdb`）及独立 Redis，沿用 [Hosted X.509 的环境变量说明](2026-11-22-sap-hosted-x509.md#复跑)：

```powershell
cargo test --locked --offline --lib --no-default-features --features runtime-defaults --config profile.test.package.gateway.debug=0 -j 2 sap_aicore_server_hosted -- --ignored --test-threads=1
```

## 桌面手测包

- 本地 `desktop / devtest`：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/release/Gateway/gateway-devtest-20261122-sap-bedrock-images-v1`，取自当时的工作树快照，不是正式 Release。manifest 与构建凭据的 `sourceTreeFingerprint` 均为 `c86aeaf35374f00e1884a5be7d6820fcde323e2df71d670156b0bbe2041d5040`，未包含便携 `.ng`。
- staging 和最终目录均通过包校验，共 6161 个载荷文件。`gateway.exe` SHA-256 为 `0b79a3cf3d29ebba42c51ba6c815596cbbcde0700651cc267cad9ac255062d01`，`gateway-ui.exe` 为 `2ce00cb320d87e0ee4648d0298d0d5f11a5c0e69f66ed8e9d30a35a028a9bcba`。
- 包内 `gateway.exe` 使用随机本机端口和独立数据目录通过 `tools/smoke-gateway-local-storage.ps1`：SQLite 就绪、重启读回、额度耗尽拒绝、失败退款、Key 轮换和正常退出。这是通用存储回归；没有启动桌面 UI，也未执行包内 SAP 成功推理、双 EXE Hosted 流程或真实租户验证。未替换或部署正在运行的 Gateway。

### 同一手测包：Bedrock 部署配置与公共准入

- 使用同包 `gateway.exe` 和独立 Local SQLite，先通过管理 API 与临时秘密授权创建合成 Service Key 账号，再以 revision CAS 保存部署级图片开关（沿用 `keep`，不重新传秘密）。旧 revision 返回 409；正常退出后用不同 PID 重启，读回同一 revision、账号 ID 和图片开关，秘密仍为 `configured=true` 且不回显。
- 关闭时，公共 Key 上传合法内嵌 PNG 得到 `400 invalid_sap_aicore`，零 OAuth。重启且启用后，远程 URL 仍返回 400，零外发；内嵌 PNG 则进入 OAuth，通过本机不转发代理仅观察到一次预期 XSUAA `CONNECT`，返回 `502 sap_aicore_token_failed`。无推理、无图片回显、两条预付消息额度保持不变；两个运行阶段均正常退出。
- 这是包内配置持久化和公共准入验收，不是成功推理、原生 UI 点击、Hosted 双 EXE 或真实租户验收。

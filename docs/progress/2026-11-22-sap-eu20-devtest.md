# SAP AI Core：Azure EU20 桌面手测包与双后端负向验收

## 包来源和隔离运行

- `desktop / devtest`，不是正式 Release。包目录 `C:/Users/Public/nas_home/AI/GameEditor/Neuro/release/Gateway/gateway-devtest-20261122-sap-eu20-v1`，共 6159 个载荷文件。构建来源 `sourceTreeFingerprint=049037226916c4f1d8134440ec71095850037b5c467ebd026950943e1c26dc87`；此包早于 o3-mini 功能修改。
- 两个 EXE 的字节数、SHA-256 与清单一致：`gateway-ui.exe` 为 `1c7c6c484a454aea9e453a008bd15e6af959dcda471be0f871691a39d3328ab7`，`gateway.exe` 为 `8f3cb83f4f3874cbffde4b96c46aa270bfe631b0e8a5040217e0d25fb717279e`。
- `tools/smoke-gateway-local-storage.ps1` 通过。隔离 SQLite 启动／重启、额度耗尽拒绝、失败退款、Key 轮换和 drain 均通过；零生成、零 PostgreSQL 或 Redis 连接。该通用存储回执不证明原生 SAP 账号重启读回。

## SAP 的包内证据

- 原生桌面实际创建 Azure EU20 空池；管理 API 读回正确部署 URL、`group-a`、`2024-10-21`、零账号，未触发探测。保存合成 SAP 账号是**同一实例的管理 API**操作，不是原生点击：WebView2 下拉选择未命中本窗口。管理 API 读回 `sap-eu20-synthetic`、`configured=true`、revision `r2-a5746df4fd61`，无秘密回显。窗口后来处于最小化，激活返回 `user input was detected in this window`，未确认重启读回或桌面正常退出；未碰正式实例。
- 同包两个独立 `gateway.exe` 使用专用隔离 PostgreSQL／Redis。创建的 SAP Hosted 账号可跨进程读回；有效 Gateway Key 经项目别名进入两个实例的 Hosted 公共 JSON／SSE 路由。非法入站 Key 得到 `401`，零外发；有效请求各尝试一次正确的 XSUAA `CONNECT`，被不转发的本地代理拒绝，返回 `502 sap_aicore_token_failed`。读回两条准确归属的失败审计、零推理、零生成用量；两进程正常 drain，停止临时启动的专用容器，保留数据。
- **未通过包内成功生成验收。** 不能把负向路径当 mTLS 或成功推理。X.509 成功另见[双 AppState 源码回执](2026-11-22-sap-hosted-x509.md)。真实租户、凭据、额度和费用均未使用。

原始一次性回执在 `C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-sap-devtest-20261122`，不是后续开发或测试依赖。

# SAP AI Core：Orchestration v2 只读部署发现

2026-11-22 联网读取了 SAP AI SDK 固定版本的 [Orchestration 客户端](https://github.com/SAP/ai-sdk-js/blob/4f624beed6988cf73fd1d322ff9bade30b66065b/packages/orchestration/src/orchestration-client.ts) 和 [部署 resolver](https://github.com/SAP/ai-sdk-js/blob/4f624beed6988cf73fd1d322ff9bade30b66065b/packages/ai-api/src/utils/deployment-resolver.ts)。客户端走部署下的 `/v2/completion`；resolver 用 `scenarioId=orchestration`、`status=RUNNING` 和资源组查部署，不指定 executable。未根据地区列表猜测新 API origin。

- 现有受保护的只读管理入口新增 `protocol: "orchestration"`；不改变省略协议时的 Azure 行为或 Bedrock 的 executable 过滤。复用同源、部署 ID、运行状态、预算、禁止重定向与脱敏约束；不将工作流元数据当作模型能力。无存储迁移或自动推理。
- 控制台启用 Orchestration v2 时可以显式查询。没有默认选择；选中条目只填部署 URL，旧绑定模型清空，需要重新填写明确的模型后才能保存。查询用 Service Key 不进入草稿或账号池。手填路径仍可用。
- 隔离本机 HTTP 回归中，无授权请求零上游；授权后恰好一次 Token 交换和一次列表请求。停止、异源、ID 不符、Azure／Bedrock 条目均被排除；响应 `no-store`，配置 revision 不变。前端验证了独立协议、查询后清空 Key、无自动选择、必须手填模型以及空池配置。

## 验证

- SAP Rust 定向 **80 通过、0 失败、5 跳过**（Hosted 隔离用例未在本轮复跑）；默认功能的 `cargo check --tests`、独立 `line-sap-aicore-chat` 的 lib／bins 均通过。
- SAP 前端定向 **23/23**、TypeScript 类型检查、56 项 manifest、有效行 checker **31/31** 与 ratchet、定向 `rustfmt` 和 `git diff --check` 通过。
- 只使用合成 Service Key 和不转发的本机 SAP 测试服务。未使用真实目录权限或租户、未出新包、未部署；已有手测包不是这次新功能的产物。

复跑可使用 `cargo test --locked --offline --lib --no-default-features --features runtime-defaults --config profile.test.package.gateway.debug=0 -j 2 sap_aicore -- --test-threads=1`；控制台测试筛选 `SapAiCoreOrchestrationDiscovery`。

## 桌面手测包与包内验证

- 随后的 `desktop / devtest` 包为 `C:/Users/Public/nas_home/AI/GameEditor/Neuro/release/Gateway/gateway-devtest-20261122-sap-orchestration-discovery-v1`，取自当时的工作树，非正式 Release。staging 与最终目录校验通过，共 6161 个载荷文件；manifest 与构建凭据的 `sourceTreeFingerprint` 均为 `a0e688930b158ab67a37e1e1ed8e9adc90be47f83426131a2a337c3214c5dcf7`，不含便携 `.ng`。
- `gateway.exe` SHA-256：`3d27b4ba32c9990d0bffb7a30391afa5b27b7b88ed4cf3b01ca4b0311689c085`；`gateway-ui.exe`：`48a68a94ea01de6663c053ae51948484d4c9975d7d6fc29b3fdbe38e497a7dea`。包内后端通过独立 SQLite 启动、重启读回、额度耗尽拒绝、失败退款、Key 轮换和正常退出。
- 在另一隔离数据目录对包内管理入口发请求：无身份 `401`、无敏感授权 `403`；未知协议、非法资源组和额外场景字段均 `400`，零 OAuth。授权后的合法 Orchestration 查询仅发起一次预期的 XSUAA `CONNECT`，由本机不转发代理拦截，返回 `502 sap_aicore_token_failed`。响应 `no-store`，路由 revision 和空池状态不变，无秘密回显，进程正常退出。
- 部署查询在包内仅验证授权和 OAuth 前后边界；成功目录列表仍是上面的源码合成回归，不是包内成功查询。未运行双 EXE Hosted 成功推理或真实 SAP，未覆盖或部署在线实例。

## 原生桌面账号保存补验

- 复用同一手测包，在独立数据目录和 `127.0.0.1:50761` 启动 `gateway-ui.exe` 及其托管后端。通过该桌面实际承载的 WebView2 自动化交互，从零创建 Orchestration v2 空池；填写合成部署 URL、资源组和固定模型 `gpt-4o`，再从真实账号对话框输入 `sap-native-orch-acceptance` 与合成 Service Key，点击保存。没有通过外部脚本直接写管理 API。
- 桌面发出的 `PUT /v1/internal/gateway/console/route-config` 与随后的 `GET` 均为 `200`，结果包含账号且 `configured=true`，响应无合成 secret；账号列表显示该账号。关闭桌面后，独立存储中仍有含该账号和部署的持久化版本 `r2-dfc108938a9f`。
- 通过桌面关闭按钮退出，确认本次桌面与托管后端均已退出、测试端口已释放，既有 4200 实例未被替换。执行工具拒绝本次重新启动命令，因此**未验证新进程读回**；同进程 GET 和磁盘版本不能替代重启验收。未执行部署查询或推理，未使用真实 SAP 账号。

## 独立后端新进程读回补验

从同一手测包（重新验证 `gateway.exe` SHA-256）启动新的 `gateway.exe`，只挂载上一步原生桌面实际写入的隔离数据目录，使用新的 loopback 端口和临时管理令牌。`/readyz` 确认 SQLite、不依赖 Redis；受保护的 `GET /v1/internal/gateway/console/route-config` 返回原版本 `r2-dfc108938a9f`、相同部署 URL、模型列表和 `sap-native-orch-acceptance` 凭据 ID，且该账号的 secret descriptor 仍为 `configured=true`。响应和本次后端日志不含合成 secret；通过管理 drain 以 0 退出，路由文件字节未变。

回执：`C:/Users/Public/nas_home/AI/GameEditor/linshi/sap-native-account-0277c6b422ad45ef8ceed01625cccadd/backend-restart-3073e84234e2490cb0e39e4078344a57/result.json`。本次证明**新后端进程**能从桌面写入的数据读回账号，没有再次打开原生窗口，不替代桌面重启后可见性验收；未调用真实目录或推理。

## 原生窗口重启读回

对同一个包重新核验 `gateway-ui.exe` SHA-256 后，用原隔离目录启动新的原生桌面进程和其托管后端。实际 WebView2 无障碍树确认页面来自 `http://127.0.0.1:50761/ui/`，凭据池仅 1 个账号，行内显示服务商 `sap-aicore-chat-api` 与已保存的 `sap-native-orch-acceptance`。只读检查，不重新提交凭据、不执行目录查询或推理。窗口在收尾时最小化，未把 UI 点击退出记成通过：验证监听者确为本轮托管后端后发送 drain（202），后端退出，再清理本轮桌面进程，确认 50761 释放。既有窗口未被操作。

回执：`C:/Users/Public/nas_home/AI/GameEditor/linshi/sap-native-account-0277c6b422ad45ef8ceed01625cccadd/native-restart-9be0078ed6eb4ac297bfad1ec7edfc7d/result.json`。此条关闭合成账号的原生窗口重启读回，不替代成功目录查询、包内双 EXE 推理或真实租户验收。

## 2026-11-23 原生窗口成功查询并保存

- 使用当前 `desktop / devtest` 包 `gateway-devtest-sap-native-catalog-20261123-102429`，6188 个载荷通过打包及最终完整性检查。manifest SHA-256 `a568b9869806f19e310e22900c07f23fffe6cfd5ee75a7f5c8e414ce73894f7f`；`gateway.exe` SHA-256 `a8e94ed7d3ba9ed491dbc652715922c8aa4a859de424274a83878428eec39ae4`；`gateway-ui.exe` SHA-256 `3247a4a8a7e069b1ce288dfe9e21be2d87048e853af9bc19050009df31678679`。
- 首轮在真实 `gateway-ui.exe` 的 WebView2 页面连接隔离 Hosted 后端，使用合成 Service Key 查询 Orchestration，收到 HTTP 200 和唯一条目 `catalog-orchestration-a`。查询输入清空、无默认选择；选择后清空旧模型，未填固定模型时不能提交。填写 `gpt-4o` 后通过页面创建空池，提交不携带查询 secret；新账号表单中的 Key 初始为空，重新录入后由页面发出 `PUT`、返回 HTTP 200，响应不含合成 secret。独立落盘 `server-first/routes.yaml` 复核了部署、模型与账号 `sap-native-catalog-acceptance`。未借管理 API 代替页面操作。
- 隔离回执目录：`linshi/gateway-packaged-provider-fixture/sap-native-catalog-first-a445f42f0a254c6099120d653178daea`。首轮新的后端进程已启动，但验收脚本把 `redis_required=true` 固定用于重启；已有路由时实际返回 `database=true, redis=true, redis_required=false, degraded=false`，脚本在业务读回前退出。修正首启／重启预期后，后续尝试在 WebView2 自动化定位阶段超时，未压过首轮已完成的查询与保存结果，也未生成全通过 `result.json`。后续从首轮持久化状态直接补验，见下。
- 所有本轮隔离进程均已退出，未改 4200、未使用真实 SAP 账号或费用。两次 Rust 构建耗时只作为该次编译观测，不能推断新架构的提速倍数。

### 从同一份原生提交重启读回

直接复用首轮包与原生提交，没有重新查询目录或重录账号。隔离 Redis 活跃 revision 为 `r2-c236f1789420`，活跃文档与首轮 `server-first/routes.yaml` 解析后完全相同。用同一包的全新后端进程从该 namespace 读取，受保护管理响应返回同一 revision、部署、资源组 `gateway-fixture`、固定模型 `gpt-4o` 和原账号；凭据描述为 `configured=true`，未回显原合成凭据。

用新的桌面进程和独立 WebView2 数据目录打开该后端，在真实窗口的账号编辑界面确认原账号与 `catalog-orchestration-a`；Service Key 字段为空、保留密码遮罩和已配置占位，不点击显示。新后端正常 drain、窗口正常退出；回读前后 Redis 文档、revision 和首轮磁盘文件均未改变，本次重启零上游发送。回执：`linshi/gateway-packaged-provider-fixture/sap-native-readback-restored-20e3f44d7c8742179c84cf80a189a6c5/result.json`。

这补齐同一份合成原生提交的「查询 → 明确选择 → 提交 → 新后端和新窗口读回」，不抵扣真实租户、未覆盖的协议和媒体。此前 readiness 断言失败与 WebView2 定位超时仍保留为历史记录。

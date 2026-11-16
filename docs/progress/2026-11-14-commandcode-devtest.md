# Command Code 当前源码手测包与隔离运行回执

本回执证明 devtest 包及以下非账号运行流程，不是 Command Code 整卡完成或三个服务商完成声明。

## 产物

- 类型：`devtest`，desktop；`Jobs=4`，保留锁定依赖，未覆盖旧包。
- 目录：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/release/Gateway/gateway-devtest-commandcode-20261114-closure1`
- 桌面入口：上述目录的 `gateway-ui.exe`；后端为 `gateway.exe`。
- 后端 SHA-256：`401964ea7610f829d2555a2a8ca560bd5d0daadcdce7042dda279b5332a539d1`
- 桌面 SHA-256：`b7454c563d82d70ec752ac3021f545dd2d745a085a5c63095d18d794c2ef9b03`
- 构建入口：`tools/package-gateway-devtest.ps1 -Edition desktop -Jobs 4 -VersionId gateway-devtest-commandcode-20261114-closure1`
- 构建、来源凭据及打包检查 exit 0；后端 Cargo 13m45s、桌面 Cargo 44.23s。
  这是本次观测，不用于推断冷/热构建提速倍数。
- 使用当前完整工作树，不撤销其他工作线修改；不是仅含 Command Code 的 cherry-pick 构建。

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-first-three-20261113`。
打包日志 `commandcode-devtest-package.log`，来源凭据同时保留于仓库
`target/release-evidence/gateway-devtest-commandcode-20261114-closure1/`。

## 实际包内运行

1. 仓库 `tools/smoke-gateway-local-storage.ps1` 通过，结果
   `commandcode-packaged-local-storage/result.json`：SQLite 就绪、零额度 429、Key/余额重启保持、
   路由失败退款、轮换保留余额且撤销旧 token；监听陷阱未收到 Redis/PostgreSQL 连接。
2. 一次性 `commandcode-packaged-systemone.ps1` 通过，结果
   `commandcode-packaged-systemone-v2/result.json`：使用独立数据目录启动；
   公共 `/v1/systemone` 无凭据 401、零额度 429、stream=true 400；重启后同一 Key 与额度拒绝保持。
   两次实例均通过 drain 正常退出。未发送可执行上游生成的有额度有效请求。
3. 一次性脚本首次运行因漏定义共用 smoke helper 依赖的 `Write-Smoke` 失败；原日志保留。
   补齐日志函数后使用全新 v2 数据目录通过，没有修改产品 EXE 或覆盖首次证据。
4. 现金边界追加验证发现脚本的外部路由文件被 Windows Local 启动默认路径替代；此前 v2
   的拒绝路径结果有效，但不能据此声明合成目录已加载。现金首次及 cash-v2 运行返回
   `No providers configured for model`，均按失败保留。修正夹具，将目录预置于隔离
   `data/local/routes.yaml` 后，`commandcode-packaged-systemone-cash-v3/result.json` 通过：
   System One 返回 HTTP 400 / `cash_usage_bound_unavailable`，现金余额前后完全一致，
   `spentMicros/reservedMicros/pendingRequests` 均为零；重启、401/429/非法 stream 拒绝和 drain 同时通过。
   产品 EXE 未修改。未发送的结论结合该错误在生产 admission 中早于预留和 sender 的位置，
   不是通过真实上游监控计数得出。

所有状态均置于证据根下的新目录，不访问真实 `.ng`、账号或数据库；旧包、测试数据和日志保留。
这些测试不需要 Docker，不替代仍开放的 Server/PostgreSQL/Redis 多实例验收。

## 验证边界

成功的合成上游推理、协议/用量/取消/退款已由当前源码 104 项 Command Code 回归覆盖；
包内本轮验证的是启动、入口拒绝、持久化和退款，不宣称原生 EXE 已完成真实上游推理。
桌面表单的组件测试 6 项与 typecheck 通过，不代替原生桌面窗口的人机交互验收。
用户已批准 System One 保留普通配额/消息预付与实际用量，现金预付明确拒绝、不发送，
现金接入留待上游费用上限合同确认；不影响其他 Command Code 接口现金能力。

原生桌面独立启动证据 `commandcode-native-launch/result.json` 表明进程存活、窗口句柄与
WebView 目录均存在；这不等于视觉或交互验收。自动控制依赖 `@oai/sky` 和
`@oai/cua/tinyskyAlt` 缺失，窗口点击/表单操作仍未验证。

用户已批准启动 Docker Desktop 并创建保留数据的隔离测试容器。Engine 已就绪；现有容器
启动后清单保存为 `docker-existing-inventory.json`，未修改既有容器。
新建 `gateway-first-three-pg-20261114`（`127.0.0.1:55147`）和
`gateway-first-three-redis-20261114`（`127.0.0.1:56456`），使用已有
`postgres:16-alpine` / `redis:7-alpine` 镜像、独立同名数据卷、`restart=no`。
`pg_isready` 与 `PING` 通过，回执 `docker-isolated-services.json`。容器和卷保留，不执行删除。
这仅解除环境阻碍，Server/双实例业务验收仍开放。
未部署、提交或推送。

# SenseNova Hosted 双 EXE：验收工具与管理链路

首批服务商收口计划中的包级验证。依旧没有使用真实账号或发送付费上游请求。

- 仓库新增 [隔离 Hosted runner](../../tools/packaged-provider-fixture/README.md)。旧包先走
  缺 CA 启动门禁，无法证明夹具生效就不发推理。新的隔离 devtest 使用固定服务商域名和
  独立 TLS 信任；常规包不允许启用此入口。不安装系统证书，不修改生产/default client。
- 合成 peer **3/3 通过**，覆盖 Chat／Messages／Responses 的 JSON 和 SSE、非白名单
  CONNECT、错误合成 Key、不受信 CA 与不同响应 ID。它只接受 loopback，不转发。
- 复用旧 `gateway-devtest-20261122-sensenova-persistence-v1` 仅验证管理链路：两个
  独立后端进程的 PostgreSQL／Redis readiness；实例 A 创建账号、凭据、别名、三种
  capability/platform access、bundle 并签发 `neuro` Key；实例 B 读回凭据且不回显，
  两个进程正常 drain。未开启 peer 服务或发送公共推理。首轮 Key 签发 HTTP 409，随后
  按生产合同传入两实例共享的随机 `GATEWAY_API_KEY_SECRET`，重跑通过。回执：
  `C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-packaged-provider-fixture/management-only-26a44dd280434d21973b477491ed6d7b/result.json`。
  同一旧包直接运行完整 runner 时，在缺 CA 门禁超时并被清理，未进入账号配置或推理阶段。
  两个专用容器已停止，隔离数据保留。
- Rust `cargo check --locked --offline --jobs 4 --no-default-features --features runtime-defaults --lib --bins --tests`、
  定向 `rustfmt --check`、56 份功能线清单、有效行 checker 31 项及 ratchet 通过。

Rust 定向测试 **39 通过／0 失败／5 项隔离用例未随此命令执行**，包含两项新增夹具输入边界单测。`powershell.exe` 出包在源码指纹预检阶段未找到 `Get-FileHash`，未生成任何包；改用同机已确认可用的 PowerShell 7 运行原打包入口，不绕过校验。

## 新包复现与修复

- `gateway-devtest-sensenova-hosted-20261122-204357` 已在 PowerShell 7 完成打包；staging／最终目录完整性通过（6167 文件），隔离 SQLite smoke 通过；后端 SHA-256 为 `6b04502737685beecb02f2d8b996c996df52e2edab463c54ba4f5431c705cc2b`。原测试 EXE 补跑隔离 PostgreSQL／Redis **5/5**；后者是双 AppState，不是双 EXE。
- 新包首个公共 Chat 请求 HTTP 429。增加诊断后，候选预览复现 HTTP 429 / `balance_not_initialized`；合成 peer 零请求，两个进程均正常 drain。夹具改用正式管理 API 初始化七次合成消息额度，不修改生产鉴权；三个入口随后均读到唯一可用候选。
- 修正余额后，Chat JSON 通过，Chat SSE 有答案和唯一 `[DONE]`，但帧中只有上游模型 `deepseek-flash`，没有公共别名。失败回执：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-packaged-provider-fixture/published-hosted-2466cffa443a4e6fb8bf72b8080d5784/failure.json`。
- 为定位其余路径，用同包做一次仅诊断运行：只在临时诊断进程内替换 Chat SSE 断言所读的模型值；六次公共调用、六条准确审计、两项 CA 负对照及正常退出执行完成。回执为 `diagnostic_only_chat_alias_known_bad`，不是包级通过：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-packaged-provider-fixture/published-hosted-d89e126ae7014760b0450359e91b1eee/result.json`。
- 源码已在 SenseNova Chat 的有界 SSE 守卫投影顶层 `model`，不改嵌套工具数据、终态或计费观察；其他服务商的 Chat 守卫默认行为、Messages 与 Responses 保持原样。正式 runner 保留别名断言并检查每个 Chat 数据帧；失败回执保存候选、peer 请求与清理状态，递归检查合成凭据是否回显。

本轮源码 `cargo check --locked --offline --jobs 4 --no-default-features --features runtime-defaults --lib --bins --tests`、定向 `rustfmt --check`、56 份功能线清单、有效行 checker 31 项与 ratchet 均通过；Python 夹具和脱敏诊断 4/4。含别名断言的 SenseNova 定向测试 **39 通过／0 失败／5 项隔离用例未在本轮重跑**；同一测试 EXE 的共享 Chat SSE 守卫 **17/17**。

## 修复后的包级验收：通过

- 桌面 `devtest`：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/release/Gateway/gateway-devtest-sensenova-alias-20261122-215418`。最终校验索引共 6167 文件；两份 EXE 的大小和 SHA-256 与校验记录一致，未执行本包的原生 UI 启动。
- `gateway.exe`：`f8d5706275bcbadce6f19a6086d74a06d0f733da196f651a4ac414d49589d8de`；`gateway-ui.exe`：`5485a3c27b7143216f2d13c50fec95e8d911536ea86d435ded65cb9d1bc59451`。
- 原样 runner 退出 0，回执 `status=passed`，其中后端及 manifest 哈希与最终包匹配：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-packaged-provider-fixture/published-hosted-1e1e8228cab24377b44d2b10041dd696/result.json`。
- 两个独立后端共享隔离 PostgreSQL／Redis，后端 B 读回 A 保存的凭据。Chat／Messages／Responses 各一条 JSON 和一条 SSE；每个 Chat SSE 数据帧返回公共模型别名。合成 peer 恰好收到六条请求，PostgreSQL 恰好六条成功公共审计，每条 2/3/5 tokens，零内部探测审计。缺 CA 和错 CA 均未发送额外推理；后端正常 drain，两个专用容器已停止，保留数据。

本回执只关闭 Hosted 双后端 EXE 的合成公共推理门槛，不替代目录、包内自动调度、桌面业务点击、真实账号／费用或部署验收。

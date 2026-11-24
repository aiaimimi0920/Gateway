# 服务商双 EXE 手测夹具

仅用于明确隔离的 Windows `devtest`。使用两个独立 `gateway.exe` 进程、空本地路由、
同一专用 PostgreSQL／Redis 和合成 TLS 服务。第一个进程通过管理 API 创建账号、凭据、
模型别名及绑定 access bundle 的 `neuro` Key，并经管理 API 初始化七次合成消息额度；
第二个进程读回凭据、验证三个可用候选，再从公共入口提交请求。
Chat、Messages、Responses 各发 JSON 和 SSE 一次，检查别名、协议终态、2/3/5 tokens、
精确上游请求、六条公共审计和正常 drain。结束保留隔离数据，不清表或清 Redis。

合成服务只在 `127.0.0.1` 监听，只接受指定 SenseNova authority 的 CONNECT 并在当前
socket 终止 TLS，不能转发。随机生成的合成服务商 Key 不写回执，根证书不装入系统。
Runner 在任何推理之前先确认包确实拒绝不存在的夹具 CA，避免误用不含隔离接线的旧包；
正向测试后另启进程，用无关 CA 检查 TLS 失败且无推理请求到达。不要导入真实凭据。
证据目录含隔离日志和一次性测试私钥，按本地临时产物保管，不发布。

## 在仓库根目录执行

依赖 Python 3.12、`cryptography`（已用 50.0.2 验证）、Docker CLI、包含该夹具接线的
新 devtest 包。仅使用已有的 `gateway-first-three-pg-20261114` 和
`gateway-first-three-redis-20261114`，需要事先启动且以非默认端口绑定 `127.0.0.1`。
PostgreSQL 专用 `gateway_fixture` 库需已按仓库流程应用至 `007`。不要把生产连接字符串
传给 runner。将 `SENSENOVA_TEST_DATABASE_URL` 设为已正确百分号编码、指向专用容器的
`postgresql://…@127.0.0.1:<port>/gateway_fixture`，将 `SENSENOVA_TEST_REDIS_URL`
设为 `redis://127.0.0.1:<port>/`。Runner 会验证容器标签与发布端口，不启动或停止容器。

```powershell
$root = 'C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-packaged-provider-fixture'
New-Item -ItemType Directory -Force -Path $root | Out-Null
$env:GATEWAY_FIXTURE_EVIDENCE_ROOT = $root
python -B tools/packaged-provider-fixture/test_sensenova.py -v

$package = 'C:/Users/Public/nas_home/AI/GameEditor/Neuro/release/Gateway/gateway-devtest-<本次构建>'
python -B tools/packaged-provider-fixture/run_sensenova_hosted.py --package $package --evidence (Join-Path $root ([guid]::NewGuid().ToString('N')))
```

仅 `result.json` 出现 `status=passed` 代表本轮 runner 完成。非零退出检查 `failure.json` 和隔离日志。
失败回执记录阶段、候选计数、合成 peer 观察和清理状态，不写原始响应。
不修改已有包，也不把夹具通过延伸为目录、调度、桌面点击或真实账号／费用验收。

## Command Code 与 SenseNova 双进程定时探测

同一专用 PostgreSQL／Redis，设置 `COMMANDCODE_TEST_DATABASE_URL` 和
`COMMANDCODE_TEST_REDIS_URL`（格式同上）；SenseNova 使用 `SENSENOVA_TEST_*`。选择包含对应产品隔离接线的 devtest 包：

```powershell
python -B tools/packaged-provider-fixture/run_commandcode_scheduler.py --package $package --evidence (Join-Path $root ([guid]::NewGuid().ToString('N')))
python -B tools/packaged-provider-fixture/run_sensenova_scheduler.py --package $package --evidence (Join-Path $root ([guid]::NewGuid().ToString('N')))
```

启动两个独立后端，使用相同的独立路由文档和唯一合成凭据；不触发公共推理或手动探测。
两项须串行运行。每项等待两个真实调度周期，验证每周期只发送一次、整个独立 namespace 恰有两条正确归属且用量为 2/3/5 的内部完成审计、零公共审计，
并在重启后读回结果。先以缺 CA 门禁证明新包接线生效；合成 peer 只接受固定域名和探测详文，
不转发。证据目录含临时合成 Key 和 TLS 私钥，不发布。未验证真实账号、Hosted 公共转发或桌面业务点击。

## Command Code 包内 Hosted 公共推理

使用上述专用容器和 `COMMANDCODE_TEST_DATABASE_URL`／`COMMANDCODE_TEST_REDIS_URL`。与定时探测分开执行；选择包含 Command Code 三个固定推理地址隔离接线的同源 devtest：

```powershell
python -B tools/packaged-provider-fixture/run_commandcode_hosted.py --package $package --evidence (Join-Path $root ([guid]::NewGuid().ToString('N')))
```

第一个后端通过管理 API 保存合成 Studio Key、声明三原生端点的目录、公开模型别名和已授权的 `neuro` Key；第二个后端经公共 Chat、Messages、Responses 各做一次 JSON 和 SSE。Chat peer 要求图片 URL 和 `detail` 原样到达，仅校验 wire，不下载图片。校验六次原生发送、公开别名（含全部 Chat JSON 帧）、协议终态和六条精确 PostgreSQL 用量审计，另验缺 CA、错误 CA 和 drain。不证明 System One、真实账号、图片下载或供应商实际推理。

System One/Jev 使用同样的隔离容器和变量，但需要包含固定 `/provider/v1/systemone` 隔离接线的新 devtest。
`run_commandcode_systemone.py` 使用相同的 `--package` 和 `--evidence` 参数。它只发送一条非流式决策，
检查原生请求结构、公开别名与结构化答案、精确 PostgreSQL 审计和消息额度从 2 扣至 1；另做缺 CA／错误 CA 负对照。
它不发现金预付请求，也不替代真实 Studio Key 验收。

## SenseNova 账号目录

使用同一专用数据库和 `SENSENOVA_TEST_*` 配置，选择包含目录隔离接线的 devtest：

```powershell
python -B tools/packaged-provider-fixture/run_sensenova_catalog.py --package $package --evidence (Join-Path $root ([guid]::NewGuid().ToString('N')))
```

从实际管理入口分别读取草稿 Key 与已保存账号目录。先验证 401、缺敏感授权 403 及非合成 Key 零发送；
两次成功读取只保留去重后的文本模型，绑回请求时的 revision，配置不写入且不发推理。
测试只访问不转发的 loopback peer；它不代替原生窗口点击、账号保存后的目录持久化或真实账号权限。

## SenseNova 图片与工具推理历史

同一专用库、`SENSENOVA_TEST_*` 和 API devtest，执行
`run_sensenova_media.py --package <包目录> --evidence <linshi 下的全新目录>`。
双 EXE 分别配置／推理；四条正向覆盖图片 JSON／SSE，以及 reasoning＋工具的 JSON
首轮和实际 assistant 回传的 SSE 续轮。严格核对图片字节、顺序、detail、reasoning
空白、工具 ID／arguments、公开正文／终态／用量、四条精确审计及净扣四条消息。
连续四条非法图片／历史必须全部 400、零 POST、零实际归属、精确拒绝原因，
且凭据仍为 active／failure_count=0、余额不变；不重置健康或余额求绿。
另验缺 CA、错误 CA、日志脱敏与 drain。`post_attempts` 包含被 peer 拒绝的
认证／正文请求，不仅统计成功 wire；不证明视频、OAuth／积分池或真实账号。

## SAP AI Core 双身份 Hosted 公共推理

仍使用同一组专用容器；将连接放入 `SAP_AICORE_TEST_DATABASE_URL` 和 `SAP_AICORE_TEST_REDIS_URL`。
用含 SAP 隔离接线的 `api` devtest 执行 `run_sap_hosted.py --package <包目录> --evidence <linshi 下的全新目录>`。
先以缺 CA 启动负对照识别旧包，未确认隔离接线前不创建账号、不发推理。Azure EU20 使用合成 client secret；
AWS 使用单独的直连 X.509 OAuth 监听器，校验客户端证书、独立 Bearer token 和实际推理目标。两个监听器
都不转发、不修改系统信任根，也不使用真实凭据。每种身份先从公共 Chat／Messages／Responses 各发一次 JSON／SSE，上游走 Chat 部署。
另用独立部署 ID 验证原生无状态 Responses、Orchestration v2 文本、Bedrock Converse 内嵌 PNG，各做 JSON／流式两次。
另验证 Azure Chat 内嵌 PNG 的三公共入口、原生 Responses 内嵌 PNG 和 o3-mini Chat 推理参数。
再用独立部署验证 Orchestration 内嵌 PNG JSON／SSE，以及首轮实际 assistant 回传的
`high` JSON／`xhigh` SSE reasoning 块与 opaque signature 保真。
再用四个独立部署覆盖 Orchestration／Bedrock 的纯文本工具和图文＋工具，各做 JSON／流式
两轮；第二轮必须提交实际首轮工具调用，逐项核对 ID、参数片段、工具结果和最终出站 wire。
共 48 条完成审计、14 次 OAuth 和 48 次 inference 尝试，检查公开别名、终态、用量、
逐次成功的消息扣减和每部署 Token 复用。两类错误 CA 另检查各一条失败审计、
零 OAuth HTTP／零推理／无实际归属、余额恢复到失败前、健康快照不变及正常 drain。
本矩阵不验证签名密码学，也不覆盖 OAuth 后撤销候选触发的普通供应商回退。
不验证部署发现、原生窗口、真实租户权限或费用。`test_sap_peer.py` 可先独立检查 TLS 对端和原生协议边界。

## SAP AI Core 包内只读部署发现

复用上面的 API 包、隔离连接及证据目录设置，执行 `run_sap_catalog.py --package <包目录> --evidence <linshi 下的全新目录>`。
验证三种目录、双身份、分页去重、同源和协议过滤、敏感授权、零推理与路由 revision 不变。
不替代真实目录权限、原生窗口选择、账号保存或重启读回。

## SAP AI Core 包内 OAuth 超时与恢复

复用上述含 SAP 隔离接线的 API devtest 与 `SAP_AICORE_TEST_DATABASE_URL`、
`SAP_AICORE_TEST_REDIS_URL`。与其它使用同一专用容器的 runner 分开、串行执行：

```powershell
python -B tools/packaged-provider-fixture/run_sap_auth.py --package $package --evidence (Join-Path $root ([guid]::NewGuid().ToString('N')))
```

检查缺 CA 启动拒绝、client-secret OAuth 超时且零推理、两路并发只交换一次 Token、
到期重新交换、精确归属审计、失败不净扣额度及正常 drain。不验证真实租户、X.509
超时、缓存容量或 Keep 绑定。

## SAP 持久化 Responses 双 EXE 生命周期

使用上述 `SAP_AICORE_TEST_*` 与包含持久化接线的新 API devtest，执行
`run_sap_stored_lifecycle.py --package <包目录> --evidence <linshi 下的全新目录>`。
runner 核对专用容器后，核对包内 `008`–`011` 与当前夹具的迁移哈希，并应用包内
附加表，不触碰线上库或清理测试数据。旧缺少 `011` 的包不能用于此恢复验收。

先验缺 CA 启动拒绝，再通过管理 API 创建合成账号、两个消息预付成员和聚合 Key。
实际创建、进程 drain/restart、跨 EXE 续接、并发单次 cancel、GET 与 DELETE；
验证墓碑与原成员过期均零发送，再经管理 API 刷新聚合成员，证明新请求可使用备用成员。预期四个回执各结算
`2/3/5` tokens、合计十次上游发送、十三条终态公共审计，按原成员只扣四条消息。
遍历全部有界 pending 页，确认这四个已结算对象不在待处理列表。

随后单独验证恢复增量：仅为本轮存活成员安装临时 PG trigger，使下一次真实公共
create 分别记录有 `2/3/5` 用量、无用量的终态，但关闭 hold 均失败。保持故障并 drain
所有旧 EXE，移除本次 trigger/function 后启动两个新 EXE，不发公共 GET，等待后台完成本地结算；
无用量的 tokens 必须维持 NULL。
另造一对明确标注为 **synthetic consumer fixture** 的未发送／未知记录，
仅第一条带耐久证明；验证恰退一次、未知预扣不动、第二个周期稳定、无新增业务
请求或 OAuth POST、无虚构公共审计。造行不证明 live submit owner 会生成未发送证明，
生产者路径由源码故障用例另验。双 EXE 环境不等于已观察到同时争抢同一行；
精确并发 CAS 由双库独立连接源码用例验证。

合成 TLS 服务不转发，记录进入持久化端点的所有尝试而不仅成功请求；核对精确
方法、路径、前序 SAP ID 和 `background:true`。本 runner 不重验错误 CA、真实
SAP、原生窗口或未知提交的人工恢复；这些不能由生命周期成功回执抵扣。

## SAP Hosted 管理探测失败与发送前撤销

复用上述 API devtest 和隔离连接，串行执行：

```powershell
python -B tools/packaged-provider-fixture/run_sap_probe.py --package $package --evidence (Join-Path $root ([guid]::NewGuid().ToString('N')))
python -B tools/packaged-provider-fixture/run_sap_fallback.py --package $package --evidence (Join-Path $root ([guid]::NewGuid().ToString('N')))
```

探测 runner 校验并安装包内 `012`，检查管理授权、OAuth 429／推理 503、`Retry-After`、
实际推理次数与状态、精确模型健康和零公共审计。fallback runner 在 SAP OAuth 停住时由
管理 EXE 撤销精确凭据，然后放行 Token，检查普通 OpenAI-compatible JSON／SSE 各只发送一次；
同时检查公开别名、终态、用量、余额、实际审计归属和 SAP 的零发送拒绝。普通 peer 只在
loopback 监听，禁用系统代理仅写到各隔离进程的本地状态目录。

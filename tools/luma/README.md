# Luma 持续开发工具

这些入口从 2026-11-19 的临时工作目录迁入仓库，随项目一起维护。脚本根据自身位置定位 Gateway，不依赖原机器的盘符、旧测试 EXE、旧日志或临时目录。要求使用仓库锁定的已安装依赖；缺少依赖时按仓库 README 准备，不自动升级或安装。

| 入口 | 用途 |
| --- | --- |
| `run-check.py` | 在仓库根运行显式命令，保存 `command.log`、`result.json` 和退出码，隔离子进程的 TEMP/TMP/TMPDIR |
| `build-ui.mjs` | 用实际 Rsbuild 配置构建正常 main 入口；可切换 Luma 合成预览及预览静态构建 |
| `fetch-contracts.py` | 匿名抓取固定官方文档，保留 URL、UTC、HTTP 状态、HTML 原文 SHA-256 和正文 |
| `capture-mcp-models.py` | 经现有管理 API 读取已保存 MCP 账号的模型列表及全部 image/video 详情；不刷新、不生成，导出排除账号和管理身份 |
| `files-browser-check.mjs` | 正常 main 网页到真实管理 TCP 的 Files 联合验证；使用仓库已安装的 Playwright 与独立浏览器会话 |
| `inference-browser-check.mjs` | 正常 main 的精确账号文本测试、显式调用、结果展示及管理／审计联合验证 |
| `mcp-browser-check.mjs` | 正常 main 的已保存账号工具目录／JSON 下载、模型列表／参数、账号切换与错误显示联合验证 |
| `mcp-creation-browser-check.mjs` | 正常 main 的已保存 MCP 账号创作、单次费用确认和原任务结果联合验证；由 Rust 显式宿主启动 |
| `web-worker-check.mjs` | 启动生产 Web worker 和独立 Chromium，经本机 HTTP 夹具核对原结果、HTTP 307 不重放及 SSE 关闭 |
| `check-web-session-redis.ps1` | 独立 tmpfs Redis 验证 Web 会话跨 owner CAS、轮换后重开和迟到回执拒绝，不访问现有数据库 |
| `task-preview-check.mjs` | 任务查询／运维详情真实组件的合成浏览器检查；独立于管理 HTTP 测试 |
| `mcp-creation-preview-check.mjs` | 创作真实表单的模型参数、费用确认、单次提交、取消／结束和宽窄屏合成检查；独立于管理 HTTP 测试 |
| `task-browser-check.mjs` | 正常 main 的取消审计、复制任务 ID、精确账号任务查询及供应商错误联合验证 |
| `normal-main-server.mjs` | 上述浏览器验证的静态页面与透明 loopback API 代理；不伪造管理响应 |
| `check-postgres.ps1` | 隔离 PostgreSQL 持久化验证；`-ServerRestart` 另起 Redis，覆盖 Server TCP、操作系统强杀和原任务追账；`-McpCreation` 覆盖持久管理任务、跨 runtime 竞争和恢复；`-McpRestart` 覆盖 MCP 管理 TCP 和强杀恢复 |
| `check-desktop.ps1` | 复用标准包检查，使用独立数据和空闲端口验证原生桌面拉起 SQLite 后端与控制台 |
| [Luma 预览夹具](../../apps/desktop/e2e/fixtures/luma-media-preview.tsx) | 使用真实组件、主题和样式；只记录请求，不连接管理 API 或服务商；纳入正常 TypeScript 检查 |

所有命令显式指定新的输出目录，遇到同名目录报错，不覆盖旧回执。脚本及夹具永久保留在仓库；日志、缓存和构建产物仍写入操作者指定的临时目录。删除临时目录后可以重新运行。

## Web Cookie 轮换与自动维护

```powershell
cargo test --locked --offline --lib --no-default-features --features runtime-defaults --config 'profile.test.package.gateway.debug=0' -j 4 -- luma_web_session --test-threads=1
node --test scripts/tests/lumalabs-session-receipt.test.mjs tools/luma/web-worker-check.mjs
# 以下两项显式运行：真实 Chromium 使用纯 loopback 合成网页，Redis 使用唯一临时容器。
cargo test --locked --offline --lib --no-default-features --features runtime-defaults --config 'profile.test.package.gateway.debug=0' -j 4 -- luma_web_session_real_browser --ignored --test-threads=1
powershell.exe -NoProfile -File tools/luma/check-web-session-redis.ps1 -Output '<新的输出目录>'
```

正常 Gateway 为已启用的 Web 账号每 5 分钟检查一次原 realm 的认证签名 GET，并接收浏览器管理的 `wos-session` 轮换；普通业务同样保存成功或失败响应中的最终 Cookie。Local 保存于原数据目录 `local/state/console/luma-web-sessions/`，Server 保存于原 console namespace 的 Redis。两者均绑定 provider、credential、原导入材料、站点、realm、UA 和认证设置；模型策略变化不重置会话。该运行材料属于私密数据，不进入媒体 JSON、日志或发布包。

发出请求前持久领取当前会话，同账号其他进程不能并发使用旧 Cookie。新 Cookie 保存失败时仅补写存储；发送后没有回执的记录不会因超时或重启而自动释放，不重放生成，需要恢复原回执或从本机浏览器重新导入新会话。401／Cookie 删除要求重新登录，不把 Cookie 保存期视为内部 token 生命周期。浏览器执行策略和远端 executor 配置仍有效；远端必须支持 `/v1/internal/browser-executor/luma-session-v1/execute`，不会降回旧路径。

## Server MCP 持久任务验证

```powershell
$scratch = Join-Path 'C:/Users/Public/nas_home/AI/GameEditor/linshi' ('luma-mcp-server-' + (Get-Date -Format 'yyyyMMdd-HHmmss'))
powershell.exe -NoProfile -File tools/luma/check-postgres.ps1 -McpCreation -Output "$scratch/postgres"
```

此入口使用带唯一守卫标记的临时 PostgreSQL 和 Redis 容器。合成上游只监听 loopback；测试经过正常管理路由与多个独立 Server runtime 和连接池，不启动 Gateway TCP 服务、不强杀进程。覆盖准备零上游、并发只创建一份原任务、管理令牌／grant 隔离、过期的 `submitting` 不重发以及缺迁移／缺签名密钥时拒绝。不要把它和公共 Agents 的 `-ServerRestart` 强杀验收混为一项。

同一脚本另提供互斥的 MCP 进程入口：

```powershell
powershell.exe -NoProfile -File tools/luma/check-postgres.ps1 -McpRestart -Output "$scratch/mcp-restart"
```

该入口以正常管理 TCP、同一管理令牌和新 grant、隔离 PostgreSQL／Redis、loopback MCP peer 验证两个中断点：已落盘 run ID 的读取被中断，以及生成已发送但回执未返回。等待 PostgreSQL 的实际 90 秒租约，不篡改 SQL 记录或快进时钟。前者只查询原 run 与原会话费用；后者保留 `unconfirmed`，原会话有 ID 时可读取会话费用，但不认作某次生成已确认或费用已结清。两者均不重新开会话或提交生成。这里的 Server 是测试 EXE 启动的正常路由，不是打包 EXE 或供应商真实收费验收。

## MCP 模型合同与创作定向回归

`approve`、`confirm`、`text_input`、`select` 和 Web `Keep` 的离线回归复用正常管理接口与秘密补丁入口，无需真实账号或 credits：

```powershell
cargo test --locked --offline --lib --no-default-features --features runtime-defaults -j 4 -- console::secrets:: protocol::luma_mcp::creation --test-threads=1
npm.cmd --prefix apps/desktop run test -- run src/features/console/LumaMcpCreationReview.test.tsx src/features/console/LumaMcpCreationPanel.test.tsx --maxWorkers=1
```

若只需要测试结果，可在 Cargo 命令中加 `--config 'profile.test.package.gateway.debug=0'` 关闭该测试目标的调试符号；断言和依赖锁定不变，不修改仓库配置。不同编译配置会使用不同缓存，不能保证首次构建更快。

review 答复沿用同一 Rust 创作过滤器；只改审阅时，前端加 `LumaMcpCreationReview.test.tsx` 并选择直接受影响的 Panel／Recovery／Reuse，不重跑目录、其他渠道或全部浏览器流程。启动下方合成预览后，只运行审阅流程：

```powershell
node tools/luma/mcp-creation-preview-check.mjs --origin http://127.0.0.1:4451 --output "$scratch/review-browser" --review
```

`--review`／`--reuse`／`--models` 互斥。审阅的图片由测试在浏览器内拦截返回，不访问 CDN 或真实账号；新增 fixture 与浏览器流程都在仓库。当前只支持声明 `answer_format` 的 review，输入 JSON 由上游按合同校验；完整边界见[审阅回执](../../docs/progress/2026-11-20-luma-mcp-review.md)。

配置变更恢复使用同一创作 Rust 过滤器；只修改恢复逻辑时，前端运行 Recovery／Panel／Reuse 三个相关文件即可，不重复模型与浏览器全套。恢复用例中的 OAuth 刷新只连接测试新建的 loopback peer，不读取或刷新真实账号：

```powershell
$scratch = 'C:/Users/Public/nas_home/AI/GameEditor/linshi/luma-mcp-recovery-新批次'
python tools/luma/run-check.py --output "$scratch/rust" -- cargo test --locked --offline --lib --no-default-features --features runtime-defaults -j 4 luma_mcp::creation -- --test-threads=1
python tools/luma/run-check.py --output "$scratch/ui" -- npm.cmd --prefix apps/desktop run test -- run src/features/console/LumaMcpCreationRecovery.test.tsx src/features/console/LumaMcpCreationPanel.test.tsx src/features/console/LumaMcpCreationReuse.test.tsx --maxWorkers=1
```

模型批次已将已核实的 18 个 image/video 模型接入创作。共享准入配置为 `manifests/luma-mcp-creation-models.json`，Rust 与表单读取同一份文件；更新配置前必须取得对应账号的实际 list/describe 合同。模型目录和原生模型的部分高级参数不等同于完整业务验收。

读取合同需要运行中的本机 Gateway、已保存 MCP 账号和管理凭据。通过进程环境安全提供 `GATEWAY_LUMA_MANAGEMENT_TOKEN`，不要将其写入源码、命令参数或日志；脚本取得普通 secret grant 后只调用目录读取接口。材料过期时返回固定错误标识；请使用账号面板的显式刷新，脚本不会替用户刷新。

```powershell
$scratch = Join-Path 'C:/Users/Public/nas_home/AI/GameEditor/linshi' ('gateway-luma-models-' + (Get-Date -Format 'yyyyMMdd-HHmmss'))
python tools/luma/capture-mcp-models.py --origin http://127.0.0.1:4200 --provider lumalabs-mcp-oauth --output "$scratch/contracts"
# 同一个 provider 有多个 MCP 账号时，另传 --credential <已保存账号ID>。
python tools/luma/run-check.py --output "$scratch/rust" -- cargo test --locked --offline --lib --no-default-features --features runtime-defaults -j 4 luma_mcp::creation -- --test-threads=1
python tools/luma/run-check.py --output "$scratch/ui" -- npm.cmd --prefix apps/desktop run test -- run src/features/console/LumaMcpCreationModels.test.tsx src/features/console/LumaMcpCreationOptions.test.tsx src/features/console/LumaMcpCreationReuse.test.tsx src/features/console/LumaMcpCreationPanel.test.tsx --maxWorkers=1
# 启动下方预览服务后，仅运行新增模型流程。
node tools/luma/mcp-creation-preview-check.mjs --origin http://127.0.0.1:4451 --output "$scratch/browser" --models
```

`--models` 与 `--reuse` 互斥。模型流程检查 Nano Banana 布尔／数字参数、Seedream 自定义尺寸、Ray HDR、原费用确认与宽窄屏；全部调用合成 client，不连接供应商。

## 素材复用定向回归

素材复用批次使用更窄的过滤器，覆盖协议、管理路由、SQLite 重启和相邻创作合同；不重跑只读目录、其他渠道或正式发布全套：

```powershell
$scratch = Join-Path 'C:/Users/Public/nas_home/AI/GameEditor/linshi' ('gateway-luma-mcp-workspace-' + (Get-Date -Format 'yyyyMMdd-HHmmss'))
python tools/luma/run-check.py --output "$scratch/rust" -- cargo test --locked --offline --lib --no-default-features --features runtime-defaults -j 4 luma_mcp::creation -- --test-threads=1
python tools/luma/run-check.py --output "$scratch/ui" -- npm.cmd --prefix apps/desktop run test -- run src/features/console/LumaMcpCreationReuse.test.tsx src/features/console/LumaMcpCreationOptions.test.tsx src/features/console/LumaMcpCreationPanel.test.tsx --maxWorkers=1
# 启动下方预览服务后，仅运行素材复用流程；没有真实上游调用。
node tools/luma/mcp-creation-preview-check.mjs --origin http://127.0.0.1:4451 --output "$scratch/browser" --reuse
```

Local 模式的 UNI-1／GPT Image 2／Ray 3.2 创作从正常账号面板进入，支持任务准备、逐次费用确认、原任务查询和取消／结束。图片参数与编辑、视频重绘／扩画和关键帧边界见[参数子批](../../docs/progress/2026-11-20-luma-mcp-options.md)；基础任务机制见[开发回执](../../docs/progress/2026-11-20-luma-mcp-creation.md)。运维详情可复制 MCP 任务 ID。以下全部使用合成上游，不消费真实 credits；已有 `mcp-browser-check.mjs` 仍只验只读目录，不冒充创作浏览器验收。

```powershell
$scratch = Join-Path 'C:/Users/Public/nas_home/AI/GameEditor/linshi' ('gateway-luma-mcp-creation-' + (Get-Date -Format 'yyyyMMdd-HHmmss'))
python tools/luma/run-check.py --output "$scratch/rust" -- cargo test --locked --offline --lib --no-default-features --features runtime-defaults -j 4 protocol::luma_mcp -- --test-threads=1
python tools/luma/run-check.py --output "$scratch/ui" -- npm.cmd --prefix apps/desktop run test -- run src/features/console/LumaMcpCreationOptions.test.tsx src/features/console/LumaMcpCreationPanel.test.tsx src/features/console/LumaMcpToolsPanel.test.tsx src/features/console/LumaOAuthPanel.test.tsx src/features/console/OperationsLumaTask.test.tsx src/features/console/lumaMcp.test.ts --maxWorkers=1
```

## MCP 创作正常网页联合验证（L1／L6）

需要仓库已安装的 Playwright、Edge 和正常 main 构建。以下在仓库根的同一个终端执行；不使用个人浏览器 profile，也不需要另开服务：

```powershell
$scratch = Join-Path 'C:/Users/Public/nas_home/AI/GameEditor/linshi' ('gateway-luma-mcp-creation-ui-' + (Get-Date -Format 'yyyyMMdd-HHmmss'))
node tools/luma/build-ui.mjs --output "$scratch/main-web"
$env:GATEWAY_LUMA_MCP_CREATION_UI_BUILD = "$scratch/main-web"
$env:GATEWAY_LUMA_MCP_CREATION_UI_ROOT = "$scratch/host"
python tools/luma/run-check.py --output "$scratch/rust-host" -- cargo test --locked --offline --lib --no-default-features --features runtime-defaults -j 4 protocol::luma_mcp::creation_tests::browser_test::luma_mcp_creation_normal_main_browser -- --ignored --exact --nocapture
```

显式宿主启动正常管理路由、独立 SQLite 和随机 loopback MCP peer，再启动无外网请求的独立 Edge。用例执行正常登录、精确账号准备、费用确认、单次生成，关闭重开账号面板后按原任务 ID 查询结果和会话费用。浏览器核对用户可见状态／链接及管理请求；宿主从同一 SQLite 审计行独立读回原任务与 run，并核对精确账号、上游调用次数和配置 revision 不变。不会打开资产链接。

成功需同时满足 `host/browser/result.json` 与 `host/completed.json` 为 `passed`，且 Rust 为 **1 passed**。该切片仅证明合成 MCP 图片流程，不包含真实收费、远端业务取消、媒体下载／解码、原生桌面、新包或 Server 强杀。

## 隔离构建和预览

从 Gateway 根目录执行。此工作站的一次性产物使用约定的 `linshi`；其他工作站可更换 `$scratch`，不必建立同样的机器目录结构。

```powershell
$scratch = Join-Path 'C:/Users/Public/nas_home/AI/GameEditor/linshi' ('gateway-luma-' + (Get-Date -Format 'yyyyMMdd-HHmmss'))

# 正常 main 入口；移除发布插件、关闭清理，不修改现有 dist 或运行实例。
node tools/luma/build-ui.mjs --output "$scratch/main-web"

# 合成预览，仅监听本机，默认 30 分钟后关闭，也可 Ctrl+C 停止。
node tools/luma/build-ui.mjs --preview --output "$scratch/preview" --port 4451

# 只构建合成页面，不启动服务器。
node tools/luma/build-ui.mjs --preview-build --output "$scratch/preview-build"
```

预览可检查分层、扩画、重绘表单和费用确认。执行后的请求记录在页面内存 `window.syntheticRequests`；页面显示合成错误是预期结果，不代表真实生成失败。端口占用时直接报错，不停止其他服务。`--lifetime-seconds` 可设置 1–86400 秒的预览生命周期。

预览还包括任务查询和取消任务的审计详情。另一个终端可运行下列脚本，检查三次显式查询的未知／零／实际 credits、关闭后清空及宽窄屏；它只使用组件合成响应，不能作为管理 TCP 或供应商验收。

```powershell
node tools/luma/task-preview-check.mjs --origin http://127.0.0.1:4451 --output "$scratch/task-browser"
node tools/luma/mcp-creation-preview-check.mjs --origin http://127.0.0.1:4451 --output "$scratch/mcp-creation-browser"
```

这些 Web 产物不是原生包。需要 EXE 时使用 [devtest 流程](../../docs/devtest-build.md)，不能把合成预览作为正常产品页面打包。

## Server 进程强杀与追账（L4／L6）

```powershell
$scratch = Join-Path 'C:/Users/Public/nas_home/AI/GameEditor/linshi' ('luma-server-restart-' + (Get-Date -Format 'yyyyMMdd-HHmmss'))
powershell.exe -NoProfile -File tools/luma/check-postgres.ps1 -ServerRestart -Output "$scratch/server"
```

需要已缓存的 `postgres:16-bookworm` 和 `redis:7-alpine`。测试只使用两个带本轮唯一标签的 tmpfs 容器、随机 loopback 端口、真实初始化 DDL 和合成 Luma 服务，不读现有数据目录，不会向供应商发送请求。测试通过普通 `POST /v1/images/generations` 走 Server 路由，在已确认原任务 ID 且首个查询仍挂起时强杀本轮子进程，再由第二个子进程读取原配置和数据库。恢复阶段只推进 worker 的测试时钟，不改 SQL 记录，不自动重发生成。此入口不是新打包 EXE、真实账户或实际等待 15 分钟的验证。

`-ServerRestart` 同时覆盖路由文档和 PostgreSQL 托管凭据。托管场景使用空路由文档，通过 PostgreSQL 的 model alias、provider account 和 credential 走正常入口；恢复前更换供应商 Key 的负对照不得再发任何上游请求。生产 Server 需要已配置且在恢复节点保持一致的 `GATEWAY_API_KEY_SECRET`，才能为新的托管任务创建持久追账标记；它只用于生成独立用途的绑定标签，不随请求发到 Luma。恢复只读有效、未归档的原账号和原 credential。原 Key、归属或签名密钥不匹配时，旧标记不能改绑或回退到路由文档。没有该签名密钥时，原公共请求照常运行，但 DB 托管任务不会自动跨进程补账。

## Web worker 的真实浏览器回环验证（L2）

账号 `User-Agent` 的 Rust → 本机／远端 worker 传递、未配置时的兼容行为，运行 `cargo test --locked --offline --lib --no-default-features --features runtime-defaults -j 4 upstream::lumalabs -- --test-threads=1`。下方真实 Edge 夹具同时检查所有业务请求实际携带指定的 User-Agent；这项通过不代表 Cookie 轮换已持久保存。

使用已安装的 Playwright Core 和本机 Edge／Chromium，显式运行下列命令。生产 CLI 会启动自己的临时浏览器和 context；不使用个人 profile、保存的账号或已登录页面。可用 `LUMALABS_BROWSER_EXECUTABLE_PATH` 显式指定本机浏览器。

```powershell
$scratch = Join-Path 'C:/Users/Public/nas_home/AI/GameEditor/linshi' ('luma-web-worker-' + (Get-Date -Format 'yyyyMMdd-HHmmss-fff'))
python tools/luma/run-check.py --output "$scratch/edge" -- node --test tools/luma/web-worker-check.mjs
```

两项必须都通过：正常路径只返回与原 POST 对应的完成产物；307 路径返回 `lumalabs_action_redirect`，不请求重定向目标。两条路径都只提交一次，并在结束时关闭 SSE。本测试只验证生产 worker 与 Chromium 的实际行为，不能证明当前 Luma 官网的登录会话、模型动作、资产或真实收费。

## 相关验证与回执

按本次改动选择相关命令，不要求每次执行全部范围。`run-check.py` 只负责记录与临时目录隔离，执行前仍须检查命令自身的行为；它不是沙箱，也不自动脱敏日志。不要将真实 Key、Cookie 或 token 放进命令参数或待归档日志。

```powershell
python tools/luma/run-check.py --output "$scratch/agents" -- cargo test --locked --offline --lib --no-default-features --features runtime-defaults -j 4 luma_agents --no-fail-fast -- --test-threads=1
python tools/luma/run-check.py --output "$scratch/files" -- cargo test --locked --offline --lib --no-default-features --features runtime-defaults -j 4 luma_agents::files --no-fail-fast -- --test-threads=1
python tools/luma/run-check.py --output "$scratch/inference" -- cargo test --locked --offline --lib --no-default-features --features runtime-defaults -j 4 luma_inference --no-fail-fast
python tools/luma/run-check.py --output "$scratch/typecheck" -- node apps/desktop/node_modules/typescript/bin/tsc --noEmit -p apps/desktop/tsconfig.json

npm --prefix apps/desktop run test -- run src/features/console/LumaMediaProbeDialog.video-controls.test.tsx src/features/console/lumaVideoProbeDraft.test.ts --maxWorkers=1
npm run test:effective-lines --prefix scripts
npm run check:effective-lines --prefix scripts
git diff --check
```

Files 管理测试复用进程内两个账号操作槽，以上 Agents／Files 命令使用 `--test-threads=1` 避免不同测试之间争抢槽位；Cargo 编译并发仍为 `-j 4`，不修改生产限流。

测试返回失败时 runner 返回同一失败码；命令无法启动时返回 127 并保存诊断。它不会选择日期固定的旧测试 EXE，也不会将失败回执改成成功。Rust、前端、真实账号、费用和原生包的验收分别记录。

## Files 正常网页联合验证

需要已经安装的项目依赖与 Edge；可用 `--channel chrome` 选择本机 Chrome。不会安装依赖、下载浏览器或复用用户 profile。合成宿主是 `src/upstream/luma_agents/files_browser_host.rs` 中的显式 opt-in 测试，不在普通回归中自动启动。

两个终端使用同一个新的 `$scratch`。先在终端 A 构建正常网页并启动 Rust 宿主；编译结束后生成 `$scratch/host/ready.json`，宿主继续等待浏览器。必须保留完整测试名，不能对短名称加 `--exact` 后把零测试退出当成功。

```powershell
$scratch = Join-Path 'C:/Users/Public/nas_home/AI/GameEditor/linshi' ('gateway-luma-files-browser-' + (Get-Date -Format 'yyyyMMdd-HHmmss'))
node tools/luma/build-ui.mjs --output "$scratch/main-web"
$env:GATEWAY_LUMA_FILES_UI_ROOT = "$scratch/host"
python tools/luma/run-check.py --output "$scratch/rust-host" -- cargo test --locked --offline --lib --no-default-features --features runtime-defaults -j 4 upstream::luma_agents::files_browser_host::luma_agents_files_normal_main_browser_host -- --ignored --exact --nocapture
```

终端 B 从仓库根执行（将 `$scratch` 设为终端 A 的实际目录）：

```powershell
python tools/luma/run-check.py --output "$scratch/browser-run" -- node tools/luma/files-browser-check.mjs --host "$scratch/host" --ui "$scratch/main-web" --output "$scratch/browser"
```

脚本完成正常登录、已保存账号操作、小文件上传/状态/复制、分页、大文件签名上传与 TTL/用户标记、取消删除/确认删除和账号切换清空；保存宽窄屏截图、无凭据的请求摘要和 AX 快照。所有上游请求由 Rust 测试层重映射到不转发外网的本机夹具。三字节和 8 MiB+1 字节输入是二进制运输夹具，不代表有效媒体摄取或真实供应商验收。

**成功需要同时满足**：`browser/browser-result.json` 为 `passed`、`host/completed.json` 为 `passed`，且终端 A 为 **1 passed**。后端独立核对九次上游调用、字节、精确账号、签名 PUT 无凭据，以及普通 Key 余额和配置 revision 不变。浏览器失败会发送 `abort`，让宿主收尾后失败；若手动中断浏览器，向 `host/command.json` 写入 `{"action":"abort"}` 可结束本轮宿主，最长自动等待 30 分钟。关闭浏览器、代理和后端后才认定资源收尾。

## Inference 正常网页联合验证

环境、独立浏览器和双终端方式同上。终端 A 构建正常网页并启动显式测试宿主：

```powershell
$scratch = Join-Path 'C:/Users/Public/nas_home/AI/GameEditor/linshi' ('gateway-luma-inference-browser-' + (Get-Date -Format 'yyyyMMdd-HHmmss'))
node tools/luma/build-ui.mjs --output "$scratch/main-web"
$env:GATEWAY_LUMA_INFERENCE_UI_ROOT = "$scratch/host"
python tools/luma/run-check.py --output "$scratch/rust-host" -- cargo test --locked --offline --lib --no-default-features --features runtime-defaults -j 4 protocol::luma_inference::wire_tests::browser_host::luma_inference_normal_main_browser_host -- --ignored --exact --nocapture
```

等 `$scratch/host/ready.json` 出现后，在终端 B 使用相同 `$scratch` 执行：

```powershell
python tools/luma/run-check.py --output "$scratch/browser-run" -- node tools/luma/inference-browser-check.mjs --host "$scratch/host" --ui "$scratch/main-web" --output "$scratch/browser"
```

脚本正常登录后进入 exact 账号，配置并关闭弹窗，核对没有发送请求；重新配置后显式运行一次，检查精确账号、模型别名、提示词和页面可见的答案，并保存宽窄屏截图。宿主独立核对一次 Chat 请求、映射后的模型、凭据隔离、2/3/5 token 审计、普通 Key 余额及 revision。管理文本测试审计沿用现有语义，记录映射后的模型；测试结果保留用户选择的别名。

同样必须同时看到浏览器 `passed`、宿主 `completed.json` 为 `passed` 和 Rust **1 passed**，不能只以页面 HTTP 200 判定成功。失败发送 `abort`，手工中断时的收尾方式同 Files。此入口只访问合成上游，不证明销售开通、真实模型资格、限速、费用或原生包。

## MCP 只读目录正常网页联合验证

使用相同的隔离环境与双终端方式。终端 A 构建正常网页并启动显式测试宿主：

```powershell
$scratch = Join-Path 'C:/Users/Public/nas_home/AI/GameEditor/linshi' ('gateway-luma-mcp-browser-' + (Get-Date -Format 'yyyyMMdd-HHmmss'))
node tools/luma/build-ui.mjs --output "$scratch/main-web"
$env:GATEWAY_LUMA_MCP_UI_ROOT = "$scratch/host"
python tools/luma/run-check.py --output "$scratch/rust-host" -- cargo test --locked --offline --lib --no-default-features --features runtime-defaults -j 4 protocol::luma_mcp::browser_host::luma_mcp_normal_main_browser_host -- --ignored --exact --nocapture
```

等 `$scratch/host/ready.json` 出现后，在终端 B 使用同一个 `$scratch`：

```powershell
python tools/luma/run-check.py --output "$scratch/browser-run" -- node tools/luma/mcp-browser-check.mjs --host "$scratch/host" --ui "$scratch/main-web" --output "$scratch/browser"
```

脚本从正常登录进入已保存账号的授权面板；展开面板不调用目录或重新登录 OAuth。显式读取工具、下载 JSON 快照、读取模型和 image/video 参数，检查未知费率、audio 详情禁用、账号切换清空、空列表和分类错误，并保存宽窄屏截图。下载文件另存为 `browser/catalog-snapshot.json`，核对输入／输出 schema、annotations、读取时间与连接信息排除；下载不会增加管理或上游调用。工具说明仅作为文本展示；脚本不调用生成工具。上游在 Rust 测试层逐请求映射到本机合成 MCP，生产端点仍固定。

成功需要浏览器 `passed`、宿主 `completed.json` 为 `passed` 和 Rust **1 passed**。宿主独立核对 18 次 POST、6 次 DELETE、六个 session 的绑定、精确账号凭据及配置 revision 不变；合成 DELETE 仅证明客户端发出收尾请求，不证明真实远端业务已释放。失败／手动中断的 `abort` 收尾方式同 Files，最长等待 30 分钟。此入口不刷新真实 token，也不证明真实模型资格、MCP 创作或原生包。

## 单任务查询正常网页联合验证

需要已有项目依赖及 Edge；使用独立浏览器，不使用真实账号。终端 A 构建正常 main 并启动显式宿主：

```powershell
$scratch = Join-Path 'C:/Users/Public/nas_home/AI/GameEditor/linshi' ('gateway-luma-task-browser-' + (Get-Date -Format 'yyyyMMdd-HHmmss'))
node tools/luma/build-ui.mjs --output "$scratch/main-web"
$env:GATEWAY_LUMA_TASK_UI_ROOT = "$scratch/host"
python tools/luma/run-check.py --output "$scratch/rust-host" -- cargo test --locked --offline --lib --no-default-features --features runtime-defaults -j 4 upstream::luma_agents::task_browser_host::luma_agents_task_normal_main_browser_host -- --ignored --exact --nocapture
```

宿主先通过现有公共生成／取消流程形成一条本地审计，供应商端只访问合成 loopback。等 `$scratch/host/ready.json` 出现后，终端 B 使用相同 `$scratch`：

```powershell
python tools/luma/run-check.py --output "$scratch/browser-run" -- node tools/luma/task-browser-check.mjs --host "$scratch/host" --ui "$scratch/main-web" --output "$scratch/browser"
```

脚本从正常登录进入运维详情，复制任务 ID，再进入原账号显式查询未知／0／12.5 credits；覆盖无效输入不发送、401 不注销管理会话、429／错任务身份拒绝、关闭／切换账号清空、原取消审计不被查询改写及宽窄屏。页面和 API 响应不注入模拟结果，只有上游由 Rust 测试层映射到合成服务。

成功须同时满足浏览器 `browser-result.json` 与宿主 `completed.json` 为 `passed`，且 Rust 为 **1 passed**。宿主独立核对六次查询均为同 UUID 的 GET、精确账号和凭据隔离，审计／配置 revision／普通 Key 余额不变；创建审计的两次合成调用另计。失败发送 `abort`，手动中断的收尾方式同 Files；宿主约 28 分钟未收到命令会关闭并报错。该验证不证明真实供应商业务、自动追账、强制崩溃恢复或原生包。

## 保存 MCP 当前工具合同

2026-11-20 已使用新保存账号通过正常管理 API 取得[26 项完整合同及当前模型参数](../../docs/progress/2026-11-20-luma-mcp-live-contract.md)。这些长期材料已归档到仓库；需要更新时仍使用下面的正常产品入口，不依赖一次性调查脚本。

在正常界面打开已保存的 MCP 账号，点击“读取工具目录”，成功后点击“导出工具目录”。下载 `luma-mcp-catalog-<UTC>.json`，包含固定 resource、协商协议版本、读取完成时间及完整已接收目录的名称、说明、输入／输出 schema 与 annotations。空目录可以保存；缺省 annotations 为 null，不推断安全性或幂等性。导出本身不发请求。

这是规范化的目录快照，不是原始 HTTP 报文。连接 token、管理 grant、账号 ID、revision、session header 和工具顶层 `_meta` 不加入快照；schema／说明中的供应商原文保持数据语义，不能保证这些任意文本天然不含敏感信息。作为项目合同材料使用前检查其内容，并放到相应 `docs/evidence/luma/` 子目录；不要把长期合同只留在 `linshi`。快照不执行工具，也不证明生成结果、费用或远端取消已经验收。

重新读取会立即清空旧快照；取消、失败、关闭、账号／连接／授权／revision 变化后不能导出旧范围结果。合成开发验证使用上面的 MCP 浏览器入口，不要求真实账号。

## 大文件与持久追账回归

以下命令只使用合成上游，不执行真实生成。大文件测试通过流式生产器检查超过 64 MiB 的真实管理上传、5 GiB 输入的背压和取消，不在磁盘生成 5 GiB 文件。

```powershell
python tools/luma/run-check.py --output "$scratch/rust-luma" -- cargo test --locked --offline --lib --no-default-features --features runtime-defaults -j 4 luma -- --test-threads=1
powershell.exe -NoProfile -File tools/luma/check-postgres.ps1 -Output "$scratch/postgres"
```

PostgreSQL 入口要求 Docker 和已有 `postgres:16-bookworm` 镜像。它创建随机名称、仅本机随机端口、tmpfs 数据的独立容器，装载仓库真实 DDL；运行标记核对后才允许测试访问，结束时只停止本次创建的容器。不会连接现有 5432，也不安装依赖或访问真实账号。验证审计 checkpoint、终态保留、两个独立连接的竞争领取、过期 running 记录补查和晚到 finalizer 的费用保留；这不等于完整 Server／Redis 业务验收或操作系统级 kill 测试。

日志、临时数据由 `--output`／`-Output` 指定，持续脚本与测试源码都在仓库；清空临时目录不影响接手者继续开发。

## 原生包与隔离启动

按标准 devtest 流程生成包后，对实际包运行以下命令。`$release` 指向新版本目录；输出目录必须不存在。桌面检查拒绝已运行的同包进程，使用独立数据目录内的 `connection.json` 设置随机本机端口，不占用或停止既有 4200 实例。

```powershell
powershell.exe -NoProfile -File tools/smoke-gateway-local-storage.ps1 -ReleaseDir "$release" -EvidenceRoot "$scratch/local-smoke"
powershell.exe -NoProfile -File tools/luma/check-desktop.ps1 -ReleaseDir "$release" -Output "$scratch/desktop-smoke"
```

第一条验证后端启动、重启、Key／余额保留、退款和轮换，并确认本地模式不连接 PostgreSQL／Redis。第二条复用 `smoke-gateway-ui-release.ps1 -LaunchUi`，核对桌面与其子进程来自同一个包、SQLite 就绪、`/ui/` 返回 200、数据写入指定目录以及进程收尾。它不自动点击 Luma 业务页面，不读取真实账号、不执行媒体生成，也不代替原生业务手测。结果保存于各输出目录的 `result.json`。

## 刷新公开合同

```powershell
python tools/luma/fetch-contracts.py --page files --page files-create --page files-complete --page llm --output "$scratch/public-contracts"
```

可选页面为 `files`、`files-create`、`files-complete`、`layering`、`llm`、`video-edit`、`video-reframe`、`usage`、`create`；省略 `--page` 抓取全部九页。每页最多读取 2 MiB、网络超时 30 秒；抓取失败写入 `sources.json` 并返回非零状态，不以错误页面当成功合同。它只使用标准库，不登录、不执行网页脚本、不调用生成接口。

一次性 `measure-*.mjs`、旧日期的 `source-snapshot.mjs`／`final-source-check.mjs` 不作为持续开发入口：规模检查使用仓库有效行工具，旧 before/after 和源码哈希已归档。早期重复预览／提取脚本由本目录入口取代；`trace_stack.py` 是已结束堆栈调查的专用脚本，保留调查结果，不作为正常测试依赖。

接手先读 [Luma 剩余工作](../../docs/providers/luma-development-status.md) 和 [归档说明](../../docs/evidence/luma/README.md)。

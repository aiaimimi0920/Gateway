# 服务商扩展桌面手测包交付

## 后续原生启动检查（2026-11-12）

证据：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-native-ui-20261112-152133/result.json`。复用已冻结包，没有重建或修改包。

在新临时目录启动 SQLite 后端，使用随机 loopback 端口 52584；UI 使用独立 `GATEWAY_DATA_DIR` 和 `existing` 连接配置，不启动 sidecar、不连接旧 4200 实例。新包 UI 进程创建了原生 Gateway 窗口，CUA inventory 确认其可执行文件路径属于新包；但两次读取窗口都返回 `window is minimized`，工具未提供该错误要求的 activation API，因此原生画面与交互验收仍受阻，不能将进程存在当成界面加载成功。初次隐藏启动的测试 UI 已定向停止，第二次普通启动的 UI 正常关闭。

独立 HTTP 检查确认 `/ui/` 返回 200，HTML 引用的 4 个 JS 和 2 个 CSS 资源均为 200；这只证明静态载荷可用，不证明 WebView 渲染、鉴权或业务交互。新后端通过 drain 正常退出，旧 PID 43052/58788 保留，模型调用为 0。没有修改生产源码或现用数据，也没有因自动化观察失败更改窗口生产逻辑。

日期：2026-11-12。类型：desktop devtest，不是正式 Release。

## 交付与来源

- 包目录：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/release/Gateway/gateway-devtest-provider-expansion-20261112-132816`。
- 桌面入口：包内 `gateway-ui.exe`；后端：`gateway.exe`。
- 使用官方 `tools/package-gateway-devtest.ps1 -Edition desktop -Jobs 4 -VersionId gateway-devtest-provider-expansion-20261112-132816`，锁定依赖，devtest profile，增量构建，无 LTO。
- 来源凭据：包内 `gateway-build-provenance.json`；指纹 `c7af5ac69d85139a1294cb23e4fc5bdcf52b5081065257f41433effc77a6e4b1`，6684 个来源文件，`sourceTreeDirty=true`。包含构建时整个工作树，不能声称只包含本工作线修改。
- `gateway.exe` SHA256：`871e335c82dd0401c5ba4f44ccf3d2cbeca46f99b0eef18c6c77e13026f223be`。
- `gateway-ui.exe` SHA256：`c28ce7e7fd35334b7ff86d93f47a4d1e49400e2a71c0358d1110cbd167723b69`。
- 本回执及计划更新发生在包冻结后，不属于上述来源指纹；不修改凭据、不为包含交付回执重复构建。

## 实际验证

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-provider-delivery-20261112-132816`。

最终 `package-3.log` / `package-exit-3.json` 记录成功退出；官方包完整性检查在 staging 和最终目录均通过。交付前再次核对两个 EXE 的 SHA256，与凭据一致。

使用 `tools/smoke-gateway-local-storage.ps1` 对实际新包、随机 loopback 端口和临时数据执行隔离运行，`local-runtime/result.json` 为 `passed=true`：

1. 空路由下 SQLite readiness。
2. 耗尽 Key 在上游执行前拒绝。
3. 实际新包进程重启后 Key 和余额保持。
4. 路由失败仅退款剩余额度，不修改购买总量。
5. Key rotation 保余额并废止旧 token。
6. 零 Redis / PostgreSQL 连接。

`modelCalls=0`，`processesDrained=true`。交付前进程核查只发现旧包的 gateway PID 58788、gateway-ui PID 43052；旧实例保留，新包测试进程已结束。未启动新桌面窗口进行人工交互验收。

## 构建失败与恢复

第一次失败来自旧 CPA BSSL 缓存引用的临时 `build/config.rs` 已不存在。没有删除缓存、升级依赖或修改生产源码；使用仓库支持的 `GATEWAY_CPA_BSSL_BUILD_ROOT=C:/Users/Public/nas_home/AI/GameEditor/linshi/cpa-bssl-devtest` 与 `CARGO_NET_OFFLINE=true`，从固定归档重新准备。

第二次编译完成但来源校验报 `Source changed during build; no devtest receipt published.`，未绕过检查或交付该次产物。第三次完整官方流程利用 Cargo 缓存成功，才发布本包。失败日志均保留，不将失败计为通过。

## 范围与未验边界

用户“同意”仅用于本次构建所需的 desktop dist 生成文件清理；未删除旧包、修改旧实例、凭据或数据库，未创建便携 `.ng`，未提交或推送。当前旧实例仍在运行，手测新版本时须避免默认端口 4200 冲突。

本轮没有生产源码修改，仅完成构建交付和文档记录。此前 Grok/Qwen 真实本地 TCP 与普通余额 Key 联合定向检查见对应回执，不重复运行。新增模板仍为 17，未新增整卡 `offline_ready`；P50/P60/P70 不因本包通过而整体完成。真实供应商、付费调用、桌面完整交互和正式发布流程未执行。

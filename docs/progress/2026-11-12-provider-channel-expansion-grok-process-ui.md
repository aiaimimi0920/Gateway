# R03：Grok 正常网页保存、宿主进程重启与恢复后调用

日期：2026-11-12（Asia/Shanghai）。工作区 `C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`；HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`。仅改 opt-in 测试宿主并新增 local_owner.rs；无生产实现或前端改动。

## 同一次跨进程链路

正常 main.tsx/Edge 页面 → 正常管理 TCP/Grant/CAS → 生产 new_local 文件配置 authority + 本地 SQLite → 首个宿主正常退出 → 新测试宿主进程从原目录恢复 → 正常网页精确账号 probe → 合成身份 GET/WS → 正常结果页面及脱敏审计。

- **保存进程 PID 10900**：通过网页编辑 grok-pipeline/exact，读取并保存新 SSO/Cookie/UA。完整 typed 配置与预期一致，其他账号未变，revision 为 `r1-07e962a7f119`；上游调用和审计均为 0。
- 发送本轮专用 save-exit 命令，宿主先 graceful shutdown/join、关闭 SQLite/peer 再写 saved.json 并正常退出。代理也正常退出。外部复核进程及旧监听都不存在后才启动第二进程。
- **恢复进程 PID 30500**：显式 resume 同一隔离目录；代码要求存在保存成功回执、PID 不同、未完成过，使用生产 new_local/recover_startup 读取磁盘 authority，恢复 revision 必须与保存回执相同。浏览器 probe 前完整配置已匹配，上游调用仍为 0。
- 进入第二个服务的正常网页并重新登录，未打开材料编辑器、未重复导入或保存。选择 exact/public-alias，仅加入一条临时合成题并运行一次；不保存策略、不启用自动测试。
- 整段 HTTP 记录只有一次配置 PUT（首进程）、一次 probe POST（第二进程），两次正常登录/Grant；probe 仅一次身份 GET 和一次 WS Upgrade。实际出站恢复后的完整 Cookie/UA，无 Authorization；三个客户端事件、fast 模式及私有 prompt 原文均断言通过。
- 正常集中 IQ 页显示 `public-alias / exact`、1/1 题正确、合成测试指数 100，额度与比值为 `—`。100 不代表真实模型智力、质量或权益。
- 唯一 completed 审计归属 exact、native model 为 grok-chat-fast、consoleModelProbe=true、accessKeyId=null；usage 三项保持未知，凭据/UA/私有 prompt 不进入审计。

这是同一轮保存材料跨真实 OS 测试进程退出/启动后的网页和上游联合，不再仅是同进程对象重建，也不是拼接之前独立测试的证据。两个进程使用同一冻结测试 EXE，但它仍是测试 harness，不是 gateway.exe 或 gateway-ui.exe 成品包；AppState/auth 仍由测试 support 构造，未调用完整生产 build_app_state/全部后台 worker。

## 代码与检查

`src/upstream/client/grok_web_pipeline_tests/browser_host.rs` 改用真实本地 authority，不再启用 FakeRedisBackend writer；有效行 182→212。新增 `browser_host/local_owner.rs` 54 有效行，专门负责隔离目录初始创建与跨进程恢复约束。默认仍 ignored，显式新根启动；恢复必须设置 GATEWAY_GROK_PROBE_UI_RESUME=1。没有新增生产网络例外，合成 wire task scope 缺失不会回退真实平台。

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-grok-process-ui-20261112`。

| 检查 | 结果 |
| --- | --- |
| Rust 构建 | locked/offline、runtime-defaults、4 jobs，退出 0 |
| 公共 Grok pipeline/TCP | 6 passed；浏览器宿主默认 ignored |
| 管理 probe 相邻 | 2 passed |
| opt-in 宿主 | 保存、恢复两次进程执行各 1 passed；不把同名测试计成两个新增测试 |
| 前端 | 正常 main 隔离构建通过；输入与此前67项测试/类型检查版本相同，不重复未变化检查 |
| checker / ratchet / 格式 | 31 passed，官方无报告 ratchet、rustfmt、限定 whitespace 通过 |
| 资源 | 两宿主及两代理均正常退出，四个监听最终均无残留 |

Cargo compiler-artifact 指向的实际 EXE 复制到证据目录并核验两端 hash 后执行；`tests.exe` SHA256：`cc9c6833eb0452d53b079a1a054094d14b3eb96a3cfaaea5c915b6ba577b6de7`。

最终前端及本批 Grok 直接输入未变；全 src 清单仅 `src/token_refresh/tests/nous_probe.rs` 有并行漂移，记录在 input-drift.json。结论绑定冻结测试副本，不宣称其他工作线最新输入全验。

主要证据：`material-saved.json`、`first-process-exited.json`、`ready-save.json`、`ready-resumed.json`、`material-restored-before-probe.json`、`browser-http.jsonl`、`browser-result.txt/jpg`、`browser-runtime-v1/saved.json`、`browser-runtime-v1/completed.json`、`host-save.log`、`host-resume.log`、`resource-final.json`。隔离根保留合成配置/归档/SQLite；TEMP/TMP 也位于本批 linshi，无真实 .ng 操作。

## 剩余边界

已退役 Grok 正常网页材料保存→真实 local→测试宿主进程重启→恢复材料实际调用这一限定空白。Qwen 不因此自动获得相同跨进程 UI 证据。未验证硬杀/断电、成品 EXE 启动/包、真实上游 TLS/WAF/SSO/challenge、其他模式权益、多轮/工具/模态、Server/PG 或普通余额 Key 扣额/退款。

新增模板仍17，无新增整卡 offline_ready，P50/P60/P70 未完成。没有真实凭据/供应商请求、在线实例变更、出包、提交或推送。

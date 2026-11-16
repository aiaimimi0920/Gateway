# Udio 正常网页与管理 TCP 音乐任务联合验收

日期：2026-11-11（Asia/Shanghai）。状态：限定离线联合链完成；承接 [Udio 精确账号音乐任务测试](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-11-provider-channel-expansion-udio-probe.md)，不是整卡或总计划完成。

## 改动与运行链

仅增加 `src/upstream/client/tests/udio_probe/browser_host.rs` opt-in 宿主及父模块两行接线，不修改生产代码。父模块有效行 272→274，新宿主 116 行。复用已有 Udio fixture、精确账号/模型、合成 remote executor 和封闭审计断言，没有另建生产发送逻辑。

实际链路为：正常 `src/main.tsx` 的独立静态构建 → Edge 新建测试标签页 → 同源 loopback 透明代理 → 生产 `build_router` 的真实管理 TCP/Grant → 普通 Udio bound remote sender → 合成执行器。代理不制造 Grant、不改 Host/Origin/认证/body。fixture 关闭内嵌 management UI，因此不能称为内嵌网页或原生包验收。

## 已观察的联合行为

1. 正常页面使用合成管理 token 登录，沿正常 secret-access 接口取得 Grant，打开 `udio-pipeline / exact` 的音乐任务测试，选择配置别名 `public-udio-music`。
2. 输入合成提示词后打开费用确认，再点击取消。取消后新刷新的宿主观察快照为 **0 upstreamCalls**，代理日志为 **0 probe POST**；正常登录及 Grant 的 POST 不误算生成。
3. 再次点击运行，页面重新显示精确账号/模型及“扣额未知、仅元数据、不下载、不恢复健康、取消不保证退款”的费用提示。明确确认后只有 **1 probe POST / 1 executor call**。
4. 正常页面实际显示：`Udio 任务终态已确认；model=udio-fixture-native；2 首歌曲元数据；音频未验证，未恢复模型健康；供应商扣额未知。` 该观察来自 CUA 正常界面，不是直接调 API 或注入组件回调。
5. 宿主正常退出前断言请求别名、解析 native model、exact 账号、`waitAudio=true`、唯一完成审计及 `audioVerified=false/supplierCharge=null`。审计没有凭据、歌词、歌曲 URL 或远端任意消息。普通 Gateway Key 余额仍2，健康表仍为空，完整配置及 revision 均未变。
6. 管理 TCP graceful shutdown/join、SQLite close、fixture 所持执行器 shutdown 和合成 peer task 收尾后才写 `completed.json`。静态代理通过 stdin 正常停止；核查两个自建 PID 与两个对外 loopback 监听均已消失。未停止其他进程，未删除证据及测试数据。

## 验证与来源

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-udio-probe-ui-20261111`。

- `build.json`：锁定、离线、runtime-defaults/API-only 的 lib test 编译成功，722.672秒，记录输入前后无变化。固定 Cargo 实际输出为 `green.exe`，SHA256：`45a835845a8c6f01d9d069161bccdadea33456d46b3060f4db622e8075c607f7`。
- `focused.json`：原8项核心测试通过，新浏览器宿主默认 ignored。随后 `ui-host.json` 显式运行该宿主，**1 passed**；按完整名合计本轮 **9 passed / 0 failed / 0 未执行核心项**。不累加上一轮172项为本轮执行数量。
- `browser-cancel.json`、`browser-confirmed.json`、`browser-http-r2.jsonl`、`browser-runtime-v1/completed.json`：费用取消、确认、正常管理路径和后端终态证据。日志文件名沿复用 helper，不代表另跑了两轮浏览器。
- `ui-reused.json`：上一轮97项前端测试、类型检查及正常main构建的全部UI输入hash仍一致，因此复用而非重复执行。实际网页来自该固定 `safe-web` 构建，不覆盖共享dist。
- 两个Rust文件官方formatter通过，checker **31/31**、官方ratchet、范围UTF-8无BOM及`git diff --check`通过；继承生成报告保留原bytes，不改policy/baseline/exceptions。
- `resource-final.json`：自建宿主/代理PID和监听终态；`final-receipt.json`：来源、计数和复用边界汇总。无生产代码修改，本批不重复生产feature检查或假发布；上一轮开启/关闭Udio的生产检查仍单独保留。

## 未覆盖与下一步

本批只退役“正常 Udio 网页 → 管理 TCP → 合成 bound executor”的单次任务测试联合缺口。远端不是 Udio 平台，也不是本地Node/browser worker；没有真实生成、费用、当前会话/模型/wire或平台内有效播放证据。取消的是尚未提交的费用确认，不外推为已受理任务的远端取消。

真实平台及权益留到集中录制；本地worker/远端lease、Server/独立进程/原生包与其他渠道、P50/P60/P70继续。仍17个新增模板、无新增整卡`offline_ready`。未读真实凭据、调用供应商、操作容器或现有实例、出包、提交或推送。

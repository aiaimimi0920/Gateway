# 服务商扩展：FreeBuff 正常 UI 与真实本地管理闭环

日期：2026-11-06。状态：R06/W04 的本地管理子链定向通过，**不是完整渠道、P50 或总计划完成**。接续 [正常 UI 子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-06-provider-channel-expansion-freebuff-ui.md)，不重复开发已有协议、管理保存或组件 UI。

## 本批实际闭合

正常 `src/main.tsx` → 透明静态 helper → 真实回环管理 TCP → 生产 `build_app_state` / `build_router` → Local runtime、持久配置 revision owner及 SQLite。管理响应、Secret Grant、配置保存、revision 和重建结果均不是前端模拟。

1. 通过正常服务商目录选择 `freebuff-web`，保存无凭据、无模型的空池。
2. 正常“添加账号”选择 FreeBuff，创建浏览器批准会话；不点击真实平台链接。合成 CLI peer 按服务端实际生成的 UUID 设备返回 challenge，不使用固定设备绕过匹配。
3. 点击“检查授权”，看到“登录材料已取得，尚未保存”；再点击“保存到凭证池”。authToken/fingerprint 留在服务器，不进入前端材料表单。
4. 正常账号编辑的“手动模型列表”保存 `freebuff-local-metadata`，走普通 metadata/SecretPatch Keep。当前 UI 没有 `account_name` 输入框；额外名称字段由独立真实管理 TCP client 设置，用于检查非 UI metadata 保留，**不冒称浏览器输入了该字段**。
5. 停止并同目录、同端口重建完整 AppState/runtime；比较完整 credential 与 revision，正常页面仍看到原账号 ID 与手动模型。
6. 正常“重新授权此账号”再次显式检查、保存。账号 ID/设备保留，使用第二组合成 token/hash，不发明 agentId、token expiry 或 refresh。
7. 再次同目录/端口重建，核对完整材料、稳定 ID、模型与名称；正常页面再次确认 ID 和模型。整个浏览器流程只有两次 code POST、两次 status GET、两次 import。

自动 TCP 测试另外覆盖无 Grant 拒绝、公开响应 no-store/脱敏、重复 import 不重投、原 Grant 在 runtime 重建后失效及重新取得 Grant。默认测试栈，不设置 `RUST_MIN_STACK`。

这些是**同一测试进程中的 runtime 重建**，不是应用进程/OS 重启或跨进程 receipt 恢复。SQLite 文件和 Local owner 已实际运行；凭据/revision 的恢复由生产持久配置 owner 和完整值比较证明，不宣称已在某个 SQLite 表直接查到凭据。

## 实现与安全边界

- 新增长期保留的 [local_tests](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/console/freebuff_login/local_tests.rs)，按 runtime、合成 peer、自动 flow、显式 browser host 分责。只增加 `cfg(all(test, feature = "line-freebuff-web-reverse-api"))` 接线，不新增生产可配置 origin 或管理测试端点。
- 每个真实 HTTP task 绑定现有 `TEST_ORIGIN`，现有 registry 再传播给其受跟踪 operation；不放宽固定 FreeBuff 产品、Origin、Grant、原 revision、SecretPatch/CAS 或 receipt 相等检查。
- 合成 peer 仅监听随机回环地址，不转发真实平台；检查 code/status 无 Authorization/Cookie、设备/opaque expiry/hash 原样绑定和请求数量。此局部 seam 不等于进程具备全局网络隔离。
- 新建绝对测试 root；明确 Local、无 PG、Redis pool closed、禁 folder/refresh 后台入口。临时环境 `TEMP/TMP` 明确指向本批 `linshi/tmp`；自动测试保留在该目录下，不读取真实 `.ng` 或历史凭据。
- 文件式 `restart/finish` 控制仅属于 ignored test host，sequence 防重放；未知/超时回执要求核对，不重发交易。普通 UI/client 没有注入假保存/Grant。
- 成功退出先 drain/await 管理 HTTP，再 shutdown/join FreeBuff 登录 owner、关闭 executor/Local SQLite，最后 await 合成 peer；仅全部完成后写 `completed.json`。错误 Drop 只作 fallback，不当作正常 join 证据。

## 可复核证据

临时脚本、合成材料、SQLite、构建、日志、前后快照均在：
[gateway-freebuff-local-ui-20261106](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-local-ui-20261106)。

| 检查 | 结果与边界 |
| --- | --- |
| 最终相关 Rust | `affected-final.json/.log`：157 passed / 0 failed / 1 ignored；含新增普通真实 TCP 测试，不与中间重跑累计 |
| 显式 browser host | `browser-host.json/.log`：ignored host 显式执行 1 passed；真实正常 main 页面参与，不是合成 API client |
| 生产库检查 | `production-check.json/.log`：locked、no-default-features、runtime-defaults、jobs4、lib，exit0 |
| Rust 来源 | 上述三个最终阶段各冻结 3214 项选定输入，阶段内无漂移；最终审计再次核对 |
| 正常前端构建 | `ui-build.json/ui-inputs.json`：正常 main、895 项选定输入无漂移，输出只在 linshi，`cleanDistPath:false`，未加载 live publisher |
| 既有前端类型检查 | 未修改前端源码；核对上一批最终 typecheck 的 939 项输入仍完全一致，复用已通过结果，不冒称本批重跑类型/82项组件测试 |
| UI 结果 | `ui-challenge.jpg`、`ui-ready-not-saved.jpg`、`ui-after-first-rebuild.jpg`、`ui-final.jpg/ui-final-dom.txt`；1280×720，最终 ID 与模型可见，当前捕获 browser warn/error 为空 |
| 真实保存/重建 | `browser-runtime/completed.json`：两次同 root/port 重建、完整材料保留、稳定 ID/设备、codePosts=2/statusGets=2、joined=true |
| 额外管理 metadata | `management-metadata.json`、`management-after-reauthorization.json`、`management-after-second-rebuild.json`；真实 TCP，不假称 UI 操作，均核对 UI 手动模型和名称 |
| 格式/规模 | 官方 Rustfmt 通过；6项源码（5新增）均 UTF-8 无 BOM，最大154有效行，唯一既有 mod.rs 为85→87；不改 baseline/exception |
| checker/ratchet | checker31 passed；最终 ratchet4575项通过、governed source >700=0，14项不可变 runtime 资产分列；checker实现未变，最终不无故重复跑其单测 |
| 报告保存 | 本批开始直接备份正确 root `artifacts/effective-code-lines.json`，两次 gate 均精确恢复；`gates-final.json` 与 `ratchet-report-before.json` 可核对，不复用上一批的错误备份路径 |
| 差异/收尾 | `scope.diff`、`diff-check.log`、`lines.json`、`process-closeout.json`、`closeout.json`；真实链、冻结来源、编码、规模、whitespace、停止回执联合核对通过 |

![第二次 runtime 重建后的稳定账号 ID 与手动模型](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-local-ui-20261106/ui-final.jpg)

## 保留的失败与限制

- `tcp-v1` 未进入 Rust 测试：原生 BoringSSL 构建的 `Lib.exe` 报 `MSB6006`、`-1073741502`。随后单独工具启动输出正常版本/help，未更换工具链、依赖或生产源码；一次同命令重试恢复。只确认构建阶段失败及重试成功，不推断其系统根因。
- `tcp-v2` 的 Rust 测试 exit0，但执行期间主代理收紧了 ignored browser host 的“非空稳定 ID”和“HTTP 200 后才判脱敏”断言，故冻结回执记录一个文件漂移、wrapper exit1；**不当作最终无漂移证据**。最终相关测试、显式 host 和库检查均在修正后的3214项输入下完成且无漂移。
- 模板 AX 将 pressed button 展示为 checkbox，首次 Playwright checkbox locator 没匹配；读取真实 DOM 后按 button 成功，不改产品控件或测试断言来掩盖。
- 本批不增加生产行为，也没有发现需修改生产语义的集成缺陷。前序扩大 Python scanner 的既有失败未重跑/清除，不报全仓绿或完整 feature 矩阵通过。
- 本批未部署、替换或重启在线实例。收尾只读发现此前 PID23980 已不在，同一旧包出现 PID12520/8776（07:05启动）；本批没有操作这些实例，也不据此猜测变更原因或声称在线实例身份始终未变。

## 清理与下一接力

文件 finish 正常完成 host 测试；静态 helper 通过 stdin stop 退出0。`process-closeout.json` 确认本批 PID12432/30612/30008/34372 均不在，65229/65260 无 listener；测试页已关闭，无 viewport override。保留测试现场，不删除数据、不操作容器；P34 Redis 单容器授权不复用。未提交、推送、出新手测/发布包、读取真实凭据或调用真实平台。

接下来仍需完整 FreeBuff agent map、session/run lease、精确账号 probe、公共业务、取消/失败与资源审计；Server/PG/Worker、跨页面/进程未知保存恢复、独立进程/原生 EXE、真实平台模型/额度/权益/费用也未验。其他 R01–R16 的未完项、P50 无凭据整包、P60 集中录制、P70 逐渠道真实验收全部保留。累计模板仍16，不增加整卡 `offline_ready`，总目标保持 active。

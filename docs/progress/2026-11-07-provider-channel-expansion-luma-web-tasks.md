# Luma Web：原任务完成准入与 local/remote 完整结果

日期：2026-11-07（Asia/Shanghai）。工作区：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`。
分支 `main`，HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`；实现含继承 dirty edits，未提交、推送或出包。

## 1. 范围与结论

承接[有效图片子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-07-provider-channel-expansion-luma-web-images.md)，
本批补现有 board 执行器的原 POST output ID、严格 completed、完整结果透传和下载前上下文准入。
不是当前官网 wire、完整会话、模型权益、供应商费用或 R14 整卡完成。模板仍 16，无新增整卡 `offline_ready`；
P50 全范围无凭据集成/devtest、P60 集中录制、P70 真实逐渠道验收仍未完成。

## 2. 实际修改

- [浏览器操作 owner](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/scripts/lumalabs-session/browser-operation.mjs)：
  signature → 订阅 → 可选 menus → 一次 POST 的顺序保留；仅接受 POST 响应中恰好一个非空、无首尾空白的 output ID。
  事件必须匹配该 ID 且 `artifact.state === "completed"`；`finalizing`、其他 ID 或未终止的 SSE tail 不成功。
  LF/CRLF/CR 完整 frame、多行 data 聚合后解析；metadata 解码上限 128 KiB、SSE 累计 2 MiB、fatal UTF-8。
  menus 的 transport/status/非 JSON 仍按原合同 best-effort，但 reader/decode/cleanup/预算失败不再吞掉并继续 POST。
  所有主流程退出统一 cancel/release SSE reader、abort fetch、释放 timer；先 cancel 再 abort，避免自造 AbortError。
  timeout 保留 504，清理失败不能保持成功；错误固定投影，不返回原 body、异常字符串或签名材料。
- [TaskResult](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/lumalabs/task_result.rs)：
  保留 `signedUrl/outputId/actionId/resolvedActionType/realmId/mediaOperation/artifactField/state`。
  local worker 从原 stdout 解析完整结果，Rust executor service 校验并完整序列化；remote/local 合流后比较本次
  realm、operation、artifact field，关闭自动发现时还比较显式 action type，再允许图片下载或媒体 envelope。
  旧 URL-only、非 completed、缺字段/坏类型/空白、foreign context 均固定错误、禁止 retry/fallback。
- [Node executor service](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/scripts/local-browser-executor-service.mjs)：
  Luma 顶层 CLI 结果完整放入 service `result`，修复原 `result: null`；其他 provider envelope 不变。
  Luma 不使用 caller `resultFilePath`、文件轮询或通用 `bodyFilePath` 读取/清理；仅消费 metadata stdout。
  编码后 stdin 1 MiB 在 spawn 前检查，stdout 128 KiB/stderr 16 KiB，严格 UTF-8/非零退出拒绝；
  stdin error 交给原 finishReject/terminate owner，早退不再因未处理 EPIPE 使 service 退出，错误固定脱敏。
- 跟随升级成功夹具、原错误断言和公共图片夹具，不改变正常管理 Key、精确凭据、图片验证或 Local 结算框架。

兼容边界：Gateway 与远端 browser executor 应同步升级完整结果合同；旧 URL-only executor 会明确失败，
不会退回本地或换账号再生成。没有改变凭据/数据库格式，也不要求用户编辑数据库；部署版本一致性仍待整包验收。

## 3. 验证与证据

全部本批调查、直接 preimage、命令、exit code、日志、来源 SHA 位于：
`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-luma-web-tasks-20261107`。
运行器把 TEMP/TMP/TMPDIR 指向该目录；没有真实凭据、Chromium、上游生成、容器或在线实例操作。

- `node-red`：0 passed / 8 failed，真实复现 finalizing、字段丢失、partial SSE、资源/预算等行为；不是导入/语法失败。
- `rust-red`：编译成功后 0 passed / 1 failed；正常 public router/Local Key 仍把仅 URL 结果取图并返回成功。
- `node-service-red`：0 passed / 2 failed，实际 Node service/HTTP/CLI 复现完整结果变 null、非零退出仍成功。
- 交叉审查后的 `node-resource-red`：0 passed / 3 failed，复现菜单 reader 失败仍提交、stdin 早退导致 `fetch failed`、
  输入先进入 worker 才超限；对应 `node-resource-green` 3/3。
- `node-final`：49/49（Luma 30、Producer 12、Suno 7；本批新增 Luma 14）。包含真实 loopback Fetch SSE 成功/超时关闭，
  以及实际 Node service + HTTP + CLI；Playwright 明确 synthetic，不冒称真实浏览器或真实平台。
- `rust-final`：147 passed / 1 ignored（新增 6；含 Agents/协议/helpers，不称纯 Web 147）。
  既有 ignored 要求显式新 fixture root 和专用浏览器 session，没有运行且不计通过。
  保护原公共 PNG/JPEG×URL/b64 成功、精确身份/一次 Local 扣费、坏资产/取消退款；新增缺失/foreign/nonterminal
  结果在资产 GET 前失败、本地余额恢复、无 fallback，以及 typed local→service serialization→remote 合同。
- `production` 与 `feature-off`：正常 runtime-defaults 和仅关闭 Luma Web 的生产 lib 检查均通过；后者复用 Cargo 指纹缓存。
  这不是全 feature matrix、Server/跨进程或原生包验收。`python-final` 的 executor 生命周期/正常启动依赖合同 26/26。
- Rustfmt、checker 31/31、ratchet 4756 文件通过；14 immutable 单列，governed source >700 为 0。
  本批受审源码 17 个，最大 485；browser owner 438→361，Node service 460→485，均保持单一职责，无例外/baseline 修改。
- 首轮 `rust-green` 编译期间修改了 6 个 JS 输入，完整 `source_unchanged=false` 原样保留；Rust 与实际消费的 worker fixture
  没有漂移。收尾 `rust-final`、两项生产、Node/Python、formatter/checker/ratchet 各 4738 个所选输入均无漂移；
  范围是运行器枚举的 Rust/前端/脚本和指定配置，不冒称全部仓库文件/外部凭据/动态输入冻结。
  编码、文档本地引用、scoped diff、`git diff --check`、最终 hash 与根报告直接 preimage 恢复由 `audit.json` 记录。

## 4. 本轮公开核查与不能扩大声称的保证

[本轮来源索引](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-luma-web-task-public-20261107-agent/sources.json)
保存官方 URL、UTC、status、SHA；[逐字片段](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-luma-web-task-public-20261107-agent/evidence.txt)
带原文件名和字符 offset。主代理抽读了索引与业务片段，没有执行下载的 JavaScript。

- `app.lumalabs.ai/` 与 `/board` 匿名 GET 都重定向官方 sign-up，最终 auth 页 403；没有取得新 App 的业务 chunk。
- 官方 `dream-machine.lumalabs.ai` 匿名 200，公开业务 JS 确有 `section_*` WebSocket、generation 状态、复数 artifacts 合并
  以及 `/api/photon/v2/generations/{id}`。这是另一个 surface，不把其 schema 偷换进新 App actions/events。
- 本批只给已有执行器增加 fail-closed 的本地准入。它不证明当前服务端仍用旧 action 名、事件结构或这些状态；
  真实当前 wire 不符时失败，不能通过放宽到 finalizing 或重发来“修好”。
- `actionId` 可以 null；Rust 事先不知道服务器 output ID，依赖浏览器将本次 POST 的 output ID 绑定到该 realm 订阅事件。
  realm/operation/artifact 是内部请求上下文，不冒称事件本身含且已经独立验证这些字段。
  自动发现实际 action type 得以保留，但尚无当前模型目录/映射证据，不能声称精确模型已绑定。
- 完整 task metadata 不证明签名资产的精确发行方、跨请求 exactly-once、服务器撤销或供应商退费。
  公共 pipeline 的失败退款只恢复本地预占；既有单图有效 bytes/Local 成功与取消合同仍由前批与本批回归保护。
- Node 本批不宣称通用 HTTP body 有界、Windows 硬进程树/lease/远端取消全部闭环；其他 provider 的通用文件路径机制未改。
  CLI standalone cleanup 的永久挂起仍依赖外层 Rust/Node deadline 与进程 owner，不另加平行 supervisor。

## 5. 接力

1. 合法取得当前 App board 业务源码/集中录制，冻结会话、精确模型/参数、任务/失败/断流恢复与资产发行方合同。
2. 有效音视频、精确媒体 probe/逐次费用确认/正常 UI/Keep、远端取消与 lease 分别继续；不能以 metadata 先恢复模型健康。
3. 官方 Luma OAuth/MCP 继续独立 client/工具/授权/刷新合同，不能拿 REST Key 或 board 成功代验，也不先建空模板。
4. FreeBuff、Suno/Udio、Kiro/Qwen 未完项和跨进程/Server/原生包、P50/P60/P70 继续按总计划推进。

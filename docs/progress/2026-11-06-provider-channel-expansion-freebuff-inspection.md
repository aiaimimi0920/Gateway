# 服务商扩展：FreeBuff 正常账号的 signed 会话与余额检查

日期：2026-11-06。工作区 `C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`，`main` / `7969f12c85efceacf2a5e384e143b5c39a550981`，包含继承及本批未提交改动。**这是 R06 的只读管理子链，不是 FreeBuff 整卡、消费准入、公共执行或 P50 完成。**

证据根：[gateway-freebuff-session-inspection-20261106](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-session-inspection-20261106)。最终结果和输入来源以该目录 `closeout.json`、`audit.json` 及各最终阶段回执为准。

## 1. 本批改变

- 新增正常 `POST /console/freebuff-session-inspection`：只接受精确 provider/credential/revision/已保存模型，业务 origin/client 从同一不可变配置 snapshot 读取，不接调用者 URL/token，不继承其他账号或 provider 模型。
- 抽取原 catalog 授权的 snapshot 部分供本入口复用。正常管理身份、Secret Grant/Origin、原 revision、登录受众不变；获取设备之后、catalog 之后和响应前继续围栏。无 Local owner/ready registration 明确拒绝，不生成 key、不 unsigned/bootstrap/register。
- 复用上一批 signed business transport：有界 slot 内仅 signed catalog GET→同 snapshot 的 signed inspect GET；不 POST admission、不 heartbeat/end/completion、不创建轮询 task、不认领远端 instance。
- 新增窄 typed 展示 parser：HTTP 200 与 body 状态同时核验，active 全部必填字段/日期顺序/非负有限 remaining 必须合法；不导出 instance、accessTier、handle/fetchId、原始 body 或退款事实。ended grace 与 active 分开，均不是本进程 lease。
- Freebucks 的字段 absent、null、模型价格缺失、明确零价分别保留；必要余额/价格格式错误不填零。一次 observedAt 上应用有序 dated price changes，再应用 UTC recurring/跨午夜/北京周末政策；只投影原 map 有的 key，不改余额，不扣 discount，不把小时、美元与 Freebucks 混用。不计算完整 legacy quota/intent，也不产生服务器锁价或可消费 quote。
- 正常 `ConsoleGlobalDialogs` 的原账号编辑已接模型选择、显式检查、短状态/余额/参考价。仅用保存目标和此账号模型；dirty、权限/revision/目标变化、取消和卸载会 abort 并拒绝迟到结果；恢复 Grant 不自动重投。原公共目录模式 guard 保留。

接口与必要边界见[管理合同 §5.4](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/provider-freebuff-login.md)，原传输合同见[signed business wire](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/provider-freebuff-business-wire.md)。本轮只读核实的固定公开参考扩充到 43 文件，均核对实际 SHA-256/Git blob SHA；URL 与固定 commit 在 `source-references.json`，不是当前平台可用性或服务端实现证明，没有执行第三方源码。

## 2. 验证与失败保留

- 入口红测 3 项实得 404，对应新 route 尚不存在；前端红测为实际账号组件缺少“检查会话与余额”。未用既有错误冒充本轮红测。
- 首轮生产 check 的 `HashMap::as_ref` E0599 是本轮实现错误，已改为借用原 credential map；首轮 14 项后端用例为 10 pass/4 fail，失败是 JSON `Number(0.0)` 与整数 Value 的测试比较，已按实际浮点合同校正断言，未改变生产数值。审查同时补 UTC offset 归一化、真实 claim header 缺席断言及 Grant/revision 双围栏场景。
- 最终正常 FreeBuff 库用例 **246 passed / 0 failed / 1 ignored**，其中本批新增 8 个管理 route 用例及 6 个纯 parser 用例。ignored 是旧的专门浏览器 host，本批未启动。仅关闭 FreeBuff **164/164**，生产 `runtime-defaults` lib check 与 workspace Rustfmt check 通过；四阶段各 **3257** 项选定输入无漂移，最终 audit 再次逐项匹配当前文件，不把两种配置的重复用例累加。
- 前端最终 4 文件 **85/85**（25 个本批新增），包含目录、设备与模型保存相邻回归；正常类型检查、隔离正常 main+实际编辑宿主构建通过。最终 test/typecheck/build 各 **959** 项选定输入无漂移，覆盖实际安装 `@neuro/ui/dist` 及其 package manifest，不只记录组件源码。
- 浏览器使用实际 `ConsoleGlobalDialogs`/`useCredentialDialogEditor` 与合成检查 API：打开不请求，明确选择后 reads=1、draftWrites=0、secretEdits=0，展示“未认领”和零参考价；390 CSS 像素下 document.scrollWidth=390、dialogWidth=358，无横向溢出。宽屏及窄屏截图在 `session-desktop.jpg`、`session-mobile.jpg`，观察在 `browser.json`。退出 dialog 后确认计数，恢复 viewport、关闭 tab、停止本批随机回环静态 server。
- 初次复制的临时 build 脚本仍指向上一临时证据根，故 `build-final` 不作为本批宿主构建证据；它重建了上一临时 web 输出，未改上一批源码。本批修正根目录后在新目录重建，最终以 `build-final-v3` 为准。UI 到真实管理 TCP 的联合验收仍未做，不能把合成 API 浏览器与生产 router/真实 SQLite/上游 TCP 分项测试拼成联合结果。
- checker **31/31**，ratchet **4632 files**，14 个 immutable runtime 单列、governed source >700 为 0。17 项源码最大 421 有效行、9 项新增最大 278；原 catalog 管理文件 232→257 为共享授权 snapshot 的窄提取，仍单一管理边界职责，不改 baseline/exception。root 报告直接备份并原字节恢复；`lines-final.json` 记录逐文件前后值。
- 保留编译/红测/数值断言/临时构建路径失败及修复回执，不报全仓绿或零 warning。最终来源、UTF-8 无 BOM、scope diff 和本地链接复核见 audit；无关 inherited edits 不撤销。

## 3. 未完成与接力

接下来在此只读 snapshot 基础上补逐次消费确认、即时价格/余额重验、发送前 durable pending/claim、unknown 对账与跨进程恢复；只有真正属于本 owner 的 active capability 才能进入 agent/run/lease/probe/公共 JSON/SSE/取消链。不要先开放会购买闲置会话的管理 admission，也不要直接复用旧 FINISH 的假零 credits/无条件 completed。dedicated reuse 缺 method/body/完整 headers，继续明列，不猜请求。

新设备/目录/会话检查 UI→真实管理 TCP、Server、跨页面/独立进程/原生包与真实权益另验。其他原始渠道缺口及 P50/P60/P70 按[总状态](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-06-provider-channel-expansion-status.md)继续，不用不断扩展 FreeBuff 可选项替代它们。

本批不读真实 `.ng`/历史凭据、不发真实认证、购买或生成请求，不出 EXE、不提交/推送/部署、不操作现有在线实例或 Docker。P34 Redis 授权已使用并清理，没有重放历史问卷。模板仍 16、无新增整卡 `offline_ready`，总目标仍 active。

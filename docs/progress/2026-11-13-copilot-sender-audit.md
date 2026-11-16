# Copilot sender 与成功审计状态保真

本块继续验证专用 sender，不解除 compiled Copilot credential 的禁路由状态，也不访问真实
账号。内核允许执行的测试使用显式构造的可信测试 gate；真实 eligibility 与公共路由另测拒绝。

## 发现与修复

上一块 canonical 已保留 `upstream_status=None`，但成功收尾的 request audit 和 conversation
archive input 都硬编码为 `Some(200)`，会丢失非 HTTP 执行语义。本块增加
`PipelineContext.observed_upstream_status: Option<Option<u16>>`，由 sender 的受信任响应赋值：

- 外层 None：旧路径尚未观察，保留历史成功默认 200。
- Some(None)：明确没有上游 HTTP 状态；当前仅由已识别的 Copilot 缺状态响应产生。
- Some(Some(status))：实际观察到的上游状态，传入审计和归档 input。

独立审查发现 ChatGPT Web 等旧 HTTP canonical 构造也可能省略 status，因此没有将所有
canonical None 一概升级成“明确非 HTTP”。实际 sender 所用映射函数另有兼容测试。
两处持久化输入共用此三态；没有更改数据库版本或响应线协议。

实际 sender 集成还发现 Responses 会被前置 `native_text` 分支截走，尝试 HTTP 后报
`Invalid upstream URL`。已在该分支判断排除 `copilot_sdk`，使通过严格 single_turn
准入的 Responses 与 Chat 一样进入 SDK sender；其他 provider 的原生分支不变。

## 用量和费用边界

本轮未修改既有计费政策。Local audit 的未知 usage 仍保存 NULL；Local 模式不 enqueue
整数 UsageReport。token 余额在未知 usage 下按现有规则保留预扣估算，不能将该估算描述为
已测得的实际 token 数。cash ledger 缺 usage 时保持 unresolved/reservation，不产生零元
成功账单。当前 CopilotIdentity 仍要求 Local runtime 且无 PG，未放开 Server 上报路径。

归档功能当前要求 PG；Local-only Copilot 的归档调用会返回无写入。因此本轮 archive
验证针对输入构造，不是 PostgreSQL/object storage 持久化或 Server 验收。

## 验证记录

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-copilot-sender-20261113`。
最终 `sender-r4.log`：修复 native 分支后重新编译，1 passed / 0 failed，内部覆盖 Chat
和 Responses 两次 SDK 执行及 Local 审计。构建总耗时 26m44s，包含共享构建锁等待及原生
依赖编译，不是纯增量耗时。两次调用的 child 均经 Windows PID 核对退出，且每个 child
各一次 session.create、session.sendAndWait、runtime.shutdown，见
`sender-r4-process-exit.json`。

本轮新增定向测试累计 6 项通过：`focused-r2.log` 的 4 项、`public-r3.log` 的 1 项、
`sender-r4.log` 的 1 项；不是一次运行 6 项全绿。使用 r4 新测试 EXE 再执行直接相关
finalizer 回归 16 passed、packing 回归 6 passed，见 `finalizer-r4.log` 和
`packing-r4.log`。未重复全量回归或调用真实上游。

formatter、`git diff --check`、checker 31 项测试和最终 effective-line ratchet 均通过。
本轮涉及文件有效行依次为 pipeline/mod 366、stage_send 499、buffered 432、sender tests
186、public tests 77、audit_persistence 200、archive 261。源码 UTF-8 无 BOM 检查通过；
最终源码与合成／根测试 EXE 哈希分别保留于 `source-final.json`、`binaries-final.json`。

`focused-r2.log` 首轮编译通过，定向 4 passed / 2 failed。两项失败分别为 sender 夹具
沿用 `hello` 而合成对端只支持预设词，以及公共 Key provider 限制提前返回 403 而断言
错误预期 400。修正为支持的 `ok` 场景与实际权限拒绝状态；没有修改生产准入或合成对端。
失败场景的唯一 child 也已核对退出，保留 `failed-fixture-process-exit.json`。另按既有
reserve/refund 行为使余额比较忽略可变 updatedAt，仍比较所有余额、额度及状态字段。

`public-r3.log` 已验证公共四种请求形态均被 provider 授权拒绝、零 attempt、无上游状态、
未知用量，余额字段保持。`sender-r3.log` 则暴露了上述真实 Responses 接线缺陷：Chat
已走通且审计断言通过，但整项测试在 Responses 报错，不能算通过；该次一个合成 child
确认退出，见 `sender-r3-process-exit.json`。修复生产分支后仅重跑受影响的 sender 测试。

首次启动测试命令时，发现合成工作目录错误地设为 probe 根的同级，而夹具要求它位于
probe 根内。在任何测试执行前，仅终止本次已核对身份的 Cargo 进程树并修正命令；保留
`focused.log` 的中止回执。未终止其他任务、未放宽 fixture 的路径校验或创建网络盘映射。

## 未完成

公开成功请求、管理保存到推理的全链解禁、实际收费 Key 的 Copilot 专项结算、真实 CLI/
账号、原生 EXE/Server 验收仍未完成，不增加整卡完成数。无提交、推送、出包或部署。

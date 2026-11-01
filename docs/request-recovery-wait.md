# 请求恢复等待

## 模式与兼容

`request_wait` 是可信服务商配置，不进入上游 JSON、headers 或模型参数。
桌面创建服务商的调用设置提供“恢复等待”：自动、5／10／15 分钟、连接期间持续等待、快速失败。
已有配置省略该字段也能读取，不迁移数据库、不改账号 ID、Key 或凭证。

```yaml
providers:
  - id: example
    preset: openai
    request_wait:
      mode: bounded
      total_timeout_seconds: 900
```

- `bounded`：默认 300 秒，可信 `total_timeout_seconds` 支持 1–900 秒。
- `client_connected`：首次预算配置时不使用默认总截止；显式可信截止仍是硬上限。
- `disabled`：不进入新恢复等待；保留原有有限安全重试，不等于禁止所有旧 retry。
- 自动：仅声明的官方 OpenAI／Codex OAuth preset 默认长等待，其他 route-document
  服务商默认有界。不会凭模型名、域名、OpenAI-compatible adapter 推断长等待身份。
- 只有 ChatCompletions、Responses、Messages 开启此恢复；搜索、抓取、媒体创建不增加重放。
- 混合路由仅在所有候选都是长等待模式时去除默认总截止；显式可信超时取最短值。

本轮接入重点是桌面／Local 的 route-document 服务商。没有对应 route-document 的
PostgreSQL／Redis 动态服务商保留原有限模式；没有宣称补齐其初始 SQL cooldown 过滤和
全部凭据轮换合同。客户端或前置反向代理自己的 timeout 也不会被网关覆盖。

## 恢复与安全重放是两件事

只为明确的 `ModelCapacity` 拒绝、发送前临时 circuit、网关网络恢复窗口、OAuth 刷新等待
保留请求。裸 503、未知执行结果、鉴权／余额／参数错误、手工禁用、缺少授权不无限等待。
从临时限制恢复后，仍须通过原发送资格检查；不会为等待清空健康状态或永久封禁。

首次仍按原路由走已授权 fallback。需要等待时锁定已授权的具体 provider＋credential＋
upstream model，之后不重新运行 stage_route，不重复配额预扣／reservation，不主动换模型。
每轮刷新 route snapshot、Key 分组交集、本地 Key 限制和同一账号的最新 OAuth override；
账号被移除／停用或权限撤回时终止。刷新工作仍由原 single-flight worker 持有，不重复消费 refresh token。

明确容量拒绝只为每次真实拒绝增加一个重新准入资格，包括携带工具声明的对话请求；
实际发送数和审计累计保留。未知网络错误维持原有限重试和工具保护，不能借等待重放副作用。
SSE 返回流所有权后，不重进调度、不换账号、不拼接第二份回答；部分流中断维持原错误终止。

Native Codex 首次 hop 的已证建连失败另走同账号短时恢复：最多 4 次额外连接资格、
30 秒重试准入窗口，仍通过原 gate／attempt budget，不加入无限等待队列。
具体安全条件和时间边界见 [请求预算](request-budget.md#native-连接建立失败的有限恢复)。
未知执行结果不会因为“本地代理不稳定”就被当作未执行。

流审计区分 `upstream_interrupted`、`deadline`、`gateway_shutdown`、
`client_disconnected` 和 `stream_consumer_closed`。取消／shutdown 写 `cancelled`，
deadline 和真实上游中断仍写 `failed`；退款和 usage failure settlement 保留。
仅明确 caller cancellation 填写 `clientDisconnectedAt`；一般流 Drop 不猜用户意图。
审计按 first-terminal-wins 保存，迟到回调不覆盖先前结果。

## 资源与生命周期

外层轮询间隔为 2／4／8／15／30／60 秒，之后封顶 60 秒，尊重更长 Retry-After。
本地容量窗口仍是独立的发送准入屏障；等待不新增真实探测，不提前发请求。
发送 permit 在睡眠前释放，恢复发送使用每 provider 的 FIFO mutex；取消等待者不阻塞下一位。

首次保留请求时登记进程内队列，限制总计 64 个实际请求、每 Key 16 个等待名额、
每 provider 32 个实际请求、估算保留量 64 MiB。
这里的 session 指一个主 AI 对话及其派生的子对话／子代理构成的任务树，
不是客户端版本、进程、窗口、网络连接或 Key。同一客户端另开的独立主对话分别计数。
使用客户端显式提供的根会话 `session-id`（兼容 `session_id`），同一已鉴权 Key 下，
同一主对话及其子任务的多个等待请求共用一个 Key 名额。最后一个请求及其持锁 turn
退出后才释放；同一请求内部的恢复轮次始终只登记一次，不按对话历史累计占额。
分组标签须非空、无控制字符且不超过 256 bytes。缺少、无效、重复或冲突的根会话标识
按实际请求分别占名额；不把鉴权主体、JSON `user`、request ID、父 thread、客户端窗口
或 prompt cache key 猜成根会话。会话标签仅用于公平计数，不授予权限。
Codex 的子代理继承根 `session-id` 并拥有独立 `thread-id`，这里有意合并前者，
不因后者不同拆分名额。客户端应为独立主对话提供不同根标签；若把不相关任务显式标为
同一根会话，网关无法从内容或线程 ID 猜测真实边界。其他客户端无可靠根标识时逐请求计数。
依据官方 [Codex 0.155.1 根会话继承](https://github.com/openai/codex/blob/rust-v0.155.1/codex-rs/core/src/session/session.rs#L809-L821)。
官方 main 的 [Responses session-id](https://github.com/openai/codex/blob/4bc746c94fce95ec7ca5f1a91a786156d240c8a2/codex-rs/core/src/client.rs#L730-L737)
也承担缓存亲和性，显式复用该标签的 ephemeral fork 会合并计数；本实现不宣称能从请求恢复所有客户端的任务树。
这不是执行去重：同会话的不同请求仍分别执行、审计、取消和记内存，全局／provider 的
实际请求上限不打折，也不自动取消旧请求、复用另一请求的回答或重放不确定的执行。
超限返回 `recovery_wait_queue_full`，不无限堆积。保留量根据 canonical JSON 流式计数后
保守估算副本开销，不是操作系统 RSS 的硬内存承诺。
拒绝日志记录 `limit`（`key_slots`／`global_requests`／`provider_requests`／`retained_bytes`）
及实际计数，不记录 Key、session 标签、凭据或请求内容；公开错误码和错误文案保持兼容。
本地请求另预留 768 KiB 身份映射预算，覆盖最多 256 对、每个 ID 最多 256 bytes
的当前／替换快照及保守树节点开销。即使入队时映射为空也预留，避免等待期间映射
增长绕过原队列记账；非本地请求不增加这项预算。预算随原 lease 一起释放，不重新入队。
队列 lease、sleep 和 transport 都归请求 future 所有，无脱离客户端的持久任务或自动落盘重放。
已验证的服务商改名映射在进程内保留原始队列 owner，旧／新名称共享限额与同一
FIFO 锁。若两侧已经形成独立活动队列，拒绝身份发布而不尝试拼接顺序；恢复发送
读取新快照时也重做此检查。持锁 turn 与 lease 共同保留记账，最后解锁后才释放。
显式取消、deadline、shutdown 在安全等待／发送边界释放资源；不把配额事务随意丢在半途中。

Managed native HTTP、通用 HTTP、CPA Rust transport 不另加一个隐藏的 300 秒总 timeout；
连接建立仍有有限超时。流交付后的 deadline 继续由同一个 budget owner 控制。
真实 HTTP 断开是否及时取消必须以 loopback TCP 回归为准，不能用任务 abort 冒充证明。

## 验证与未交付边界

测试仅使用虚拟时钟、隔离 SQLite 和 loopback 合成上游；不调用真实账号、不批量探测模型。
具体 passed／failed 回执写入进度记录，本文不代替执行证据。
当前源码不改变正在运行的 4200 EXE；新 EXE 构建、打包和用户手测单独进行。

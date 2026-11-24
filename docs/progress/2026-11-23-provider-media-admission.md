# 首批服务商：媒体、推理历史与发送前拒绝验收

## 范围

本批合并补验 SenseNova Chat 图片／工具推理历史、SAP Orchestration 图片／签名块，
并修复隔离包实跑发现的 SenseNova 发送前拒绝错误。不是三服务商整卡完成，
不涉及真实 Key、租户、费用、线上实例或现有业务数据库。

## 实际缺陷与修复

旧 API 包的 `sensenova-media-20261123-1940/failure.json` 保留失败：
四次正向成功后，前三条非法输入返回 400，peer 没有新增 POST，但公共审计误计
attempt=1 并归属账号；凭据被累计为 failure_count=3／cooling，第四条变成
429／`access_candidates_exhausted`。余额仍为 5／1，未发现额外扣费。

根因是 `begin_attempt()` 先于原生 planner；本地校验错误进入普通异步失败反馈。
现在在现有 `stage_send/admission.rs` 复用 SenseNova 原生 `build_request_plan`，
于 concurrency／发送准入／attempt 之前校验，失败只追加有界的拒绝原因。
成功请求仍遵循原授权、限流、预算与取消；没有降低健康阈值、重置凭据或放宽输入。
为避免跨 executor 重构，成功路径暂保留发送内部再次建包；SenseNova 不走 XML
工具桥，因此不会在两次校验之间被该桥改写。

新增 Local 回归又发现提前 `route_audit` 的候选归属被终态 `.or(old)` 保留。
将原有 running 归属写入整体移至 planner 成功之后，未改变全局审计合并语义。
本修复只证明该 planner 拒绝路径；后续 AIMD 等待取消或其他准入拒绝不在此证明内。

## 受控验收设计

- SenseNova 双 EXE：管理写入／另一进程读回，四条正向包括图片 JSON／SSE、
  reasoning＋工具 JSON 首轮和实际 assistant 回传 SSE；严格核对图片参数、
  reasoning 空白、工具 ID／arguments、公开正文、终态与唯一 2/3/5-token usage。
- 连续四条非法图片／历史必须全为精确 400，POST 总数不增长，四条失败审计
  attempt=0、空实际归属、精确拒绝账号／凭据／模型与两种原因各两条，
  余额不变且 wrong-CA 前凭据为 active／failure_count=0／无冷却。
- 健康查询是上述时点的凭据表读回，不是全模型／进程健康证明。
  本次 `validate_plan` 拒绝不派生 detached 凭据失败任务；旧实现即使反馈延迟，
  也会被 attempt=1 的审计断言捕获。
- SAP 32 条矩阵另见主台账：完整统计被 peer 拒绝的 inference POST 尝试；
  Orchestration 推理使用实际 JSON assistant 续轮，保留完整 content／opaque
  signature 块。不证明密码学签名、图片＋tools 组合或真实上游。
- 所有 runner 串行复用专用隔离 PostgreSQL／Redis；TLS peer 不转发，不修改系统
  信任根。缺 CA、错误 CA、脱敏和正常 drain 单独检查。

## 本批验证状态

第一轮编译暴露一个模块路径和两个审计字段访问错误，已按真实类型修正。
第二轮 Nextest：**144 passed／2 failed／6346 skipped**，
run ID `9f682fc7-d253-473f-b043-cb2a2387f7c2`；
两个失败均为上述 Local running 归属问题，未放宽断言。
修复后第三轮 **145 passed／1 failed／6346 skipped**，
run ID `f124b9c6-1f71-4440-89c4-b92577a99019`。仅剩测试对拒绝列表顺序的错误假设：
实际 SQLite 行已为 provider=null、attempt=0，列表先有 unrelated 的
`model_not_supported`，再有 exact 凭据的目标拒绝。只将该断言改为按
账号＋凭据＋模型＋原因精确查找，保留全部零发送／余额断言；
生产代码不再改动，已通过的 145 项不重复空跑，单独复验这一项。
精确复验 **1/1 passed**，run ID `1b568a20-c8f8-4257-8693-4fbafb52e600`。
因此本批 146 项均有最终输入适用的通过回执（145＋1，不冒充单次全绿运行）。
日志位于 `linshi/gateway-packaged-provider-fixture/sensenova-preflight-nextest-20261123-r{2,3,4}.log`。
修复后新 API 包验收见下，不用旧 EXE 代替。

Python 夹具 **26/26 passed**，34 个 Python 文件 AST 检查通过；
定向 Rust 格式和 scoped diff 检查通过。权威行数 ratchet 通过，
6143 个文件、governed source >700 为 0；后续等行数位置调整另过 fast gate。
本批 `stage_send.rs` 500→500、`admission.rs` 50→56 有效行；
图片／推理测试为 119／164 行，新增媒体 runner 为 139 行。
全仓格式检查的既有无关差异见 SAP 持久化回执，未擅自全仓格式化。

## 新 API 手测包与双 EXE 回执

包类型为 `api`／`devtest`，不含桌面 UI，不是正式发布，也未替换运行实例。

```text
C:/Users/Public/nas_home/AI/GameEditor/Neuro/release/Gateway/
gateway-devtest-provider-media-admission-20261123-194746
```

- EXE SHA-256：`630fb2ed3ee0e8357760bfa809134d8be2de0900c77b8b7578ba187cd0c63547`
- manifest SHA-256：`0da4a3fead6479cb48ea2901f2fb499b27f03475a2fd5aa4aebea253da3b4347`
- 构建来源指纹：`b4a51bc032878f35ff5bf0072d4a5478581ad1f426111409598fc717f3669f50`
- provenance `builtAt`：`2026-11-23T11:55:55.4627702+00:00`。
  manifest 的 `buildTimestamp` 是确定性 Git 时间，不作本次编译时间。
- 复用主 `target/`，3 jobs，本次 Cargo devtest 构建 8m03s；不是冷构建基准，
  不据此承诺提速倍数。来源与包完整性门禁、锁定 worker 导入和 API artifact smoke 通过。
- 发布副本的 provenance 与构建证据哈希相同；包内 34 个 Python 文件与本次仓库夹具
  逐文件 SHA-256 相同，包含本次 32 项 SAP 矩阵，不再是旧包的 28 项脚本。

五项 runner 串行使用专用隔离 PostgreSQL／Redis，全部 `status=passed`；
五份回执的 EXE／manifest 哈希均与上表相同。证据根为
`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-packaged-provider-fixture`，
下表目录均以 `-media-admission-20261123-195840/result.json` 结尾：

| 目录前缀／runner | 实际结果 |
| --- | --- |
| `sensenova-media` | 4 条成功、4 条非法输入全部精确 400；peer 总 POST 保持 4，失败审计零实际归属／零 attempt，精确拒绝原因；凭据仍 active／0／无冷却，余额不额外扣减 |
| `sap-hosted` | 10 lanes、32 条成功审计、32 次 inference 尝试、10 次 OAuth；每次成功扣一条消息，Orchestration 图片和实际 assistant 的完整推理块续轮通过 |
| `commandcode-hosted` | 三个原生公共入口各 JSON／SSE，6 条成功审计；Chat 图片参数及公开别名通过 |
| `sensenova-hosted` | 三个原生公共入口各 JSON／SSE，6 条成功审计；共享发送前校验没有破坏正常推理 |
| `commandcode-systemone` | 1 次原生决策及精确成功审计，公开别名／结构化答案通过，消息额度 2→1 |

合计 **49 条成功公共审计**，另有 SenseNova 四条精确输入拒绝；不将 TLS 负对照混入
成功数。各 runner 的缺 CA／错误 CA 负对照、日志脱敏和正常 drain 均通过，
记录的 writer／reader 进程均已退出；未停止专用数据库容器或清理其数据。
本次不重跑已独立验过的定时 worker 或 SAP 177 项持久化恢复回归。

SAP 矩阵不证明工具调用、图片＋tools 组合、签名密码学或错误 CA 后的退款／失败审计。
不代表原生窗口或真实租户验收。归档回执后仅更新仓库文档，构建来源指纹因此与收尾
文档状态不同；生产代码和夹具未再变化，实物包及其中的文档快照不改写、不重新打包。

## 仍未完成

后续源码状态：SAP planner／OAuth 已移到实际 attempt 之前，并补齐阶段化健康与
探测证据；本轮又补齐精确零发送 JSON／SSE fallback 的拒绝记录、SenseNova 未确认
内容块／非法 text 的发送前拦截，以及 SAP 无用量消息预付恢复。详见
[统一源码回执](2026-11-23-sap-stored-responses.md#拒绝记录与消息预付恢复)。
这些补丁尚未进入上表历史包；下述剩余项中“尚无错误反馈”的描述是该包交付时状态。

Command Code 保持用户批准的公开 Provider API 范围，真实 Studio Key 单列；
后置 OAuth、远端 Responses 生命周期和 System One 现金合同不标为已实现。

SenseNova 视频 wire、控制台 OAuth／客户端资格／积分池仍缺正式合同；
图片与推理历史通过不能抵扣。SAP 仍缺未知提交权威 ID 绑定／人工结案、
无耐久未发送证明的退款处置、不确定 mutation 的人工处置，以及完整 provider
错误分类／持久健康反馈、现金 tariff、其余批准协议与原生窗口／真实租户验收。

后续健康反馈实现必须在发送观测层保留事实，不能从最终 settlement／refund／
projection 错误反推账号失败；普通 SAP 还存在 planner／OAuth 位于 attempt
之后的问题。HTTP 拒绝归属与是否处罚须分开，且不得因此开启生成重试或回退。

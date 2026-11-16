# Bedrock 公共 TCP：成功、工具/签名历史与取消结算

日期：2026-11-07。工作区：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`。

承接[异常子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-07-provider-channel-expansion-bedrock-exceptions.md)和[公共签名历史子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-07-provider-channel-expansion-bedrock-public-history.md)。本批新增真实回环 HTTP 验收，**没有修改生产源码，没有发现需修复的生产缺陷**。它退役的是下述普通 Local 次数 Key 场景的公共 TCP 证据缺口，不代表 W06 或整个计划完成。

## 1. 实际入口与变更

新增入口 [bedrock_public_tcp_contract.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/tests/bedrock_public_tcp_contract.rs)，按责任拆为：

- [runtime.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/tests/bedrock_public_tcp_contract/runtime.rs)：生产 `build_app_state`、新建隔离 Local SQLite、真实管理 HTTP 创建普通接入 Key，次数余额 10；精确 audit/余额/并发 permit/健康计数断言。
- [upstream.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/tests/bedrock_public_tcp_contract/upstream.rs)：Gateway 和合成 AWS 两个随机 `127.0.0.1:0` HTTP server；带明确屏障的上游响应与 Drop 检查，服务正常关闭并 join，异常退出另有 abort 兜底。
- [requests.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/tests/bedrock_public_tcp_contract/requests.rs)：三公共协议及原生请求，公共下一轮只使用生产响应签发的 continuation。
- [outbound.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/tests/bedrock_public_tcp_contract/outbound.rs)：检查实际 JSON/SSE 字节中的标准 text/tool 内容、扩展和唯一成功终态；原生帧复用已有独立 CRC reader。
- [success.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/tests/bedrock_public_tcp_contract/success.rs)：三轮工具/签名历史、跨 JSON/SSE 输出和成功结算。
- [cancellation.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/tests/bedrock_public_tcp_contract/cancellation.rs)：真实 `TcpStream` 关闭、七类上游边界、失败中性和退款。

这是生产 AppState 与两个真实 HTTP server 在同一测试进程内运行，**不是 `Router::oneshot`，也不是独立 Gateway 进程或 EXE**。provider 配置、接入 Key 和 SQLite 都是合成隔离材料；临时数据通过本批 runner 的 `TEMP/TMP/TMPDIR` 放在证据目录，不接触真实 `.ng`。

## 2. 成功与历史续接

Chat Completions、Messages、Responses、原生 Bedrock 四入口，各覆盖 JSON 与流式起步，共 8 个成功测试；每项三轮，第三轮切换输出模式，共 24 次成功请求。原生请求的历史始终采用原生结构，公共请求通过显式 `gateway_bedrock_history` 携带原响应 turn。

逐轮断言：

- 标准文本、工具 ID/name、字面 JSON 参数 `{"v":"&quot;"}`、完整有序 reasoning/signature/redacted 内容和工具结果不丢失、不作 HTML 解码。
- 生产-issued continuation 的 source、实际 resolved model、完整原生 messages/system 前缀 SHA-256；内部 policy 文本不进入公共响应，Gateway 扩展不发送给 AWS。
- 原生 EventStream 帧 CRC、事件与 payload 和预期原生序列一致；公共流式恰有一个成功终态。
- 每轮只有一次上游发送，usage 为 prompt/completion/total `3/4/7`，精确普通 `access_key_id`、实际 provider account、resolved model 与 route attempt 记录一致。
- 余额 `10→9→8→7`；settlement 后所有候选 permit 为 0，成功健康计数逐轮增加。

三公共协议分别篡改旧 user 历史，六个起步组合均在网络前返回 400；上游请求仍为 3 次，余额仍为 7，成功健康计数仍为 3。它们是 6 个本地拒绝场景，不计入 80 次上游调用。

**传输边界：**生产 buffered 路径有意调用 `/converse-stream` 后聚合，即使调用者要求 JSON。本批捕获并断言这一真实行为，没有新增 `/converse` 上游 JSON 传输验收。loopback 使用兼容 Bedrock adapter/Bearer 合成材料，不绕过官方目的地 allowlist，也不代替显式官方 AWS Bearer/IAM 认证验收；显式目标/签名合同沿用前批独立证据。

## 3. 七边界真实 TCP 取消

四入口各有 buffered/stream 两项测试，每项依次覆盖七个同步边界，共 **8 个测试、56 个取消场景**：

| 边界 | 上游实际状态 |
| --- | --- |
| `BeforeHeaders` | 请求已收到，handler 尚未返回响应头 |
| `BeforeBody` | 已返回头，首个业务 body 尚未提供 |
| `PartialFrame` | 一完整帧后，下一帧 CRC 尚差 3 字节 |
| `Partial` | 文本 delta 已到，完整响应仍悬挂 |
| `Reasoning` | reasoning delta 已到，响应仍悬挂 |
| `ToolArguments` | 工具参数只收到未完成的 `{"v":` |
| `BeforeEof` | 完整 AWS stop/metadata 序列已发出，但实际 body EOF 悬挂 |

调用者通过真实 socket 发送正常 HTTP 请求，在明确屏障上检查余额已预扣为 9、audit 仍为 `running`，再关闭 socket。可流式观察的边界先读取到对应 text/reasoning/tool 片段，不用固定 sleep 代替到达同步。随后断言上游 handler/body Drop、唯一 `cancelled` audit、余额退回 10、permit 释放、无重试或备用候选重放、并发 limit 与成功计数保持中性；短静默窗口再次核对没有晚到重放或重复终态。

原生 `messageStop`/`metadata` 有意等真实 EOF 后才对调用者发出。`BeforeEof` 的原生同步点是最后一个非终态 `contentBlockStop`（index 4），同时明确断言尚无 `messageStop`/`metadata`。不能为了方便测试提前发出成功终态。

普通 Local Key 没有 delegated `source_access_key_id`，因此该字段应为 `None`。`client_disconnected_at` 也为 `None`；consumer Drop 不能证明用户意图，trace 使用 `request_consumer_closed` 或 `stream_consumer_closed`，不伪造 `client_cancelled`。

24 次成功加 56 次取消，共 80 次已断言 one-send 的上游调用；这是场景数量，**不能写成 80 项 integration 测试**。

## 4. 最终执行与生产编译复用

本批执行以下五个终态阶段，各阶段 4,252 项所选输入的阶段前后及最终逐项 SHA-256 一致；不是全仓所有文件冻结。

| 阶段 | 实际结果 |
| --- | --- |
| `contract-v5`：`cargo test --locked --offline --no-default-features --features runtime-defaults -j 4 --test bedrock_public_tcp_contract -- --test-threads=1` | 16 passed / 0 failed / 0 ignored；全部为本批新增 integration 测试 |
| `lifecycle-final-v1`：同组合 `--lib pipeline::lifecycle` | 6 passed / 0 failed / 0 ignored；既有生命周期回归 |
| `rustfmt-final-v1` | 7 个新增 Rust 文件 `--check`，exit 0 |
| `checker-final-v1`：`npm.cmd run test:effective-lines --prefix scripts` | 31/31 |
| `ratchet-final-v1`：`npm.cmd run check:effective-lines --prefix scripts` | exit 0；扫描 4,827，14 immutable runtime artifacts，`governed source >700=0` |

两份 Rust 日志按 harness 与完整测试名去重：**22 passed / 0 failed / 0 ignored**。不叠加前批 236/631 项，也不累加中间重复绿测。新 Rust 有效行依次为入口 42、runtime 216、upstream 171、requests 69、success 164、cancellation 148、outbound 139；最大 216，无新增超限文件。

本批**没有重新执行正常/代表性关闭 Bedrock 的生产 lib 编译**。复用前批 `production-final-v1` 与 `disabled-final-v1`：receipt 成功且阶段内无漂移，全部 4,245 项旧输入与当前逐项同 hash；当前仅增加本批 7 个独立 integration 测试输入，Cargo、依赖、build script 和所有既有生产/测试/worker 输入未改。复用 receipt 的绝对路径、SHA-256 和差异列表见 `reused-evidence.json`。代表性关闭组合去掉 `family-bedrock-converse-official-api` 与 `line-aws-bedrock-converse-official-model-api`，不冒称完整 feature 矩阵。

## 5. 中间失败与最终审计

保留全部中间日志，不覆盖失败、不改生产语义来迎合夹具：

- `contract-v1` 编译失败：夹具访问不存在的 `GatewayRequestAuditView.route_attempt_count`，改读实际 `route_trace["routeAttemptCount"]`。
- `contract-v2` 0/16：夹具误认为 buffered 应走 `/converse`，并把普通 Local Key 当 delegated source Key。按生产真实 owner 修正传输与归属断言，不属于生产红测。
- `contract-v3` 15/16：原生取消夹具错误地等 EOF 前不会发送的 metadata；读取原生编码 owner 后改用最后非终态块屏障，并加强无成功终态断言。
- `contract-v4` 16/16：当时仅四个取消边界；`contract-v5` 扩到七边界并增加精确归属/成功健康断言，才是最终 integration 证明。

证据根：[gateway-bedrock-public-tcp-20261107](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-bedrock-public-tcp-20261107)。最终 [audit.json](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-bedrock-public-tcp-20261107/audit.json)、[test-identities.json](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-bedrock-public-tcp-20261107/test-identities.json) 和 [scoped-diff.patch](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-bedrock-public-tcp-20261107/scoped-diff.patch) 检查五阶段终态输入、全部直接写前副本、9 个调查过但未改的生产 owner、编译复用来源、范围 diff、UTF-8 无 BOM、本地链接、有效行数及 `git diff --check`。本批不重建写前镜像；新文件前像为空，既有文件使用直接备份。根有效行报告恢复写前原 bytes，baseline/exceptions 保持不变，不覆盖继承 dirty 报告。

## 6. 仍未完成与下一接力点

已退役缺口：**普通 Local 次数 Key 的三公共协议和原生入口，上述 text/tool/reasoning/signature 历史三轮成功、跨输出模式和七取消边界已有公共 TCP 证据**。不能再笼统列成“公共 TCP 一概未验”。

仍需完成完整声明参数/模态、其他适用余额/权限/存储/运行模式的联合验收、Server/PG/Hosted、跨进程及原生包、真实 AWS IAM 权限/模型开通/额度/期限与最终集中验收。这里的 witness 仍不是 MAC、AWS signature 验证或额度共用证明；没有把有限合成场景升级为完整产品保证。

W06 不标整体 `offline_ready`；新增正常模板仍为 17。其他渠道、P50 全范围无凭据集成与 devtest、P60 集中录制、P70 真实逐渠道验收继续，总目标保持 active。本批不读取历史或真实凭据、不访问真实供应商、不创建或清理容器、不操作现有实例、不出包、不提交或推送，亦不复用已消费的 P34 Redis 授权。

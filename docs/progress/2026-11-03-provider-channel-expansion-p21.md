# 服务商分渠道扩展：第 21 批，Responses 终态错误出口脱敏

> 2026-11-03（Asia/Shanghai）；P01 / P10 的公共协议安全切片，不是新渠道、完整 W05 或总目标完成。
> 本批实现与 167 项定向离线验证已通过；不是 W05/W06 整体、OAuth 工作包或总目标完成。
> `main`，HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`；不提交、推送、打包或重启实例，保护其他桌面连接、设置和 UI 脏改。

## 1. 范围与真实 red

API Key、OAuth、Web 继续按产品、受众、实际额度分别评估；Web 不是兜底，未知共享关系隔离并标 `unknown`。R01–R16 全范围保留。本批不改变渠道身份、认证受众、凭据池或外部产品事实，不引入第三方代码或依赖。

证据目录：[P21](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-provider-expansion-20261103-p21)。在修改生产实现前，实际运行：

| 日志 / 过滤器 | 真实结果 |
| --- | --- |
| `red-responses-v1.log` / `--lib protocol::responses::tests::` | 40 pass / 2 fail，exit 101 |
| `red-responses-full-v1.log` / `--lib protocol::responses::` | 51 pass / 2 fail，exit 101；与 P18 完整过滤器相同 |

两次均为 `accumulate_responses_rejects_explicit_failure_after_partial_output` 和 `translate_responses_failure_never_emits_successful_stop` 失败，原文分别为：

```text
assertion failed: !error.message.contains("private-upstream-detail")
assertion failed: !errors[0].contains("private-upstream-detail")
```

没有 clean-HEAD 对照，不能称已证历史 baseline；也不把 narrow 的 42 项当完整 53 项。

## 2. 根因、最小修复与兼容边界

[terminal.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/responses/terminal.rs) 原 `CompletionError::Upstream(String)` 接收模式 sanitizer 处理后的任意 provider message/code。模式脱敏能识别部分秘密，却不能保证任意私有 prose 都不外泄；该字符串进一步进入 GatewayError、ProtocolStreamError 及其 Debug/source chain。

现在 `Upstream(&'static str)` 只持有固定本地文案，不再复制 provider 的 message、code、type 或 partial output。结构化 `error.code` 优先；只有字段缺失才回退 `error.type`，存在但 null/非字符串/未知时不借 type 猜分类。顶层 `error:null` 可以读取 `/response/error`。不 trim、不做子串或大小写归一化来接受未知码。

本地精确兼容白名单映射为六个固定标签：`credential_invalid`、`token_expired`、`model_unsupported`、`quota_exhausted`、`rate_limited`、`content_filter`；其他输出固定 generic。它是本地兼容策略，不冒称完整官方错误码清单。未知 prose 不再诱导 invalid-key 等启发式分类，可能减少旧的文本猜测归因，这是有意的信任边界收紧。

保持 `responses_stream_upstream_error` code/prefix 与既有 `GatewayError::server_error` 构造；不改 ErrorKind、HTTP 500、retryable、fallback、retry-after、native transport 或增加重放资格。六类标签由现有 provider classifier 读取，不顺手修改健康/重试策略。

**本批不是所有 Responses wire 或 stream audit 的全量脱敏：**

- [completeness.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/responses/completeness.rs) 保留 native 原始失败 bytes，再输出固定 typed error；原始 bytes 仍可能含上游 prose，保持 wire fidelity，不改该 owner。
- [stream_diagnostics.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/pipeline/stage_send/stream_diagnostics.rs) 仍观察原始 error JSON，并用已有模式 sanitizer 保存有界诊断；本批没有把其任意 prose 全部移除。
- 本批范围是 CompletionError 出口，以及聚合/Responses→Chat 转换生成的错误对象。普通 transport error 仍保留其原类型和语义，不冒称任意网络错误链均由此固定脱敏。

## 3. 实际验证

新单元测试覆盖任意中英文长 prose/恶意 code/type、Display/Debug/source chain、本地固定长度、精确白名单分类、原 ServerError 策略、冲突字段、UTF-8 单字节分片及源释放；新 production-library integration 使用公开库 API 而非源码副本，覆盖 buffered/translated 错误出口和 native 原 bytes/typed error 的明确区分。

### 3.1 先复现并保留 native 合同冲突

`production-contracts-v1.log` 实际编译完成（10m16s），native 合同 **4 pass / 1 fail**；另两套 completeness **8 pass**、本批新错误出口合同 **4 pass**。wrapper 返回 exit 1。失败为 `native_incomplete_is_valid_but_failure_or_invalid_json_is_not`，原文：

```text
assertion `left == right` failed
  left: 2
 right: 1
```

旧断言要求单一 response.failed 输入只得到一个 Err；生产 guard 的既有明确行为是先返回该原始 bytes，再返回 typed error。本批不改 guard；改为检查 upstream failure 的原帧字节完全相同、恰一 typed upstream error；本地 invalid JSON/冲突终态仍只有 parse error。不是将这次合同断言修订伪装为产品 red/green，也没有删除原失败日志。

### 3.2 最终测试结果

| 日志 | 实际 test 数 | 结果 / 范围 |
| --- | ---: | --- |
| `green-responses-v1.log` | 57 | pass；完整原 53 项与新增 4 项，P18 两项失败转绿 |
| `stream-error-v1.log` | 4 | pass；typed protocol/transport/source identity |
| `stream-decode-v1.log` | 13 | pass；有界 SSE/UTF-8/framing |
| `upstream-stream-v1.log` | 47 | pass；观察、终态、错误及取消 |
| `qwen-pipeline-v1.log` | 16 | pass；真实公共 Chat/Responses、流、取消及归属 |
| `provider-failure-v1.log` | 1 | pass；该过滤器只有 1 项，六类兼容另由本批 2 套新测试真实调用 classifier 验证 |
| `finalize-v1.log` | 12 | pass；终态及健康归属 |
| `production-contracts-v2.log` | 17 | pass；非 test seam 生产 library：native completion 5、completeness 8、新 terminal error 4 |

最终 **167 pass / 0 fail / 0 ignore**（单元 150 + 生产 integration 17）；`audit-results.mjs v2` 实际解析八份日志/十个 suite，拒绝空过滤器、失败或 ignore，生成 `test-results-v2.json`。不把 red、生产合同 v1 的重复执行或用例内部循环累计为独立测试数。

完整 Responses 的 test profile 重新编译用时 15m15s，不是 EXE 构建或打包速度。新增生产合同证明 partial 后明确失败不会成为 buffered success 或 translated DONE/成功 finish；单字节分片含 UTF-8，tracked callback 是 Interrupted/false。native 新合同故意检查原始失败 bytes 仍含合成 private marker、指针和内容相等，随后恰一固定错误，并拒绝继续消费后续伪 completion；它不是 wire redactor。

`run-static-v1.ps1 -Version v2` 已实际 exit 0：官方 Rustfmt、checker **31/31**、无报告 ratchet **4027 文件 / 14 immutable / governed source >700=0**、全工作树 `git diff --check`。v1 早期结果也保留。CRLF→LF 提示不是空白失败；未覆盖既有脏报告、baseline 或豁免。

`source-before.json` 在生产修改前记录初始 3 个源码；新增 production 合同和旧 native 合同在改动前另有 `source-additional-before.json`。`source-frozen-v1.json` 在生产合同 v2 启动前冻结全部 5 个源码；此前单元/相邻回归对应的 library owner hash 与最终冻结一致。P19/P20 已冻结 owner hash 均保持匹配。

| 源码 | 前 → 后有效行 |
| --- | ---: |
| `src/protocol/responses/terminal.rs` | 128 → 145 |
| `src/protocol/responses/tests/mod.rs` | 56 → 57 |
| 新 `terminal_error_tests.rs` | 259 |
| `tests/responses_native_completion_contract.rs` | 166 → 171 |
| 新 `tests/responses_terminal_error_contract.rs` | 206 |

259 行单元测试围绕同一个 terminal-error 信任边界，属于 251–500 的单一职责范围，不为少量行数机械拆分，也不申请豁免。新增测试不读秘密、不访问外网；长输入有界，单字节流检查源停止和释放，没有后台进程或无限队列。生产映射只匹配固定字面值，不复制 provider prose，也不触碰认证/存储/网络 owner。

最终 source/documents 和 tracked/untracked diff 检查另在 `source-verify-final.log` / `untracked-diff-final.log` 留存；未跟踪文件 `git diff --no-index --check` 只接受 0/1，不接受 whitespace 的 3。

## 4. 执行与剩余边界

Cargo 由主代理串行，统一 `--locked --jobs 4 --no-default-features --features runtime-defaults`，TEMP/TMP/TMPDIR 指向本批 `runtime-temp`；不启动默认 management-ui / Web 发布清理 hook，P13 已发生授权遗漏仍保留。没有读取真实 `.ng`、历史凭据、数据库或在线实例，没有真实登录、生成、付费上游、EXE 包或部署。

PG probe principal、PG auto-route source reserve 动态取消、lifecycle 无 runtime 独立测试、reasoning/refusal/多 choice/legacy function_call 与完整工具保真、Bedrock 普通参数及 reasoning/signature 和专属完整取消链、其余 API/OAuth/Web/媒体、P50 devtest 包/隔离启动、集中录制及真实逐渠道登录/刷新/生成/额度验收继续开放。以前 feature/前端/expiry 失败不因本批消失，**总 goal 继续 active**。

下一最小实现优先 Bedrock 普通生成参数的双向桥接：输出 token limit、temperature、top_p、stop。只读源码核验显示，`packing.rs` 当前把跨协议通用字段原样放顶层，而 `normalization.rs` 将 native inferenceConfig 整体留在 extra；原生往返有合同，不证明跨协议参数生效。先按权威 API 合同核实 wire/值域，再为 Chat/Responses/Anthropic↔Bedrock 添加 red 与实际最终 body 合同；确定冲突优先级，保护原生未知字段，不把 top_k/penalties/seed 擅自映射。这不替代其余 OAuth/Web/媒体工作包。

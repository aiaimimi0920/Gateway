# 模型声明观测

Gateway 使用现有 dispatch、协议 parser 和管理鉴权提供模型名观测；不增加路由策略、模型鉴真、计费规则或后台供应商探测。

## 读取方式

管理接口 `GET /v1/internal/gateway/operations/summary` 的 `summary.modelObservations` 返回当前进程最近最多 100 条记录，按旧到新排列。沿用现有 management token；普通推理 API Key 不能读取。该字段是向后兼容的可选增量，消费端应允许将来出现新的观测状态。

记录在进程内存中保存，进程重启清空；不会写入用户数据库、usage 队列、审计正文归档或公开 Prometheus 标签。没有为该字段新增桌面专用面板，也不能据此查询完整历史。

示例只表示字段合同，不是真实供应商测量：

```json
{
  "observedAt": 1792411200,
  "providerId": "example-provider",
  "requestedModel": "friendly-alias",
  "upstreamModel": "sent-model",
  "declaredModel": "declared-model",
  "declarationSource": "model",
  "aliasApplied": true,
  "relation": "declared_differs",
  "verified": false,
  "stream": true,
  "observationStatus": "declared",
  "observationEnd": "source_dropped"
}
```

## 字段含义

- `observedAt`：记录发布时的 UTC Unix 秒。
- `providerId`：选中候选的 provider account ID，不含 key、token、凭据内容或完整 URL。
- `requestedModel`：调用者请求的模型名或 alias。
- `upstreamModel`：本次候选实际使用的发送模型，不从下游响应反推。
- `declaredModel`：原始上游响应中实际观察到、且通过有界 ID 校验的字符串；缺失或不能可靠观察时为 `null`。
- `declarationSource`：`model`、`message.model` 或 `response.model`；未知时为 `null`。
- `aliasApplied`：有效 requested / upstream ID 不同时为 `true`，只表示正常映射，不是错误。
- `relation`：自报 ID 与发送 ID 一致为 `declared_matches`，不同为 `declared_differs`，缺少任一有效 ID 为 `unknown`。
- `verified`：本功能始终为 `false`。供应商自报的名称并不证明实际模型身份；正常版本后缀、服务端映射也可能造成差异。
- `stream`：调用者是否请求流式；非流式客户端也可能由已有 adapter 使用流式上游。
- `observationStatus`：有效声明为 `declared`；缺失或未接入原始证据为 `missing_or_unobserved`；已归一化 Bytes 为 `synthetic_or_unobserved`；超限、非法声明、无法解析的 envelope、相互冲突分别为 `observation_limit`、`invalid_declaration`、`invalid_envelope`、`conflicting_declarations`。
- `observationEnd`：buffered 路径为 `buffered`；原始流读到 EOF 为 `eof`，返回原始错误为 `transport_error`，观察器被拥有者释放为 `source_dropped`。

**`source_dropped` 不是“用户取消”或“请求失败”的证明。** 现有转换器也会在收到合法终态后主动释放原始流。本字段描述观察器生命周期，不替代既有 completion、请求终态或健康状态。

## 覆盖和 unknown 边界

非流式路径覆盖 OpenAI Chat、Responses、Anthropic 原始 JSON parser 及原生 JSON passthrough；保留原有 canonical model fallback 和下游 alias 输出。DashScope 的 Chat 投影会填入调用参数，因此显式仅从投影前的原始 body 提取证据。

流式观察位于 HTTP 原始 bytes、协议转换之前，读取已知 Chat choices envelope、Anthropic `message_start.message.model` 和 Responses response-level envelope。支持跨 chunk、UTF-8 BOM、CRLF 和多行 data；不递归扫描正文中的 `model`。工具注入的 buffered Anthropic→Responses 聚合链同样在转换前观察，转换后的合成模型声明不作证据。

没有原始来源信息的 `UpstreamStreamingResponse::Bytes` 一律保持 unknown。Gemini/Web/reverse/custom 等未接入明确原始声明提取的 adapter，以及部分 adapter 内部 forced-stream 聚合，不因最终 canonical model 有值就被推断为 declared。媒体、二进制、缓存命中及尚未建立成功上游流的错误不保证产生本记录。因此，这不是所有请求的完整审计，也不声称全部 provider 已具备自报证据。

## 有界性和安全

每条仅保留最多四个短 ID、来源、标记和时间。ID 最多 256 字节，只接收 ASCII 字母数字及 `/._:-+@`，拒绝控制字符、空白、常见秘密前缀；不截断后伪造一个新 ID。未知 ID 为 `null`，不能用于授权或定价。

SSE 只为解析 envelope 暂存有界帧：每帧最多 64 KiB / 1024 行，每条流最多观察 8 MiB。超过观察预算后，已有声明也降为 unknown，不再解析；**原始流不受此预算截断或报错**，仍交由原协议及完成证明 owner 处理。临时帧不会进入长期记录，提示词、回答、reasoning、工具参数、认证头不会作为观测字段保存。

ring 使用独立短临界区，无每模型无限 map、后台任务或额外请求；EOF、error、Drop 最多发布一次。它不修改重试次数、请求 deadline、账号授权、健康冷却或扣费模型。

本功能没有结构 fingerprint 学习、候选模型推断、自动 suspect gate 或跨账号 opaque reasoning 转发。

## 本地验证入口

```powershell
cargo test --locked --offline --lib model_ -- --test-threads=1
cargo test --locked --offline --test model_evidence_contract --test responses_reasoning_config_contract --test account_protocol_routing_contract -- --test-threads=1
```

验证使用隔离 loopback 上游和合成凭据；源码测试不等于真实供应商、打包 EXE、部署实例或业务验收。

# 上游计费表达式错误的智能路由回退修复

日期：2026-11-02。范围：共用上游错误分类、发送前候选切换与健康反馈。

## 根因与修改

lanln 的真实请求返回 HTTP 400、`new_api_error / model_price_error`，主消息为
`model gpt-6.1-sol tiered expr run failed: expr compile error: unknown name cc12h (1:62)`。
旧逻辑按普通 `BadRequest / Abort` 处理，导致后续候选不再尝试。
这属于中转站的预计费表达式配置错误，不是调用方请求参数错误。

新增 `src/error/model_pricing.rs`，对有界的主错误消息进行精确识别；只接受
HTTP 400/500/502/503，且 code 缺失、为空或为 `model_price_error`。不扫描 prompt、
任意 JSON 字段，不覆盖冲突的认证/参数 code，也不把所有 HTTP 400 当作可重放错误。
分类改为 `ServerError / FallbackProvider`，保留原始 status、code、provider 及脱敏消息，
`retryable=false`，不重复请求同一失败候选，继续现有智能排序后的合法备用候选。

健康反馈记为非永久的 `ProviderTransient / CredentialModel`，接入既有失败评分、
黏性释放和模型熔断，不把这类配置故障永久判定为凭据无效。SSE 在输出移交前的同类
结构化错误也进入该流程，保留计费错误的主消息和 code。

## 安全与资源边界

- Chat/Responses 即使声明 tools，明确的预计费拒绝也可切换下一合法候选；媒体创建不新增重放资格。
- 已输出或响应已移交后禁止自动换供应商重放；不能重复工具或内容生成。
- 原请求 deadline、总计最多 6 次发送、Key/账号组/模型/服务商授权及其他准入限制保持不变。
- 不修改请求模型、不解除手动禁用；没有合规备用时仍返回真实失败。
- 不增加后台任务、持久化状态或依赖；识别输入最多 4096 字节，诊断继续脱敏并限长。

## 验证结果

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-model-pricing-fallback-20261102/`。

1. 修复前红测：`pricing_compile_error_is_provider_failure_not_bad_request` 按预期失败，
   实际 `BadRequest`，期望 `ServerError`，见 `red-pricing-classification.log`。
2. 最终定向 Rust 测试共 **99 passed、0 failed**，见 `focused-final.log`：
   - HTTP 客户端错误分类 4 项；模型容量分类 5 项；发送运行时 60 项。
   - 请求预算 12 项；拒绝后的候选预算 8 项；回退授权 4 项；本次计费错误合同 6 项。
   - 覆盖 Chat/Responses、流式/非流式、tools、智能排序、无授权负对照、六次预算、
     无备用保留原错、SSE 输出前切换及输出后禁止重放。
3. 使用本轮实际编译的 debug Gateway，在独立数据目录及随机端口运行公共 API 验收，
   详见 `public-api-smoke-receipt.json`。合成上游依次返回计费 HTTP 400 和成功响应：
   - `/v1/chat/completions` JSON + tools：HTTP 200，有效回复 `42`。
   - `/v1/responses` SSE + tools：HTTP 200，收到 `response.completed` 及 `42`，
     不向调用方泄露被成功回退的 `model_price_error` 或 `response.failed`。
   - 每次计费失败候选及成功候选各调用一次，模型始终为 `gpt-6.1-sol`，tools 保持不变。
   - 独立进程已停止，真实上游调用次数为 0，在线 4200 实例未重启。
4. 最终格式、checker 自测、有效代码行 ratchet、diff 检查见同目录 `*-final.log`。
   有效行数及 UTF-8 无 BOM 检查见 `verification.safe.json`；不重建 baseline。

第一轮绿测有一个新增测试 fixture 顺序错误：在 `RequestBudget::run()` 之前耗尽了工具
预算。已将 `begin_attempt()` 移入被测 `run`，未为测试错误改动生产代码。公共 API 首次
fixture 将配置误放在数据根目录，已修正为 `<data>/local/routes.yaml` 后通过；初次失败
记录保留，不作为真实生成失败。编译仍有工作区既有 unused/dead-code 等警告，不算零警告构建。

## 发送前拒绝的参考依据

已联网核对 [New API 固定提交](https://github.com/QuantumNous/new-api/tree/70e6af6429824f99de98e96e65254c8c2ef7ee0b)：

- [price.go:396-402](https://github.com/QuantumNous/new-api/blob/70e6af6429824f99de98e96e65254c8c2ef7ee0b/relay/helper/price.go#L396-L402)
  在预计费阶段运行表达式，completion tokens 为 0，错误格式与本次真实消息一致。
- [compile.go:199-218](https://github.com/QuantumNous/new-api/blob/70e6af6429824f99de98e96e65254c8c2ef7ee0b/pkg/billingexpr/compile.go#L199-L218)
  返回 `expr compile error: ...`。
- [request_billing.go:59-67](https://github.com/QuantumNous/new-api/blob/70e6af6429824f99de98e96e65254c8c2ef7ee0b/relay/request_billing.go#L59-L67)
  将其包装为 `model_price_error` / HTTP 400，在预扣费前返回。
- [relay.go:160-177](https://github.com/QuantumNous/new-api/blob/70e6af6429824f99de98e96e65254c8c2ef7ee0b/controller/relay.go#L160-L177)
  预计费失败直接返回，尚未进入渠道 relay/retry 循环。

上述源码仅作为精确拒绝信号的设计依据，未声称已核实 lanln 实际部署版本，
也没有复制第三方实现进 Gateway。

## 交付状态

源码修复及隔离公共 API 验证完成；未编译手测桌面包，未替换或重启用户运行的 EXE，
未提交或推送。本轮未真实重放 lanln 请求，也不把站点模型目录可读或用户暂记可用
改写成该模型已经生成成功。

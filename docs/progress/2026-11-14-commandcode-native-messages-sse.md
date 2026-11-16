# Command Code 原生 Messages SSE 扩展

范围：首批三服务商中的 Command Code 回复侧流式兼容；不扩大 S06/Gemini 授权，不代表整个服务商完成。

## 合同与修改

本轮已联网核对 Anthropic Messages streaming 合同：
https://platform.claude.com/docs/en/build-with-claude/streaming 。
它允许多个累计用量的 `message_delta`、thinking/signature、完整内容块、ping 和未知事件扩展。

- 原生回复保留 thinking、signature、redacted thinking、server tool、citations 和未知扩展；不再强制重建为文本／函数工具 canonical 内容。
- 抽出块校验模块；已知块保持类型校验、工具参数唯一完整 JSON、共享参数预算与签名结束条件。
- 多个 `message_delta` 使用最新累计计数，不累加重复统计；允许 provisional null reason，但必须有一致的非空终态才能结束。
- 终态及后续控制事件仍待 clean EOF 才公开；尾部错误仍脱敏、失败退款且不重放。
- 终态缓存新增共享字节预算，沿用已有 64 MiB 限制。测试直接检查 admission 边界与整数溢出，不构造巨型夹具或改小生产限额凑绿。
- 复用现有 usage/completion tap 增加专用 Messages 模式，只观察 native 生命周期事件和 native 用量字段；未知事件中的 `usage/status/output` 以及别名计数不能污染计费或终态。其他服务商默认模式不变。
- 原生 type 同时是输出 SSE event 字段，拒绝 CR/LF，避免扩展类型注入额外事件行。

## 验证

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-first-three-20261113`。

- 红测：`commandcode-sse-extensions-red.log`，1 passed / 1 failed；完整扩展流在旧 guard 被拒绝，符合预期。
- 绿色产品回归：`commandcode-sse-extensions-green.log`，**66 passed / 0 failed**，exit 0。编译耗时 18m23s（含共享构建锁等待），测试 73.90s；涵盖 1/47/8192 字节碎片、完整内容等值、四种终态、公开计费 5 tokens、错误退款和单次发送。
- 共享回归：`commandcode-sse-shared-stream.log` **53 passed**，`commandcode-sse-shared-anthropic.log` **60 passed**，均复用本次实际生成的测试 EXE。
- 有效行 checker **31 passed**，最终 ratchet、定向 rustfmt 和 scoped diff whitespace 检查通过。
- 改动文件有效行：stream guard 236 → 251，新 block 校验 107，usage 257 → 298，completion 171 → 225，translation 338 → 341，terminal 279 → 285；未增加大文件例外或修改基线。
- 独立 feature 构建沿用前序回执，本切片未再次执行；当前源码已通过上述 runtime-defaults + copilot-sdk-runtime 产品测试编译，不将其冒充所有 feature 矩阵通过。

## 保留边界

此切片不是请求侧原生保真、Responses SSE 完整扩展、工具注入路径、System One/Jev、ZDR、真实 Server 部署或当前源码 EXE 包验收。Command Code 仍为开发中，SenseNova Token Plan 与 SAP AI Core 仍排队。

# Command Code 原生 Responses SSE 收口

范围：Command Code 原生 Responses 回复流、公开模型别名、终态校验及用量／完成语义观察。
这不是服务商整卡完成，也不扩大此前 S06/Gemini 的窄授权。

## 现状与修复

本轮联网重新核对 https://commandcode.ai/docs/provider ：Responses 使用 OpenAI Responses 格式，终态携带用量，支持客户端工具及转交上游的内置工具声明。

公共入口红测确认：目录选路已经保留原生扩展事件，问题不是该路径强制重建为 Chat；实际失败是未投影公开模型别名，以及非法终态仍可能公开为成功。

- guard 继续有界解码、唯一 JSON 校验、错误脱敏与 clean EOF 后发布终态；新增 event/type 一致性、事件类型 CR/LF 拒绝、终态 envelope 校验与 event/status 一致性。
- 仅对已知 Responses 生命周期 envelope 的 `/response/model` 投影公开别名；不递归改写工具内容或未知扩展字段。原始模型证据仍在 guard 前观察。
- 保留 reasoning、refusal、custom-tool、web-search 与未知 `response.*` 事件，不另建有损 canonical 重构路径。
- 复用已有 usage/completion taps，增加明确的 Command Code Responses 观察模式；只读取已验证终态，复用产品 JSON 的原生用量与完成语义。不把未知事件的 usage/status/output 当控制数据，不让别名计数字段覆盖 native 计数。
- incomplete 的 content_filter 等原因优先于输出中的工具类型，不统一误记成 tool_calls 或 length。
- 独立只读审查发现完成观察器最初条件过宽，已按请求局部 candidate 的 `/responses` 选择结果收窄，并增加 Chat→Responses 旧桥接的隔离回归。

## 验证回执

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-first-three-20261113`。

- `commandcode-responses-sse-red.log`：**0 passed / 2 failed**，公开模型别名等值断言、非法成功终态抑制断言均复现。
- `commandcode-responses-sse-green.log`：初版 **70 passed / 0 failed**。不是最终源码回执。
- `commandcode-responses-sse-final.log`：包含观察器接线范围补强和桥接隔离回归的最终源码 **71 passed / 0 failed**，exit 0。编译 12m26s，测试 77.47s。
- 最终测试 EXE 的共享回归：stream **53 passed**、Responses **60 passed**、Anthropic **60 passed**、SenseNova **24 passed**、SAP **26 passed**；见 `commandcode-responses-sse-shared-*.log`。
- 有效行 checker **31 passed**，最终 ratchet、定向 rustfmt、scoped diff 检查通过。
- 有效行：Responses guard 71 → 120，新 guard 测试 73、新公共入口测试 140；共享 stream 181 → 189、usage 298 → 322、completion 225 → 246；preflight 375 → 376、translation 341 → 347、terminal 285 → 292。无新增大文件例外，无基线调整。
- 本轮没有再跑独立 feature 矩阵，也没有生成 EXE 交付包或访问真实账号；不以 runtime-defaults 测试编译代替这些验收。

## 尚未关闭

请求侧完整原生保真、System One/Jev、ZDR 与产品资格、适用的真实 Server／多实例、当前源码 devtest 包及隔离运行流程仍由首批三服务商总清单管理。此处合成上游结果不能替代真实账号验收。

下个功能块的只读定位：`runtime.rs` 在原生分流前调用通用 text/function admission，尚会拒绝 reasoning、媒体和 custom tools；仅替换 plan 为 raw clone 不能解决。须采用产品内同协议 admission 与保真请求准备，并同步 canonical 已生效的字段修改，避免 raw 旧值绕过策略。跨协议转换仍需独立保持契约。现有 `stage_send/tool_bridge.rs` 明确排除 Command Code 工具注入，应继续走原生工具，而不是为注入器增加一条平行模拟路径。

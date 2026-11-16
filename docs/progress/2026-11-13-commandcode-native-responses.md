# Command Code 原生 Responses 文本与函数工具切片

本次是首批三个服务商中的 Command Code 增量实现，不是整个产品完成回执。

## 实现与边界

- 将保存的目录协议声明编译为内部 metadata，仅对声明支持 `/responses` 的模型和公共 Responses 请求选择原生路径。选择只修改请求局部 candidate，不修改保存的账号，不把目录声明标成推理验证通过。
- 原生请求使用固定服务商地址及账号 Bearer，不泄漏调用方身份或内部目录字段；保留原有无跳转、不重放规则。旧配置和 Chat 路径不变。
- JSON 回复使用有界、重复键拒绝的解析；拒绝上游失败、错误正文及不能保真的输出。SSE 先完整解码再发布，失败与取消正文脱敏，终态等严格 EOF 后才发布，截断不能伪装完成。
- 新 Responses pack 入口保留所有 system 消息及文本 part 的顺序；既有通用入口行为不变。
- 函数工具两回合、用量审计、额度扣减和失败退款通过真实 LocalRuntime 与合成上游验证。

当前仍只导入 Chat-capable 模型；Responses-only／Messages-only 不在本次扩池范围。原生 Messages、reasoning／媒体／refusal 保真、System One/Jev、ZDR 及其他交付门槛仍未完成。特别是 JSON refusal 当前明确报错，不能当作已支持，也不能静默丢弃为成功。

## 验证

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-first-three-20261113`。

- `native-responses-r3.log`：`commandcode_` 筛选 **39 passed / 0 failed**；编译 10m08s，测试 68.32s。使用 `runtime-defaults,copilot-sdk-runtime`，不是三个服务商的独立构建回执。
- `shared-responses.log`：复用同一测试 EXE，`protocol::responses::tests` **47 passed / 0 failed**。
- 公共 JSON/SSE 工具流程验证实际 `/responses` wire、system 多消息多 part、账号身份隔离、5 tokens 审计及消息额度 2→0。
- 失败测试覆盖 JSON failed、SSE failed／cancelled／截断，检查已发布 chunk 不泄漏失败正文，不重放且退款。
- r2 曾有 **38 passed / 1 failed**：测试保留响应流引用导致审计等待超时。r3 显式释放测试持有的流引用后通过；没有为通过测试修改生产生命周期语义。
- `gate-checker.log` 的 checker 31 项与 `gate-ratchet.log` 通过；定向 rustfmt 和 scoped diff 检查通过。新增／修改文件在本次行数清单中均低于 500 有效行，未修改 baseline。

没有调用真实服务商、修改真实凭据或运行实例，也未出包、部署、提交或推送。UI、持久化重启、原生包及适用 Server 验收不以本次测试替代。

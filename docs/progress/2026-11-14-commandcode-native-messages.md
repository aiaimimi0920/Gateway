# Command Code 原生 Messages 文本与函数工具切片

本块继续首批三个服务商中的 Command Code，不接管 Gemini 功能，不并行扩展后两个产品。
上一块构建门禁已解除；本块目标是让公共 Messages 实际使用目录声明的原生上游，而非转 Chat。

## 合同与实现

2026-11-14 联网复核官方 https://commandcode.ai/docs/provider ：Messages 使用
`POST /provider/v1/messages`，账号 Bearer Key 鉴权；模型的 `supported_endpoints` 决定可用端点。
官方当前描述 Claude 模型使用 Messages，因此只做 Chat 桥接不能算原生接入。

- 本地候选按选中凭据及实际 upstream model 的目录声明派生协议能力；原生选择仍受 allowed／forced family 限制。
- 只修改请求局部 path 和 family，保持产品 adapter、固定地址、专用 Bearer 及禁止身份头混入的发送边界。
- 专用 Messages planner 禁用隐式自动 cache 标记，保留原生 system 文本块和工具选择；JSON reader 在有界、重复键拒绝及内容／用量检查后转换 canonical 回复。
- SSE 检查事件次序、block 索引、文本／函数类型、失败及错误尾部，终态等干净 EOF 后才发布。所有打开工具的参数共享有界预算，在 block stop 验证完整且无重复键的 JSON object。
- 原生流绕过错误的 Chat→Messages 二次转换；保留 Messages usage tap，补齐 data-only 帧的 event name，并把公开模型投影为调用者别名。原始上游模型仍由前置观察器记录。

## 验证记录

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-first-three-20261113`。

- `native-messages-red.log`：先运行新增公共两回合测试，预期失败 **0 passed / 1 failed**。
  现有 Chat reader 接收 Messages 回复返回 502，证明接线缺口真实存在。
- `native-messages-r1.log`：初版编译失败，family 比较闭包变量遮蔽引发 `E0277`。
  已改为目标 family 与 allowed family 明确比较，并补强制／允许协议矩阵测试。
- 审查同时补齐流工具参数完整性；截断、array、null、顶层／嵌套重复键均有测试，合法中文参数使用单字节 chunk 验证。
- `native-messages-r2.log`：修正后的产品定向回归 **44 passed / 0 failed**，编译含锁等待 15m46s，运行 81.99s。
  使用 `cargo test --locked --offline --lib --no-default-features --features runtime-defaults,copilot-sdk-runtime --config profile.test.package.gateway.debug=0 -j 2 commandcode_ -- --test-threads=2`。
- 同一测试 EXE 的 `routing::protocol_resolution::tests` **35 passed**（`messages-routing.log`），
  `protocol::anthropic::` **60 passed**（`messages-anthropic.log`）。路由回归含上述产品测试，不相加计数。
- `messages-feature.log`：`cargo check --locked --offline --lib --bins --no-default-features --features line-commandcode-provider-api -j 2` 通过。
- 公共用例包括仅声明 Messages 的账号、JSON／SSE 两回合工具、身份隔离、system、工具选择、用量及额度；失败用例包括错误正文、截断、成功终态后的 error 及不完整函数参数，要求退款且不重放。
- `messages-r2-checker.log`、`messages-r2-ratchet.log` 通过。`messages-r2-lines.json` 中所有本块文件均不超过 500 有效行，未修改 baseline。
- 定向 rustfmt、scoped diff 和源码 UTF-8 无 BOM 检查通过，源码及测试 EXE 哈希在 `messages-source-hashes.json`。

## 尚未收口

这里不是完整 Messages 或整个 Command Code 产品完成回执。自动目录导入仍只纳入 Chat-capable 模型；
本次 Messages-only 测试通过保存配置提供声明，不能代替自动目录导入和专用 probe 的闭环。
专用 probe 仍固定 Chat，待补目录驱动选择，不得以失败后改端点重放推理探测。

reasoning／媒体／refusal、未知 block 保真、非流 stop_sequence 等输出信息、System One/Jev、
ZDR、管理重启、UI／可运行手测包及适用 Server 验收仍开放。不能把明确拒绝未实现内容视为完成支持。
未调用真实上游，未修改真实凭据／运行实例，未出包、部署、提交或推送。

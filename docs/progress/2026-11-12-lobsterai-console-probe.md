# LobsterAI 专属控制台模型测试

## 实际缺口与修复

LobsterAI 公共推理已有专属目录和 sender，但显式控制台模型测试此前仍落入通用生成路径。新增回归在未改生产代码时失败，报告为 `Model call failed. Model: o3-synthetic-chat. Cannot reach the NVIDIA model endpoint.`，证明该入口没有复用 LobsterAI 产品链路。测试仅使用合成凭据及不转发的 loopback 上游，不是真实上游认证失败。

本批增加 `provider_runtime/lobsterai_model_probe.rs`，从既有控制台模型分发接入：

- 要求显式模型测试及已配置模型／别名；默认无 options、禁用账号、未配置模型不发送。
- 保留精确账号，审计初始化后核对当前 target；虚拟解析后再次核对，不静默改用另一凭据。
- 复用 catalog 声明、每请求虚拟解析、固定 Chat／Messages sender、无跳转及产品身份头。保留现有特殊 profile／Kimi／媒体等拒绝边界。
- 提取既有 Ticket 的按 provider ID 捕获入口，公共 candidate 调用仍委托同一实现；捕获 access／context／generation 和 readiness。真实 sender 401 只通知既有 durable refresh owner，不新建刷新 owner，也不重放当前推理。
- 60 秒整体预算，max_tokens 256，非流式请求；仍接受产品支持的 JSON 或完整 SSE 回复。响应体上限为 64 KiB，完整文本最多 8192 字，不把截断文本当成功。
- probe 专用 Chat 准入在 canonical 转换前拒绝多 choices 和非 null refusal；再检查完成文本、非空、无工具。公共读者仍使用原有 16 MiB 限额与既有行为。
- 复用取消安全 audit／health owner。未知 usage 保持未知；Messages 审计保留原生 input/output 和 cache 分项，不按公共 Chat 口径再次加算。成功响应不从可伪造的 canonical `_upstream_status` 推定 HTTP 状态。
- 虚拟或 Messages selection 写入 `upstreamModelResolution` 和协议 family；原测试模型身份仍用于健康反馈。输出中的 access token 脱敏，原始上游错误不进入报告。

SSE 继续沿用 LobsterAI 的既有终态合同：text + `[DONE]` 可投影为 canonical stop，不声称一定观察到了原生 finish_reason。错误尾、未完成 EOF 等仍由既有 stream owner 拒绝；probe 的额外检查不是另一套完整 SSE 协议实现。

## 本地验证

新增 8 项测试，覆盖：

1. 显式控制台调用走固定产品 sender，精确 Bearer、配置别名、审计与 5 tokens。
2. 普通目录／`yd-fast` × Chat／Messages × JSON／SSE；虚拟请求先 resolve 后推理，真实 concrete model 和协议记录进审计，外来 cookie／key／account 头被清除。
3. 真实 HTTP 401 唤醒已有 refresh worker；在显式第二次调用前仅有失败推理和 refresh，后续调用才使用新 token，无隐藏重放。
4. 默认、disabled、错误模型及过期捕获 target 不发送。
5. 多 choices、JSON/SSE refusal、工具响应、length、空文本、超长文本、超大 body 和截断 SSE 均失败。
6. 无 usage 仍可返回完整文本，但审计用量未知；敏感文本脱敏，DONE-only 终态语义明确验证。
7. 307／403／429／500 不跳转、不重放、不泄漏错误正文。
8. 取消释放响应体，记录 cancelled／未知用量，不写健康状态。

最终命令：`cargo test --locked --offline --lib -- lobsterai --test-threads=2`。

**87 passed / 0 failed / 0 ignored，其中新增 8 项**。覆盖现有 LobsterAI 管理、刷新、目录和公共推理回归；测试执行 53.11 秒，完整命令 859.80 秒，包含编译。前置 red 回归为 0 passed / 1 failed，完整命令 1540.22 秒（含共享构建锁等待和编译）；失败记录保留，不与最终 green 混报。

有效行 checker 31 项、ratchet、scoped rustfmt、diff 检查通过。13 个 Rust 文件测试前后 SHA256 一致，UTF-8 无 BOM；未修改 baseline／例外。主要有效行：console 分发 418→422，recording 249→264，dispatch 65→72，runtime 150→163，Chat reader 137→152，Messages reader 164→177；新增 probe 147、准入模块 45、测试 233／155。所有本批修改文件均不超过 500 行。独立只读窄审查未发现阻断缺陷，不替代测试或真实产品验收。

证据：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-lobsterai-console-probe-20261112`，含 before 快照、red／green 日志与回执、源码哈希、scoped.diff、final-validation.json。只声明本批文件来源，不声明整个共享 dirty worktree 的完整来源。

## 未完成范围

本批调用真实 console probe 分发函数和 LocalRuntime，不冒称已完成管理授权／命名计划／自动调度端到端验收。真实账号、订阅费用、自有身份及能力头接受性、跨请求持久虚拟绑定、特殊模型、Server／Redis、独立 feature 和原生包仍开放。

全范围仍为 67 个工作项未完全关闭，不关闭 LobsterAI 整卡。未读取真实凭据，未提交、推送、出包、部署或修改运行实例。

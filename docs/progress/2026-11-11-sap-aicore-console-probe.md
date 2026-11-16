# SAP AI Core 专属控制台模型测试（2026-11-11）

状态：本地专属模型测试子项通过；SAP 整体仍为 `partial_pass`。不将本地合成测试等同真实账号、费用、部署发现或完整产品验收。

## 改了哪些内容

- 显式模型／提示词测试接入 SAP 专属 sender，保留精确账号和已配置部署／别名解析。临时、保存及命名测试计划共用该入口。
- 默认无提示词、无测试计划的连通性检查仍不生成、不交换 token；通用目录、发现和普通 Key header 通道仍拒绝 SAP。
- 复用已有 Service Key `client_credentials` owner、缓存和目标校验，不新建刷新任务，不自动重试或跨账号回退。推理 sender 仍只发送短期 token 和资源组，不发送完整 Service Key。
- 探测完整响应最多 64 KiB，单次总预算 60 秒；只接受完整非空文本、正常 stop，无工具调用或拒绝，最多 8192 字符。拒绝坏 JSON、重复字段、矛盾用量、截断和超长输出。
- 在答案预览、评分和持久化前，脱敏完整 Service Key、clientid、clientsecret 和本次实际使用的 access token。错误只保留本地诊断，不保存上游错误正文。
- 复用取消安全的审计与模型健康 owner；记录实际用量，缺失用量保持未知。探测不使用普通接入 Key，不扣其余额；这不承诺上游免费或取消退款。

## 验证

- Rust 定向联合回归：**41 passed / 0 failed / 0 ignored，其中 SAP 26 项**。
- 命令：`cargo test --locked --lib -- sap_aicore provider_runtime::qwen_model_probe provider_runtime::model_probe protocol::openai::chat_admission --test-threads=2`。
- UI 相关回归：3 个文件、13 项通过；为现有 SAP 模板、CredentialTestDialog、CredentialTestManager，不包含真实浏览器视觉验收。命令中另有一个未匹配文件路径，未计为测试文件或通过项。
- checker 31 项、ratchet、定向 rustfmt、UTF-8 无 BOM 与 scoped diff 检查通过。
- 本批 10 个 Rust 源文件与编译前快照一致；不声明整个继承 dirty 工作树的源码来源。

新增 7 项 Rust 测试覆盖实际管理 Secret Grant、外部账户拒绝、禁用账户／错误模型不外发、单次／临时／命名／已保存计划与结果读取、真实 loopback OAuth+Chat wire、token 复用、逐字段与 token 脱敏、未知用量、坏响应和超长输出、307/401/403/429/500 不重放、取消关闭源连接与取消审计。原有公共推理 JSON/SSE、工具、授权缓存与管理边界回归同轮通过。

首轮 Rust 为 40 passed / 1 failed：新增用例错误地以仅含 credentialIds 的 JSON 请求执行已保存策略，现有解析器正确返回 400。用例已改用现有精确账号空请求体入口，并校验 policySource=account；生产校验没有放宽。首次失败与最终重跑分别保留。

## 尚未完成

真实 SAP 账号／部署权限／费用、自动部署发现、其他协议、自动调度的 Server／Redis 与原生程序验收、Server／Redis、独立 feature 裁剪、原生 EXE 和真实浏览器验收仍开放。没有读取真实凭据或 `.ng`，没有提交、推送、出包、部署或重启。

证据：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-sap-probe-20261111`。完整工作清单仍按 [Magpie 看板](../providers/magpie-workboard-2026-11-08.md) 逐项追踪，不因关闭该子项减少厂商工作项分母。

补充（2026-11-12）：本地真实定时循环及 LocalRuntime 存储重开后的间隔保留已验证，见 [自动调度验收](2026-11-12-commandcode-sap-automatic-probes.md)；不代表 EXE 进程重启或 Server／Redis 验收。

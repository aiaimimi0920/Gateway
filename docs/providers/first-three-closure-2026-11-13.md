# 首批三家服务商：交付与验收

截至 2026-11-24，**Command Code、SenseNova Token Plan、SAP AI Core 已按调整后的范围完成本轮开发**。真实账号、租户权限和费用未验；没有切换 `4200`，也没有正式发布。以下包级结果来自隔离环境和合成上游，不代表所有能力都在同一个包里验过。

## 1. Command Code

- **已交付：**Studio 手工创建 Key、按 `supported_endpoints` 声明接入原生 Chat／Messages／Responses（JSON／SSE）、System One/Jev、ZDR、目录及自动探测。原生 Chat 支持用户图片，不推断逐模型 Chat reasoning 能力。配置见 [Provider API 合同](commandcode-chat-contract-2026-11-10.md)。
- **已验：**三协议及 System One 隔离 Hosted 双进程推理见[合批回执](../progress/2026-11-23-provider-media-admission.md)。[调度和桌面回执](../progress/2026-11-22-commandcode-packaged-scheduler.md)分别记录双后端实际调度周期、原生窗口添加／保留 Key 编辑／重启读回；探测审计与公共调用审计分开。
- **未验或延期：**真实 Studio Key、套餐权限及费用未验；OAuth 自动签发、远端 Responses 查询／取消／删除，以及 System One 现金预付不纳入本轮。后者现有入口在发送前明确拒绝，不影响消息预付。

## 2. SenseNova Token Plan

- **已交付：**`token.sensenova.cn/v1` 的订阅 Key、目录、原生三协议（JSON／SSE）、合同允许的图片和推理历史，以及发送前内容校验。配置与两套 API 域名边界见 [Token Plan 合同](sensenova-token-plan-contract-2026-11-10.md)。
- **已验：**[Hosted 双进程推理](../progress/2026-11-22-sensenova-packaged-hosted.md)、[双进程自动探测](../progress/2026-11-23-sensenova-packaged-scheduler.md)、[目录与原生窗口保存／重启](../progress/2026-11-22-sensenova-packaged-catalog.md)各有独立回执。[合批回执](../progress/2026-11-23-provider-media-admission.md)验证图片、工具推理历史和非法输入零发送；后续加强的内容块校验仅有[源码回执](../progress/2026-11-23-sap-stored-responses.md#拒绝记录与消息预付恢复)，未出新包。
- **未验或延期：**真实 Key、订阅权限及费用未验。视频、第三方控制台 OAuth／积分池延期；重开须有同一 Token Plan 域名的可执行 Chat 视频请求示例与限制，或正式第三方应用注册及授权合同。不借用第一方登录或另一 API 域。

## 3. SAP AI Core

- **已交付：**Service Key（client secret／X.509）、只读部署发现、Azure Chat、原生无状态 Responses、Orchestration v2／Bedrock Converse 及合同列明的工具、图片、推理组合。部署设置与各开关见 [SAP 接入合同](sap-aicore-chat-contract-2026-11-10.md)。
- **已验：**[Hosted 工具与媒体矩阵](../progress/2026-11-23-sap-stored-responses.md#2026-11-24双-exe-工具续轮与-tls-拒绝)为 **48/48**；[原生 Orchestration 目录、账号保存及重启](../progress/2026-11-22-sap-orchestration-discovery.md#2026-11-23-原生窗口成功查询并保存)有独立桌面回执。
- **持久化 Responses：**[受控合同](sap-stored-responses.md)默认关闭，覆盖创建、续接、查询、取消、删除与有明确耐久依据的本地账务恢复。[前台 SSE 包级回执](../progress/2026-11-23-sap-stored-responses.md#双-exe-sse-验收通过)验证两个隔离 `gateway.exe` 共享 PostgreSQL／Redis 时首帧前持久化、截断后跨进程 GET、单次结算和审计。这里的“双 EXE”是同一后端程序的两个进程，不是桌面壳加后端。后台 SSE 已实现 `starting_after` 续接，Local 定向 Nextest **28/28**（`332510c2-fd82-4ee8-b237-e86410e1f987`），合成上游只收到一次 POST、断线后两次 GET，结算幂等；未出后台 SSE 新包，未验 Hosted 双实例或真实租户。
- **未验或延期：**真实部署资格、租户及费用未验；未知提交对账和人工处置未实现。重开须取得能把原次未知发送与 SAP 对象或操作结果唯一绑定的权威证据。普通 GET、人工填写 ID、观测标签或查无记录都不能代替该证据。现金 tariff 及未列出的协议／媒体组合不在本轮范围。

## 验收回执怎么查

- [三家合批媒体与接入回执](../progress/2026-11-23-provider-media-admission.md)：API devtest `gateway-devtest-provider-media-admission-20261123-194746`，合计 **49** 条成功公共审计。
- Command Code 专项 API devtest `gateway-devtest-commandcode-systemone-20261123-084916`：在 `linshi/gateway-packaged-provider-fixture/` 下分别查阅 `commandcode-systemone-gateway-devtest-commandcode-systemone-20261123-084916-2a2f0769498a43609ba05bbb6bc30d4e/result.json`（System One）、`commandcode-hosted-systemone-package-92247c3340b94d6d8e78edd6dc08229b/result.json`（三协议）和 `commandcode-scheduler-systemone-package-953a70bd1db04dc59aa0a81ed57dcb79/result.json`（调度）。三份回执绑定同一 EXE SHA-256 `191b3f70d27983250dc15454d629a50dfbdbfbdac2e053c3ed55d948487983ec`。
- [SAP 持久化与恢复回执](../progress/2026-11-23-sap-stored-responses.md)：公共生命周期、恢复、工具矩阵与前台流各自绑定包名、回执和适用边界。
- 更广的产品范围见[服务商扩展清单](magpie-remaining-2026-11-13.md)。原逐项开发计划和中间尝试不再作为交付状态；后续只在对应范围重新立项。

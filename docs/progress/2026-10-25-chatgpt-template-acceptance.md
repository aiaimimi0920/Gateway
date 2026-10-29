# ChatGPT 标准模板原生通道验收（2026-10-25）

## 结论与范围

`chatgpt` 标准模板已完成新调用流程的原生通道接入与真实验收，登记 `flowVersion: 1` 蓝色完成标记。
通过 UI 创建空服务商、导入一个既有账号，再完成 UI 手动测试和普通 Gateway Responses 的
非流式、SSE 流式调用。三次均获得已完成的非空回答 `OK`，不是仅模型列表或管理接口 HTTP 200。

本轮仅使用独立后端 `127.0.0.1:4300` 和源码 UI `127.0.0.1:1435/ui`。原 4200 实例未停止、
重启或修改。原 ChatGPT 配置与本轮导入材料已保存在用户私有备份目录，不进入源码或公开证据。
只复制现有 Access Token 与 Account ID，不复制或使用 refresh token，避免干扰原实例续期。

## 实现与实测发现

- 标准模板固定官方 Base URL、调用适配器、指纹适配器和专用适配器；仅创建空池。
  UI 显示 OpenAI Responses、Codex、无专用适配器，Basispoints 默认关闭。
- ChatGPT 手动模型测试不再自行拼接 URL 并直接 POST；改为使用生产 dispatcher 与公共 planner，
  落实三适配器配置、extra body 和客户端指纹，保持精确账号调用，不额外路由或 fallback。
- probe 保留 60 秒生成/读取超时、probe 层 256 KiB SSE 累积上限、完成事件和非空文本校验。
  错误不回显上游响应体，成功文本先脱敏；兼容完成事件 output 为空但已完成 item 有回答的情况。
  该上限不代表专用执行器内部缓冲的全链路峰值上限。
- 添加账号默认仍是 ChatGPT OAuth 登录，新增“现有 Access Token”导入方式，要求 Secret Access、
  非空 token 和有效 Account ID。切换 Provider、切回 OAuth、关闭弹窗或授权失效时按生命周期清理材料。
  token 使用既有独立 secret-edit 通道，不进入普通前端配置文档。
- 首次真实 UI 导入发现账号虽保存成功，但缺少 `credential_identity_category_id`，因此不进入订阅子池。
  已补订阅分组选择及持久化，并在新的空隔离数据目录重新完成 UI 创建、导入与调用验收。
  最终账号沿用原配置的 `pro20x` 分组，不根据 token 类型猜测订阅等级。
- 补齐用户声明模型的追加式注册：合并 provider 支持模型、仅补缺省默认模型和精确模型路由。
  保留已有路由优先级、禁用状态与其他 provider 顺序；重复注册幂等，不冒充在线模型发现。
  编辑账号不覆盖原 OAuth headers、refresh token 或 refresh endpoint。

## 真实 UI 与请求证据

通过 UI 完成：添加服务商 → 标准模板 `chatgpt` → 添加 → 添加账号 → 现有 Access Token →
选择 `pro20x` 并导入 → 手动测试。没有通过直接写数据库或管理 API 代替 UI 创建/导入。

最终配置仅含一个 provider 和一个 credential：

```json
{
  "providerId": "chatgpt",
  "credentialId": "chatgpt-native-acceptance",
  "model": "gpt-5.6-luna",
  "call_flow": {
    "call_adapter": "auto",
    "fingerprint_adapter": "codex_cli",
    "special_adapter": "auto"
  }
}
```

自动测试关闭，只启用手动连通用例；本轮共三次真实模型请求。

| 验收项 | 真实结果 | 标识 |
| --- | --- | --- |
| UI 手动测试 | `passed`，1/1，回答 `OK`，1499 ms | `probe-ba72a745-3194-44c6-a1e1-041344ad8877` |
| 普通非流式 `/v1/responses` | HTTP 200，`completed`，回答 `OK` | `0246e437-f09f-4072-8d50-6e43a212d598` |
| 普通 SSE `/v1/responses` | HTTP 200，`response.completed`，回答 `OK` | `ae32865d-897a-45aa-82fd-7e40a78d4da7` |

两次普通请求审计为 `gw-audit-85e67e56-d28f-485f-a7db-e89d10a828aa`、
`gw-audit-0697a6da-e82a-441b-a3d3-b0d86999a274`；均使用 `chatgpt-native-acceptance`，
`attemptedProviderIds=[chatgpt]`、`routeAttemptCount=1`、`direct_http`，无其他账号或服务商 fallback。
两次普通调用各报告 input tokens 18、output tokens 5，不包含 UI probe 的总用量。

保存后 revision 为 `r2-67aa6db50665`。原 ChatGPT provider 内容与原 routes SHA-256 均保持不变，
导入的 Access Token 和 Account ID 与备份一致，隔离凭据未含 refresh token。

## 开发侧验证与构建

- 前后端均先红后绿：probe 合同捕获旧路径未应用 `extra_body.probe_marker`；UI 合同捕获缺失的分组/模型注册。
- 后端定向测试 9 项通过：`basispoints_channel_contract` 4、`codex_native_fidelity_contract` 4、
  `console_codex_model_probe_contract` 1。合同覆盖精确认证、公共 planner、指纹、401、截断流、完成回答、脱敏和审计。
- 最终 6 个前端定向文件共 54 项通过，TypeScript typecheck 通过。
- Rust formatter、31 项 effective-line checker 测试、ratchet 与 `git diff --check` 通过。
  新增或修改源码均未超过 500 有效行；未增加超限文件或申请例外。
- 独立只读复核确认 token 分离/清理、订阅分组和追加式模型注册的保护逻辑；运行结论以本轮测试为准。
- 使用 `tools/build-gateway-devtest.ps1 -Edition api -Jobs 4` 构建本轮 API devtest EXE，
  默认端口 4300，来源凭据 `buildKind=devtest`，不属于正式发布包或桌面分发包。
- 验收 EXE SHA-256：`eedb26cbc8401b03b6dededf654021c980face0cbf74d01600297f7f2b083834`。
  后续分组/模型注册与蓝色标记属于前端源码更新；API-only 构建不冒充最新完整桌面包。

最终有效行：probe 281（原 255）、probe 合同 117（原 104）、CredentialDialog 361（原 319）、
凭据编辑 hook 296（原 288）、credential draft 131（原 129）、新模型注册模块 17、新模板测试 83。

公开脱敏证据目录：
`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-chatgpt-adaptation-20261025/`。
包含三次调用摘要、精确账号审计、原数据完整性检查、红绿测试日志和 UI 截图。
`ui-probe-passed.png` 展示手动测试通过；`ui-template-verified.png` 展示最终蓝色模板与固定调用流程。

## 未覆盖的范围

本轮没有重新完成浏览器 OAuth 登录或 refresh 生命周期验收，没有测试另外两个既有账号，
不能据配置中旧 token 的过期时间断言其 runtime 当前凭据不可用。没有对 Basispoints 公网通道、
全部模型、长上下文、长时间稳定性或 ChatGPT Web 模板作真实验收。
Basispoints 本地合同通过不等于公网业务验收通过。没有运行全量 Cargo 回归、正式发布审计，
没有生成新的桌面 EXE 分发包，也没有提交或推送代码。

## 收尾复核

重启本轮源码 UI 后，真实页面已显示蓝色 `chatgpt` 及“新调用流程已验收”辅助标记；没有重复
添加服务商或再次触发模型请求。源码与公开文本证据的秘密匹配检查通过，新增/修改文本为 UTF-8 无 BOM。
最终再次核验原 provider/routes 不变、验收 EXE 哈希匹配，并按可执行文件和脚本身份关闭本轮进程。
2026-10-25T06:55:12Z 收尾记录确认 4300/1435 均无监听，原 4200 `/healthz` 返回 HTTP 200。
隔离数据、首次失败现场、凭据私有备份和公开证据全部保留，没有删除用户数据。

# ChatGPT Web 完整会话材料导入与持久化梳理

## 本轮结论与范围

本轮是现有实现审查与流程整理，不代表已交付新的导入 UI 或自动持久化实现。未修改 4200、未导入生产账号、未进行新的上游调用。

当前控制台 ChatGPT OAuth 是 Codex OAuth，不会自动得到完整 ChatGPT Web 会话。`src/console/chatgpt_oauth/persistence.rs` 的 `validate_provider` 与 `save` 明确限定 `chatgpt-codex-oauth-official-api` preset；不能只改这个限制就认为 Web 接入完成。

`src/console/chatgpt_oauth/material.rs` 的授权码交换使用 Codex client ID，保存 access token、refresh token、到期时间、账号 ID 与邮箱；ID token 用于解析身份，没有作为完整原始材料保存。此流程不采集 ChatGPT 网页 Cookie、浏览器存储或网页设备环境。浏览器完成登录，并不意味着 localhost OAuth 回调会收到该浏览器的 Cookie。

## 已有能力与缺口

| 环节 | 当前事实 | 不能据此宣称 |
| --- | --- | --- |
| Codex OAuth 导入 | token 交换后通过配置事务保存到 Codex provider | 自动创建 Web provider 或获得网页登录 Cookie |
| Web worker 准备 | 可返回 token、Cookie、UA、device/session、客户端版本等材料 | 必然获得新的 token 或可无限续期 |
| 正常管理接口导入 | 前轮已用 secret grant/secret patch 导入 token、Cookie 及白名单元数据，并完成纯 HTTP 双模式调用 | 已有面向用户的一键完整材料导入 UI |
| Web 刷新写回 | `src/keepalive/chatgpt_web/runtime.rs` 合并运行时字段，写 Redis 和可选 PostgreSQL | 本地 EXE 已有同等的持久化闭环 |
| 重启复用 | 前轮验证了全新数据目录导入后调用 | 自动刷新后重启恢复已验收 |

本地存储规则明确关闭服务端 Redis pool。Web 刷新写回函数忽略 Redis 写回结果，且此函数没有本地配置事务分支；因此不能将它登记为本地持久化完成。运行时对象或浏览器 profile 也不能替代正式凭据保存验收。

文件夹导入的 Web normalizer 从头重建 payload；保存 `rawSource` 不代表其中的 Cookie、headers 或 extraBody 已变成有效运行时字段。应统一材料映射，不能将文件夹导入与控制台导入视为已经等价。

## 建议的统一导入流程（待实现部分）

1. 用户在 `chatgpt-web` 下选择“导入会话材料”或“从已登录浏览器准备会话”。OAuth 保持独立入口，不静默复制 Codex 凭据。
2. 浏览器准备只访问用户指定账号；首次登录或需要人工验证时明确停在需要用户操作的状态，不无界重试。
3. 将准备结果转换成白名单材料：access token、真实 token 到期时间、Cookie、UA、device/session ID、客户端版本、语言与时区，以及实际执行链需要的 bootstrap 字段。不得整体持久化 worker 调试输出、body preview 或全部浏览器存储。
4. Cookie、token、refresh token 走既有秘密保存机制；普通配置和 UI 回显不得包含明文。Refresh token 必须同时保留其来源、client ID 与受信任 endpoint，不能套用另一个 OAuth client。
5. 元数据统一规范化。JWT `exp` 与网页 session `expires` 分开记录；不可用较晚的网页时间延长 access token。现有 RFC3339 `+00:00` 解析兼容性问题需单独修复并测试。
6. 本地模式通过既有配置事务 owner 保存，保留 provider/credential ID 与其他配置；带 revision 冲突检查，不能在请求线程直接覆盖 routes 文件。服务端使用其已有存储 owner。
7. 完成状态区分“已导入”“材料已准备”“调用已验证”“需要重新登录”。只有确认新 token 或实际有效期推进，才能标记续期成功。

## 必须补齐的验收

- 使用合成材料验证字段白名单、长度限制、敏感字段隔离、无效日期与不匹配身份拒绝。
- 验证正常管理 API 导入、读回、独立进程重启后字段与秘密保留；普通配置响应不得回显秘密。
- 验证浏览器刷新材料后的本地事务写回，配置并发冲突、失败可见和重试不会覆盖其他账号。
- 在独立 4300 中，以一个精确账号和 `gpt-5.4` 验证重启后的 buffered/SSE；严格禁用浏览器，检查实际文本与正常结束，不批量调用模型。
- 自动续期另列测试；当前样本无 refresh token，前轮 worker 前后 token 与 JWT exp 相同，不能标记为续期通过。

前轮运行证据见 `2026-10-28-chatgpt-web-session-modes.md`。本轮不重复消耗真实账号，只整理已核实边界和下一步实现合同。

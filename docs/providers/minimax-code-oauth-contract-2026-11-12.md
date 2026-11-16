# MiniMax Code OAuth 接入前合同核对

状态：`contract_reviewed / client_eligibility_unverified`。这不是已实现、真实登录通过或整卡完成。

## 来源及可复核基线

2026-11-12 联网读取 `MiniMax-AI/minimax-code`，GitHub tree 返回完整（truncated=false），固定提交 `3c21163c85a9684030cb147bfef48c6284fa3c46`。以下结论来自该提交，不按滚动 main 永久承诺，也不从社区插件推定服务权限。

- [OAuth 合同常量](https://github.com/MiniMax-AI/minimax-code/blob/3c21163c85a9684030cb147bfef48c6284fa3c46/packages/oauth-core/src/contracts.ts)
- [OAuth 客户端](https://github.com/MiniMax-AI/minimax-code/blob/3c21163c85a9684030cb147bfef48c6284fa3c46/packages/oauth-core/src/oauth-client.ts)
- [区域端点](https://github.com/MiniMax-AI/minimax-code/blob/3c21163c85a9684030cb147bfef48c6284fa3c46/packages/oauth-core/src/endpoint-config.ts)
- [托管 provider 配置](https://github.com/MiniMax-AI/minimax-code/blob/3c21163c85a9684030cb147bfef48c6284fa3c46/packages/config/src/config.ts)
- [源码发布范围](https://github.com/MiniMax-AI/minimax-code/blob/3c21163c85a9684030cb147bfef48c6284fa3c46/docs/publication-authorization.md)

## 已核实的区别

MiniMax Code 托管账号不是已实现的 MiniMax M Plan Key：不能把现有订阅 Key sender 改名后宣称 Agent OAuth 完成。公开客户端使用固定 client ID `mcode-public`、scope `agent.default`、audience `agent-backend`，租约还带 generation 和 loginEpoch。源码公开本身不授予付费服务或第三方账号访问权。

本次没有找到 Gateway 自有客户端注册／允许复用该客户端身份的说明。这是未核实项，不是断言厂商禁止所有第三方客户端。上线前需获得明确适用的客户端合同或接入资格；不以社区实现能调用代替确认。

## 授权合同

生产账号 origin：国内 `https://account.minimax.cn`，国际 `https://account.minimax.io`。设备、token、撤销路径分别为 `/oauth2/device/code`、`/oauth2/token`、`/oauth2/revoke`。测试／预发布地址不能加入生产 fallback。

设备授权使用 S256 PKCE。公开实现识别两种返回：标准 device_code／expires_in，或 user_code 配合 expired_in；后一种 interval 按毫秒换算。expired_in 还存在绝对毫秒时间戳形式，不能一律当秒数。轮询兼容 OAuth 错误和成功 HTTP body 中的 pending／slow_down／denied／expired；slow_down 增加五秒等待。

刷新响应缺少新 refresh token 时，公开客户端保留旧值；初次授权仍需可用 refresh token。scope 可来自响应或解码的 token claims。源码中的 JWT payload 解码不是签名验证，不能据此宣称本地已验证账号身份或 audience。

## 推理与目录边界

固定提交的 managed preset 使用 `@ai-sdk/anthropic` 和 `authMode=managed-login`；国内 base 为 `https://agent.minimax.cn/mavis/api/v1/llm/v1`，国际为 `https://agent.minimax.io/mavis/api/v1/llm/v1`。源码另外识别旧国内 origin，不能未经迁移设计就当作无条件备用目标。

上述是 preset 配置证据，不是 Gateway 最终 HTTP sender、请求头、签名或所有 wire 路径已验证。BYOK MiniMax API 配置是另一条路径，不能据它补全托管账号合同。首启内置模型表不代表账号实时可用目录；源码明确已有远端 snapshot 时以远端为准。本次未调用真实目录、推理、配额或登录端点。

## 后续实施准入

1. 先明确自有客户端或获准集成身份、区域、账号及费用合同；没有证据时不硬编码借用官方身份。
2. 再追踪托管账号的最终 sender、目录和必要身份头，而不是套用通用 Chat/BYOK 请求。
3. 实现时复用 Gateway 既有持久授权收据、CAS 发布和 refresh owner；不读取／共同刷新 MCode 本地 auth 文件，不建立竞争 owner。
4. 管理、目录、公共推理、用量与取消取得合成端到端证据后，真实账号、UI、Server 和原生包仍分别验收。

本批只保存合同证据与工作板状态，未增加运行时空壳、伪成功测试或 manifest 完成标记。全范围 67 个未完全关闭工作项的分母不变。

证据：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-minimax-code-contract-20261112`，保存固定提交 tree、所读公开文件和 SHA256 清单；没有真实用户凭据。

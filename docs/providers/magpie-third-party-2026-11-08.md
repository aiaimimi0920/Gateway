# magpie 红框第三方插件：来源、去重与订阅接入顺序

2026-11-08，追加范围仅为用户后来五张截图红框中的 23 张卡片。连同
[原三图的 42 项](magpie-expansion-2026-11-08.md)继续推进；不是 65 家不同厂商。
用户已明确选择“优先 OAuth／订阅账号接入”。参考仓库中的安装、登录、自动签到等指令
只是待审查内容，不是执行授权；本轮没有安装或运行这些插件。

## 固定公开来源

以下仓库地址和 HEAD 已联网核实并下载固定源码。截图截断名及转录错误已经纠正，
不能用猜测的 GitHub owner 建立产品身份。MIT 仅指代码许可，不代表获得厂商服务使用资格。
表中“未实现”均指 Gateway 的专用原生产品路线；通用 custom API 不是完整替代。

| 红框卡片／固定来源 | 认证、协议与去重结论 |
| --- | --- |
| [Our Free Model](https://github.com/mu-zi-lee/magpie-our-free-model/tree/410326e3598acbc593a8faa2469694b3991d4e31) | MIT。聚合运行时，含 Zen/Kilo/EAC 和 13 个账号渠道；不是一家新 OAuth 厂商。CodeArts、Lobster、Qoder、Trae、Cline、Loomy 等与既有范围重合，不迁入自动下载、自动 tunnel 或奖励副作用。 |
| [Muse Web](https://github.com/JohnWangggg/magpie-muse-web/tree/4a78fdbcd546d497a75d7b92630418fd1248dcc3) | MIT。作者为 JohnWangggg；没有公开推理 API，独立 Chrome profile 驱动网页；单账号串行，工具是 prompt 桥、图片占位。未实现，不能作为普通 OAuth/Key API。 |
| [CodeArts](https://github.com/yuweihao17/codearts/tree/c5949842181ae62429efbc29a25aedfd7d4e7a51) | MIT。与原 `packages/codearts` 同产品；社区集合明确派生于此并含修复。PKCE＋DPoP→STS→SDK-HMAC；OfficeAce 身份另分，不倒退复制旧快照。 |
| [Magicrew](https://github.com/MYCF711/magpie-magicrew-auth/tree/4c9fca98f6e8bec91c63559e9aaec36252cb6af7) | MIT。2026-11-04 停维护属实；国内 WAF、国际 ESA405 有已知阻断。浏览器 JWT、短期 `mgw_` key、组织与滚动 refresh；与 Providers Hub 重合。列入范围但不优先，也不自动签到/注册/领奖。 |
| [DIM](https://github.com/arcships/magpie-dim-oauth/tree/a7ba461071fb100985cd9ae36edd15be47965ee9) | MIT。真实 owner 为 arcships。国内/海外 OIDC discovery、PKCE、userinfo、refresh_token、账号目录；Chat/Responses/Messages/Gemini 由目录逐模型声明。未实现，新增红框订阅优先候选。 |
| [DeepSeek Web](https://github.com/mu-zi-lee/magpie-deepseekweb2api/tree/64684f53d46632873cf24f93361810d00da392be) | MIT。手动 userToken/可选 Cookie→私有 Web session＋PoW＋SSE。不同于现有 DeepSeek 官方 API；无已核实的第三方 refresh grant，不称 OAuth。 |
| [OpenCSG](https://github.com/CheinTian/magpie-opencsg-auth/tree/74ac4c2f928e31f15109c6dd9512c81618cb82f4) | MIT。浏览器 IAM 登录换 AI Gateway key，JWT 不落盘，最终不是可续期 OAuth session。Chat 和 live 目录；未实现，需独立审查其非标准 callback/state 合同。 |
| [Notion](https://github.com/resucdueff/magpie-notion/tree/2ba7442fb9f4dcbc2c8029975e438348b274477c) | MIT。手动 `token_v2`、私有 AI API、Chrome TLS 指纹；工作区和模型发现；纯文本，无 agent tools，过期重导。未实现，不等于公开 Notion Integration API。 |
| [TeleAgent](https://github.com/cuddlesgiggles/teleagent-auth/tree/3e7062a473fccedde9f6211e30d730425db8c429) | MIT。桌面会话或手动 X-Token；全局 relay 身份、两层 HMAC 和客户端版本；无第三方续期，需原客户端重登。未实现，不自动解密桌面登录态。 |
| [ModelScope msmoli](https://github.com/MisonL/magpie-plugin-msmoli/tree/69132df3e3368f0ea3c7012c4bbb40972003a84e) | MIT。Cookie 读取签到、奖励、余额；源码明确不提供 chat models。单列辅助能力，不计为新增模型推理服务商，不顺便执行有副作用的签到。 |
| [Marvis](https://github.com/MYCF711/magpie-marvis/tree/4839bb772840e856f73177f8a73792151ee98dbc) | MIT。QQ/微信浏览器账号→access/refresh；业务接口 MD5 签名、推理 Chat。实际 refresh 为 `/marvis_client_login/marvis_refresh_token`，不是过时概要里的 GET；静态模型表。未实现。 |
| [QwenWork device 插件](https://github.com/harwinvoid/magpie-plugin-qwenwork/tree/fdf9894729a856a825d191cbd5128c02a91cc0a0) | MIT，vendor 另保留 community MIT。设备 PKCE→device token/refresh；COSY AES/RSA/签名与 agent_chat_generation，不是裸 Chat API。原固定 `packages/qoder` 已含 QwenWork 和 enterprise 产品参考；Gateway 未实现。 |
| [Loomy](https://github.com/MYCF711/magpie-loomy-auth/tree/2c24ea4a9c8ee39fa2cd9013a76d9dda45f97ced) | MIT。与原 `packages/loomy` 同产品；短信/HMAC 或 session、双头鉴权，14 天无 refresh。模型/积分目录另读，不创建伪 refresh token。 |
| [AtomCode](https://github.com/aqiu817/magpie-atomcode/tree/9db706404f4164c2a299c1d8591e2699212054db) | MIT。真实 owner 为 aqiu817。AtomGit broker OAuth、token refresh、推理签名；与原 `packages/atomcode` 同产品，后者还含 CLI/旋转结果缓存等合同。未实现。 |
| [SenseNova](https://github.com/aqiu817/magpie-sensenova/tree/6a1de41384ba4e9d1b7285101025738eed32bfcc) | MIT。与原 `packages/sensenova` 同 Token Plan 产品；推理仍是 API key，控制台 OAuth/cookie 另作额度与续期，不是小浣熊。未实现，不把控制台 token 当推理 key。 |
| [EdgeOne](https://github.com/Sliverkiss/magpie-edgeone/tree/9b985f14caa96c306c04a5e73468cef62a753629) | MIT。匿名 DeepSeek Harness session/agent RPC＋events.mux；不是腾讯 EdgeOne Key API。需要远程会话、取消和资源 owner；未实现。 |
| [AMD Radeon Cloud](https://github.com/aqiu817/magpie-amd-cloud/tree/feb76d9136a74e01b418b7f61f2668b623b6c922) | MIT。审核账号签发 Bearer key，无 refresh；原生 Chat、`models`、`usage`；不同于 Responses。未实现，排在订阅优先批次之后。 |
| [LobsterAI](https://github.com/Jasery/opencode-lobster-auth/tree/f7661daee1a3017aaf61a58c1b53d418fbd4ebc9) | MIT。独立浏览器 exchange/refresh 与桌面借用必须分开；源码会刷新浏览器账号，README“从不刷新”不准确。上游即使 stream:false 也返回 SSE。与 Providers Hub 同产品，未实现。 |
| [DuMate](https://github.com/R227BOUT/magpie-dumate/tree/44aff39429d89867cc005ab2dc78ff1de15810e2) | MIT。百度 Cookie→临时 gateway token；按服务端 expiry 刷新，Cookie 本身无续期合同；静态智能路由别名，无 `/models`。未实现，不能称普通 API key。 |
| [Cloudflare AI Gateway](https://github.com/overstarry/magpie-cloudflare-gateway/tree/b24038f794efe5b9622bcb0cb0c133006f8bfde4) | 未找到 LICENSE，package 未声明 license。Account ID＋Gateway ID＋Run/Read Token＋可选 BYOK alias；Chat/Responses/Messages 原生 provider 路径。**不是本轮 Workers AI**，未实现；可根据官方文档独立重写，不复制未获许可代码。 |
| [HF SSH Chat](https://github.com/mu-zi-lee/magpie-ssh-chat-hf-co/tree/ea35c73bb0f1be7c16aacb706ddfed2076ff4bf8) | MIT，含其他 NOTICE。专属 SSH key＋chat.hf.co:22＋PTY/screen parser；TOFU host key，单模型、buffered 工具桥，非 HTTP HF Router。未实现，不自动下载 cloudflared 或暴露临时公网。 |
| [QwenWork 第二实现](https://github.com/tianyn1990/qwenwork-magpie/tree/8edf121209eae125c93523ede4d87c9801a1505f) | MIT。同一 QwenWork 产品/额度；独立 device 登录，但签名依赖用户已装官方 WASM，未分发该二进制。两个固定档位不是 live 目录。与上一实现比较取舍，不重复建两个服务商。 |
| [OpenCode 2 Free](https://github.com/ferocknew/magpie-opencode-2-free-plug-in/tree/e3417609fd8f0fa2849acaa2a1d009bb64fb4d56) | package 声明 MIT，未找到 LICENSE。与原 Zen Free 同免费层；客户端资格、地域及数据条款仍适用。参考有伪造 UA/工具/session、最多八次 5xx POST 重放和截断兼容；不照搬这些行为冒称通用免费 API。 |

## 订阅优先的实施顺序与边界

1. 先完成已有订阅实现的实用入口：本轮补 Grok Build 的原生 device grant，复用现有
   refresh generation、SecretPatch 保存、模型调用与 SSE owner；不启动 magpie/官方 CLI，
   不读取已有客户端材料。原 code/manual callback 保持兼容。客户端 ID/scopes 仍由用户明确填写，
   不借用参考代码的身份。
2. 红框新增订阅以 DIM 为首个候选：独立国内/海外 issuer、授权会话、nonce/state/PKCE、
   凭据受众绑定、轮转刷新持久化、逐模型协议目录和精确账号调用必须一起完成。
   社区 client ID 是否可供 Gateway 使用、redirect 登记及真实账号验收仍未证明，不能直接点亮。
3. AtomCode、Copilot、Nous、MiniMax Code、Lobster 等按各自授权/刷新合同逐批实现。
   单次 refresh token 要串行和持久化，不与用户桌面程序争抢；网页登录换 key 的 OpenCSG
   与持续 OAuth token 路线分别处理。
4. QwenWork、CodeArts、Qoder 等签名/RPC 产品单独实现 codec、目录和取消 owner。
   不用普通 Qwen workspace API、Gemini 或 generic Key 模板冒充，不为模型名相似混合账号。

以上是范围与排序，不是这些产品的实现完成回执。没有新增整卡蓝标或真实上游成功声明。

## 已核实的官方公开约束

- [Cloudflare BYOK](https://developers.cloudflare.com/ai-gateway/configuration/bring-your-own-keys/)：
  供应商 key 留在 Cloudflare；移除 provider auth，使用 Gateway 身份和可选 alias。
  [Unified Billing](https://developers.cloudflare.com/ai-gateway/features/unified-billing/) 的
  `cf-aig-no-wholesale:true` 用于禁止第三方 billing fallback，不代表 Workers AI 免费。
- [OpenCode 当前模型政策](https://opencode.ai/docs/console/models/)明确
  “Free-tier chat models require OpenCode”；部分模型有地域及数据保留/训练限制。
  公共 `public` token 不代表独立 Gateway 获得相同资格。
- [AMD Radeon Cloud 官方文档](https://amd-aim.github.io/radeon-cloud-docs/)于 2026-11-12
  补充核实：当前还声明 Messages；Token Factory 禁止生产、第三方转供和账号池。
  上表为旧固定插件记录，不代表当前全部官方能力或 Gateway 接入资格。
  pts 不是现金；重置窗口与旧插件描述存在差异。见 [资格与合同](amd-radeon-qualification-2026-11-12.md)。
  没有账号权限、生成或费用验收。

## 证据与非目标

固定源码、HEAD 索引、公开文档和本轮测试日志放在
`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-magpie-expansion-20261108`。
认证/协议事实来自固定源码；其中作者的“实测通过”、余额、模型数量和额度文案不是 Gateway 本轮实测。
没有使用真实 `.ng`、浏览器/桌面 profile、真实凭据或付费生成；未出包、提交、推送或切换在线实例。

# Magpie 剩余范围盘点（2026-11-11）

依据当前实施看板逐行计数：**67 个工作项尚未完整关闭**。这是工作项数量，不是厂商数量、manifest 数量、接口数量或工时估计。

原 42 项加红框 23 张卡，共 65 张来源卡；排除 Providers Hub、Our Free Model 两个聚合外壳，合并 7 张重复卡，得到 **56 个主工作项**。聚合包再补充 **11 个额外工作项**，合计 67。

当前主工作项中，12 项已有不同范围的本地集成切片，完整产品验收仍开放；其中 LobsterAI 已完成授权、本地持久刷新、管理/UI/目录、普通目录模型直选 Chat／Messages 与虚拟模型双协议公共推理的本地验证，真实账号、跨请求持久会话绑定、特殊模型与 Server 等仍未完成。其余 44 个主工作项待原生实现或资格核实，额外 11 项分别待实现或差异核对。完整关闭数仍为 0，不将合成合同验证等同真实账户、包或部署验收。

计数规则：国内/国际站、同一卡已有的产品分支放在原工作项内逐站跟踪。例如 Qoder 项包含看板已明确的 QwenWork/enterprise 分支，CodeArts / OfficeAce 保持原卡分组；不表示一条实现就能覆盖各分支。ModelScope msmoli 是辅助能力，仍占一个范围工作项，但不计作推理厂商。

MiniMax M Plan 订阅 Key 切片已有证据，但原卡要求的 MiniMax Code Agent OAuth 仍待实现，不能以相邻产品抵扣。54 条 metadata-only manifest 也不能代替完成数。

## 主工作项（56）

| 编号 | 工作项 | 当前层级 |
| --- | --- | --- |
| 1 | Agnes AI | 已有本地集成切片，完整验收未完成 |
| 2 | AtomCode | 待原生实现或资格核实 |
| 3 | GitHub Copilot | 待原生实现或资格核实 |
| 4 | JoyCode | 待原生实现或资格核实 |
| 5 | Kimi Work | 待原生实现或资格核实 |
| 6 | OpenCode Zen Free | 待原生实现或资格核实 |
| 7 | ZCode | 待原生实现或资格核实 |
| 8 | Qoder | 待原生实现或资格核实 |
| 9 | Devin | 待原生实现或资格核实 |
| 10 | Zed | 待原生实现或资格核实 |
| 11 | Factory | 待原生实现或资格核实 |
| 12 | Grok Build | 已有本地集成切片，完整验收未完成 |
| 13 | Command Code | 已有本地集成切片，完整验收未完成 |
| 14 | MiMo | 待原生实现或资格核实 |
| 15 | Cursor | 待原生实现或资格核实 |
| 16 | Kiro | 已有本地集成切片，完整验收未完成 |
| 17 | MiniMax Code | 待原生实现或资格核实 |
| 18 | Warp | 待原生实现或资格核实 |
| 19 | WorkBuddy | 待原生实现或资格核实 |
| 20 | Cline | 待原生实现或资格核实 |
| 21 | Trae | 待原生实现或资格核实 |
| 22 | Open Design | 待原生实现或资格核实 |
| 23 | CatPaw | 待原生实现或资格核实 |
| 24 | Loomy | 待原生实现或资格核实 |
| 25 | Nous Portal OAuth | 已有本地集成切片，完整验收未完成 |
| 26 | 商汤小浣熊 | 待原生实现或资格核实 |
| 27 | SenseNova Token Plan | 已有本地集成切片，完整验收未完成 |
| 28 | InkStone | 待原生实现或资格核实 |
| 29 | AutoClaw | 待原生实现或资格核实 |
| 30 | CodeArts / OfficeAce | 待原生实现或资格核实 |
| 31 | GCP AI | 已有本地集成切片，完整验收未完成 |
| 32 | Cloudflare Workers AI | 已有本地集成切片，完整验收未完成 |
| 33 | River | 已有本地集成切片，完整验收未完成 |
| 34 | SAP AI Core | 已有本地集成切片，完整验收未完成 |
| 35 | OrcaTerm | 待原生实现或资格核实 |
| 36 | AStudio 讯飞 | 待原生实现或资格核实 |
| 37 | Agnes Code | 待原生实现或资格核实 |
| 38 | Tabbit | 待原生实现或资格核实 |
| 39 | Codar | 待原生实现或资格核实 |
| 40 | MClaw | 待原生实现或资格核实 |
| 41 | Antigravity CLI | 待原生实现或资格核实 |
| 42 | Muse Web | 待原生实现或资格核实 |
| 43 | Magicrew | 待原生实现或资格核实 |
| 44 | DIM | 已有本地集成切片，完整验收未完成 |
| 45 | DeepSeek Web | 待原生实现或资格核实 |
| 46 | OpenCSG | 待原生实现或资格核实 |
| 47 | Notion | 待原生实现或资格核实 |
| 48 | TeleAgent | 待原生实现或资格核实 |
| 49 | ModelScope msmoli | 待原生实现或资格核实 |
| 50 | Marvis | 待原生实现或资格核实 |
| 51 | EdgeOne | 待原生实现或资格核实 |
| 52 | AMD Radeon Cloud | 公开资格已核实：Token Factory 禁止生产、第三方转供及账号池；通用共享接入受阻，个人开发限定范围另定；原生实现与验收仍未完成，见 [资格记录](amd-radeon-qualification-2026-11-12.md) |
| 53 | LobsterAI | 授权、本地持久刷新、管理/UI/目录、普通目录模型直选 Chat／Messages 与虚拟模型双协议公共推理切片已验证；真实账号、跨请求持久会话绑定、特殊模型、Server 与原生包等验收未完成 |
| 54 | DuMate | 待原生实现或资格核实 |
| 55 | Cloudflare AI Gateway | 待原生实现或资格核实 |
| 56 | HF SSH Chat | 待原生实现或资格核实 |

## 聚合包额外工作项（11）

| 编号 | 工作项 | 待办 |
| --- | --- | --- |
| 57 | Accio Work | 核对 Hub 产品与现有 Accio 的身份和协议差异 |
| 58 | MonkeyCode | 国内、国际两站均待实现，作为一个产品项跟踪 |
| 59 | 七牛云 MaaS / Modelink | 待原生实现，产品分支分别核对 |
| 60 | 百智云 | Hub 有源码但不随固定 npm 版本发布，待合同核对与实现 |
| 61 | 灵犀 | 固定快照缺可用实现文件，待取得协议合同 |
| 62 | 豆包工作 | 固定快照缺可用实现文件，待取得协议合同 |
| 63 | Dola | 固定快照缺可用实现文件，待取得协议合同 |
| 64 | CodeBuddy IDE | 待原生实现 |
| 65 | Kilo 免费池 | 待原生实现与使用资格核对 |
| 66 | EAC 授权聚合网关 | 待原生实现，不将底层模型品牌重复计数 |
| 67 | Gemini Code Assist / Antigravity | 核对现有 Cloud Code 路线；不当成 Gemini Web 或本次新增完成项 |

## 去重映射

| 红框卡 | 归入主工作项 |
| --- | --- |
| CodeArts | CodeArts / OfficeAce |
| Loomy | Loomy |
| AtomCode | AtomCode |
| SenseNova | SenseNova Token Plan |
| QwenWork device | Qoder |
| QwenWork 第二实现 | Qoder |
| OpenCode 2 Free | OpenCode Zen Free |

Providers Hub 与 Our Free Model 的其他子渠道已在主工作项中覆盖来源，例如 AStudio、LobsterAI、Magicrew、AutoClaw、Loomy、WorkBuddy、Qoder、Trae、Cline、小浣熊、MiniMax Code、ZCode 与 CodeArts；重复出现不再新增工作项。

细项状态以 [持续实施看板](magpie-workboard-2026-11-08.md) 和每批实际验证回执为准。此盘点保留分母，不因新建模板或新增源码模块自行减项。

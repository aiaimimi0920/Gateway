# Luma 长期证据归档

2026-11-19 从现存临时目录逐文件复制 101 份证据，共 3,166,074 bytes；复制后全部 SHA-256 与原文件相同，见 [archive-index.json](archive-index.json)。没有删除原始目录，没有把临时数据库、浏览器 profile、账号授权或整个构建目录带入仓库。

| 目录 | 内容 |
| --- | --- |
| [gateway-luma-web-session-20261121](gateway-luma-web-session-20261121/verification.json) | Web 会话维护、轮换写回、真实 Edge 回环、Redis CAS、新桌面 devtest 及隔离启动；真实账号续期未验 |
| [gateway-luma-gaps-delivery-20261121](gateway-luma-gaps-delivery-20261121/verification.json) | 五项明确缺口的 desktop devtest 包、6154 项载荷、源码与 EXE 哈希、隔离启动；另有原 Edge 会话 Cookie 轮换的脱敏观察，无新生成 |
| [gateway-luma-explicit-gaps-20261121](gateway-luma-explicit-gaps-20261121/verification.json) | 四种 MCP 交互显式答复及 Web Keep 归属保护的红绿定向验证和 12 个源码哈希 |
| [gateway-luma-mcp-creation-browser-20261121](gateway-luma-mcp-creation-browser-20261121/verification.json) | 正常 main 创作至管理 TCP、SQLite 与合成 MCP，单次提交、原任务链接与会话 credits；不含实际供应商调用 |
| [gateway-luma-mcp-server-20261121](gateway-luma-mcp-server-20261121/verification.json) | Server MCP 持久任务的源码哈希、PG SQL 验证与路由测试结果；不等于供应商、TCP 或 OS 强杀验收 |
| [gateway-luma-hosted-20261121](gateway-luma-hosted-20261121/verification.json) | PostgreSQL 托管原凭据强杀后补账、换 Key 阻止恢复与路由文档回归；仅合成，含红绿检查和源码哈希 |
| [gateway-luma-four-20261121](gateway-luma-four-20261121/verification.json) | 同 run 会话审阅、Web 音视频链接、PostgreSQL CAS 与合成 Server 强杀原任务追账；定向结果与范围源码哈希，无真实收费或新包 |
| [gateway-luma-mcp-review-20261120](gateway-luma-mcp-review-20261120/verification.json) | 显式 review 答复、持久单次提交与会话游标的定向验证，组件宽窄屏截图、源码 hash；无真实账号、无收费、无新包 |
| [gateway-luma-mcp-recovery-20261120](gateway-luma-mcp-recovery-20261120/verification.json) | 配置变化与已发布 OAuth 刷新后的 Local 原任务恢复，正常管理 API／持久化合成验证、前端红绿结果、源码 hash；无真实刷新、无收费、无新包 |
| [gateway-luma-mcp-models-20261120](gateway-luma-mcp-models-20261120/verification.json) | 一次已授权显式刷新、18 项模型及全部详情合同，18 模型创作的定向验证、实际组件合成浏览器与源码 hash；无收费、无新包 |
| [gateway-luma-mcp-workspace-20261120](gateway-luma-mcp-workspace-20261120/verification.json) | 已保存素材 ID／原工作区复用、连续任务与重启绑定的定向验证，前端与合成浏览器回执、源码 hash；未收费、未重新打包 |
| [gateway-luma-mcp-options-20261120](gateway-luma-mcp-options-20261120/verification.json) | MCP 模型参数、参考图、图片编辑及视频重绘／扩画的正常管理路由合成验证，前端红绿记录、源码 hash 与本批交付结果；无真实收费 |
| [gateway-luma-mcp-creation-20261120](gateway-luma-mcp-creation-20261120/verification.json) | Local 显式图片／视频创作、持久任务和取消／费用的管理 API 合成回归，前端与构建回执、原始失败和源码 hash；未收费、未重新打包 |
| [gateway-luma-mcp-live-contract-20261120](gateway-luma-mcp-live-contract-20261120/verification.json) | 新保存账号的当前 26 项完整合同、18 项模型与三项 describe；正常管理 API 真实只读成功，配置未变；排除 token 与连接身份，无生成 |
| [gateway-luma-mcp-snapshot-20261120](gateway-luma-mcp-snapshot-20261120/verification.json) | 目录 annotations／JSON 下载、官方插件固定 commit 原文、正常网页下载／宽窄屏与定向红绿回执；无真实账号 |
| [gateway-luma-task-browser-20261120](gateway-luma-task-browser-20261120/verification.json) | 正常 main 取消审计／复制／原账号单任务查询，管理 TCP／合成上游核对、原始红绿日志和宽窄屏；无真实账号 |
| [gateway-luma-task-lookup-20261120](gateway-luma-task-lookup-20261120/verification.json) | 单任务状态／credits 手工查询、运维事实展示、官方 Usage 合同、管理鉴权／transport 测试与合成组件宽窄屏；无真实账号 |
| [gateway-luma-mcp-browser-20261120](gateway-luma-mcp-browser-20261120/verification.json) | 正常 main 工具目录／模型参数、授权拒绝修复、管理 TCP／MCP session 收尾、宽窄屏与原始红绿日志；全部为合成账号 |
| [gateway-luma-inference-browser-20261120](gateway-luma-inference-browser-20261120/verification.json) | 正常 main 精确账号文本测试、结果展示修复、管理 TCP／审计回执、宽窄屏截图与原始红绿日志；全部为合成账号 |
| [gateway-luma-files-browser-20261120](gateway-luma-files-browser-20261120/verification.json) | 正常 main Files 浏览器／管理 TCP 联合回执、宽窄屏截图、失败及成功日志、源码哈希；全部为合成账号 |
| [gateway-luma-files-extended-20261120](gateway-luma-files-extended-20261120/verification.json) | 签名上传／TTL／用户标记官方合同、定向测试和编译回执、源码哈希；无真实账号资料 |
| [gateway-luma-cancellation-20261120](gateway-luma-cancellation-20261120/verification.json) | 取消／超时任务费用审计、原始定向测试与编译回执、源码哈希和规模记录；无真实账号数据 |
| [gateway-luma-files-20261120](gateway-luma-files-20261120/sources.json) | Files 创建接口官方原文、源码范围及本地验证回执；不含真实上传或删除 |
| [gateway-provider-resume-20261119](gateway-provider-resume-20261119/verification.json) | 最新结构化验证、65 个范围文件源码哈希、Inference 原始日志、五页官方文档及合成宽窄屏截图 |
| [gateway-luma-layering-20261118](gateway-luma-layering-20261118/sources.json) | 分层/Create 官方原文、原始失败日志、UI 红绿记录、规模与截图 |
| `gateway-luma-failure-audit-20261117` | 失败费用审计的红绿日志、相邻回归与编译记录 |
| `gateway-luma-settlement-20261117` | 晚到费用补查、堆栈调查、修复后回归与编译记录 |
| [gateway-luma-video-edit-20261117](gateway-luma-video-edit-20261117/sources.json) | 视频重绘合同、测试、范围哈希和合成截图 |
| [gateway-luma-video-operations-20261117](gateway-luma-video-operations-20261117/sources.json) | 视频扩画合同、测试、范围哈希和合成截图 |

原始失败仍是失败：例如 11 月 18 日 `rust-layering.log` 为 13 passed/2 failed；后续修复和通过的记录另存于 11 月 19 日回执。没有修改旧日志或将其改称成功。部分前端及 Agents 检查当时仅保留工具结果，`verification.json` 已明确该边界；本次没有凭摘要伪造原始日志。

公开 HTML 是当时匿名读取的字节快照，只用于合同追溯，不执行其中脚本。源码哈希是当时范围快照，包含继承修改；归档后文档引用更新会改变当前文档哈希，不能要求它们与历史快照恒等。原日志中的绝对路径和旧测试 EXE 名称也是历史信息，不是今后运行指令。

早期 11 月 5–14 日的一些证据目录在迁移前已不存在，54 处历史引用记录在 [missing-historical-evidence.json](missing-historical-evidence.json)。历史进展文章保留当时结论，本档案不补造丢失数据，也不能重新证明那些真实账号验收。

持续使用的构建、合同抓取、命令回执、浏览器验证入口和预览夹具均在 [仓库开发工具](../../../tools/luma/README.md)。旧 snapshot/measure、重复 preview/extract 和专用栈调查脚本不再是接手依赖。新回执先写操作者指定的临时目录，确需长期保存时审查后归档；不要将含秘密的运行数据复制到这里。

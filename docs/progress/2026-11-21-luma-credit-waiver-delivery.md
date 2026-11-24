# Luma 额度放行与最新手测交付

## 用户决定

用户在 2026-11-21 明确要求：如果确认因没有额度无法继续测试，可以放行，没有必要一直测试。沿用原 Edge 唯一创作请求 `HTTP 402 / Not enough credits` 和后续额度页面快照，本轮不再发送付费生成。该项记录为 `waived_insufficient_credits`，不计入实测成功数量，不生成虚假的任务、媒体、取消或账单回执；它不再阻塞现有支持范围的交付。

## 交付范围与修复

此次 desktop devtest 交付包含前次包之后的 MCP 素材复用、18 个模型、配置变更恢复、显式 review、Server 持久管理任务，以及原账号追账、Web SSE／重定向／额度拒绝处理。保留现有工作树改动，构建以整个当前源码为输入；不替换现有 4200 实例、账号或数据。

出包核查发现部署载荷只列出前三份 PostgreSQL 初始化 SQL，遗漏 `004-gateway-key-cash.sql`、`005-gateway-refresh-authority.sql` 和 Luma 所需的 `006-gateway-luma-mcp-creation.sql`。现已补齐。包回归直接使用仓库初始化文件，并逐份比较产物字节、manifest 和 checksum；新初始化文件遗漏时该测试也会失败。它只检查交付完整性，不连接或迁移现有数据库。

修复前真实运行打包回归失败，明确列出三份缺失文件。修改不涉及 Rust 业务或收费逻辑，复用此前已通过的 Luma 58 项 Rust 回归和其他独立切片，不重跑供应商生成。

## 验证和入口

标准入口：`tools/package-gateway-devtest.ps1 -Edition desktop -Jobs 4 -VersionId gateway-devtest-20261121-luma-handoff`。包类型固定为 `devtest`，目录独立且不可覆盖；产物必须通过源码指纹、清单与 EXE 哈希检查。随后用 `tools/luma/check-desktop.ps1` 验证独立数据目录、随机端口、SQLite、管理页及本轮进程清理。

**交付完成：** [桌面入口](../../../release/Gateway/gateway-devtest-20261121-luma-handoff/gateway-ui.exe)，完整目录为 `C:/Users/Public/nas_home/AI/GameEditor/Neuro/release/Gateway/gateway-devtest-20261121-luma-handoff/`。该包基于构建时的整个 dirty 工作树，源码指纹 `3b46b009a758c320698c32847655f86528c8687d3fe8574effca20a7b4d669fb`；构建前后及打包时一致。交付后只补本页、状态页和证据记录，未再修改产品源码或重新构建。

| 验证 | 结果 |
| --- | --- |
| 打包回归 | 修复前 1 failed，准确指出 004／005／006 遗漏；修复后相关 20 项全部通过 |
| 规模与源码检查 | 有效行检查器 31 passed、ratchet、PowerShell 语法、范围内差异与 UTF-8 无 BOM 检查通过 |
| 标准构建 | desktop devtest，4 jobs；后端 13m23s、桌面壳 19.28s；完整重跑命令 1202.56s |
| 包完整性 | 暂存与最终目录的 6152 项校验文件及两份 EXE 大小／SHA-256 通过；全部六份 PostgreSQL 初始化 SQL 与源码字节一致 |
| Luma 载荷 | `browser-operation.mjs` 与模型清单和源码一致；未携带 `.ng`、`.ng-devtest-4300` 或真实 `deploy/.env` |
| 隔离桌面启动 | 新 UI 拉起同包后端；端口 58249，独立 SQLite 就绪，`/ui/` 200；本轮进程全部退出，供应商调用 0 |

| EXE | SHA-256 |
| --- | --- |
| `gateway.exe` | `190d3a571f81cab468d4427f903a1ea85de003cc4a24962d2745ccc885da8f7a` |
| `gateway-ui.exe` | `8274772679207ea5dbcacecd23d4bc9892d85e70c4e961a343a22a062ba173b3` |

首次 Windows PowerShell 构建在编译前因继承 PowerShell 7 模块路径而找不到 `Get-FileHash`，如实记录为失败。按仓库 `powershell_test_utils.py` 的既有约定，仅移除该子进程继承的 `PSModulePath` 后重跑通过，不改变系统配置或业务代码。

长期证据见[验证总账](../evidence/luma/gateway-luma-delivery-20261121/verification.json)、[额度放行记录](../evidence/luma/gateway-luma-delivery-20261121/credit-waiver.json)、[包检查](../evidence/luma/gateway-luma-delivery-20261121/package-inspection.json)及[隔离启动](../evidence/luma/gateway-luma-delivery-20261121/desktop-smoke-result.json)。持续工具在仓库，临时目录不作为交接依赖。

测试前后的原 Gateway UI PID 24600、后端 PID 6620 均保留。当前实例没有替换；手动使用新包时应先正常退出旧版，再启动新包的 `gateway-ui.exe`，避免默认 4200 端口冲突。此次隔离启动不等同原生 Luma 业务点击通过。

## 支持边界

- MCP 分层布局、其他交互题型和任意 client tool 尚未实现完整合同；额度测试放行不等于这些能力已经实现。
- Web 原任务成功回执、完成事件与最终媒体关联没有真实成功样本；该付费验收按用户决定跳过，不再为此循环。
- L4 以已实现的 credits 事实追账和进程恢复范围收口，不新增 USD 换算或远端取消保证。
- 本轮不提交、推送、部署或更改原实例；新包属于日常手测，非正式 Release。

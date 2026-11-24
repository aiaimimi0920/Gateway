# Luma 六项收尾：大文件与持久追账

用户明确重启 L1–L6，允许读取已有 MCP／Web 账号，暂不执行收费生成。本批只完成已声明范围，不扩展其他服务商或新增媒体合同。

后续源码增量：[Local MCP 显式图片／视频创作](2026-11-20-luma-mcp-creation.md)已完成窄流程实现，MCP 回归 36 项、共享资产相邻 1 项、前端 34 项通过。下面包回执仍只对应大文件／追账批次，不包含后来新增的 MCP 创作代码；L1 全范围未据此关闭。

## 改了哪些内容

- Files 签名上传上限提升到 5 GiB，保留小文件缓冲兼容路径。大文件以流式 multipart 连接单次 storage PUT，入口限制解析器预读、输出有背压；校验文件长度、字段数量与结束边界。输入闲置 60 秒终止，大文件上游总预算 1 小时；只有输入和 PUT 均成功才 complete。没有 detached producer，也没有磁盘 spool。
- 原审计行记录已验证的任务和账号绑定，后台每轮最多处理 8 项；仅 GET 同一任务，最多 12 次，每次 30 秒，间隔上限 64 分钟。费用未知不写零，排队时的零费用不作为终态结算。401／403、不同任务身份或账号／revision 改变停止追账。
- SQLite 与 PostgreSQL 使用 CAS 同时比较任务事实和领取记录，阻止重复领取及陈旧写入。复用 SQLite 实例维护；Server 超过任务最大生命周期的 checkpoint 可补读费用。原请求的成功／取消／失败、mediaVerified、次数余额和 cash 均不由追账改写。正常 finalizer 保留追账标记与已经补回的费用。
- 持续 PostgreSQL 入口 `tools/luma/check-postgres.ps1` 使用已缓存镜像、独立随机端口和 tmpfs，应用真实仓库 DDL；测试核对本次运行标记。所有持续脚本和回归源码在仓库，临时目录只存运行数据。

## 验证与问题记录

公开 Files／create／complete 合同已于 2026-11-20 联网读取，三页 HTTP 200，原文和 hash 见[合同归档](../evidence/luma/gateway-luma-six-20261120/contracts/sources.json)。合同仍是申请签名地址、单次 PUT、原 ID complete；未提供 range 或 multipart 断点续传保证。

初轮上传测试暴露 multer 会把连续 ready 的输入提前全部缓冲；修复入口 yield 和分块后，5 GiB 合成流的背压／取消通过。第二轮相关回归为 366 passed／4 failed／6 ignored：失败来自公共路由绑定使用了错误的候选字段，以及完整 multipart 输入过早耗尽 metadata 预算。这两处已按真实调用点修正，最终复测以本目录归档的验证回执为准，不将初轮结果计作通过。

最终 Rust 定向及相邻审计生命周期回归 **387 passed / 0 failed / 7 ignored**；其中 PostgreSQL 专项另显式执行 **1 passed**，使用真实仓库 DDL，覆盖双连接竞争、过期 running 任务追账和晚到 finalizer，修正后的脚本已验证结束时自动停止本轮容器。生产 `runtime-defaults` 和关闭功能的 `--no-default-features` 检查通过。前端 Files 定向 **16 passed** 和类型检查通过；有效行检查器 **31 passed**，ratchet 通过，`git diff --check` 与改动 Rust formatter 通过。原生包与启动结果见下一节。

第一轮 PostgreSQL 入口的结束阶段发现 PowerShell 将 Docker template 中的引号剥离，身份检查安全拒绝停止容器。已改为读取 labels JSON；对原容器核对同一个运行标记后完成停止，没有触碰其他容器。前端复测曾错误从仓库根直接启动 Vitest 而缺少 jsdom；使用项目自己的 `npm run test --prefix apps/desktop` 后 16 项全通过，没有修改测试或生产行为来规避该环境错误。

## 原生交付与隔离运行

标准桌面 devtest 包已发布到 `C:/Users/Public/nas_home/AI/GameEditor/Neuro/release/Gateway/gateway-devtest-20261120-luma-large-files-reconciliation/`。构建前后源码指纹一致，manifest／build provenance 均为 devtest；暂存与最终目录的两个 EXE、6123 个清单文件及 SHA-256 均通过标准检查。总命令耗时 962.03 秒，后端 Cargo 11 分 24 秒，桌面 Cargo 12.91 秒；这是本次实测，不作为冷／热构建提速承诺。[构建回执](../evidence/luma/gateway-luma-six-20261120/package-result.json)与[来源记录](../evidence/luma/gateway-luma-six-20261120/package-build-provenance.json)已归档。

| 文件 | SHA-256 |
| --- | --- |
| `gateway.exe` | `bc887d71e00ce267334819ea9ab50c5af3896e842f2ff26f802255ac2ce99c7d` |
| `gateway-ui.exe` | `32199345482a6dbe8d6102d61c7ecc0bde6bf3d6c84b9f9857b446a541c517d0` |

[本地运行回执](../evidence/luma/gateway-luma-six-20261120/local-smoke-result.json)通过：独立数据／随机端口启动、SQLite 就绪、Key／余额重启保留、路由失败退款、轮换撤销旧 Key、零 PostgreSQL／Redis 连接，两次后端均正常 drain。没有供应商生成调用。

[桌面运行回执](../evidence/luma/gateway-luma-six-20261120/desktop-smoke-result.json)通过：新桌面 EXE 拉起同包后端，父子进程关系核对成功；独立端口 55348 返回 SQLite 就绪，`/ui/` 返回 200，数据库落在指定临时目录，检查结束无本轮进程残留。原有 4200 后端 PID 4652／桌面 PID 4076 保留。没有执行 Luma 原生页面点击或真实业务，不用进程启动代替业务验收。

持续入口为 `tools/luma/check-desktop.ps1`，复用现有标准包／桌面检查；不依赖临时目录中的脚本。首轮遗漏复用 HTTP 辅助函数所需的 `Write-Smoke`，如实记录失败并清理本轮进程；补齐后使用同一个包通过。有效行工具的一次绝对输出路径调用被拒绝，改用仓库标准命令后通过，未改工具策略。出包后仅增加此检查脚本、文档与归档回执，产品源码未再改动；不重建已验证的产品二进制。

## 尚未关闭的边界

1. 用户随后在现有 Gateway 保存 MCP 账号，[当前真实只读验收](2026-11-20-luma-mcp-live-contract.md)已取得 26 项完整工具合同、18 个模型和三项参数，MCP 合同阻碍解除；创作执行链仍未实现。Edge 窗口读取的工具故障仍影响 Web wire 获取，MCP 不能替代 Web 协议。没有收费生成、账号刷新或复制真实浏览器资料。
2. 供应商受理但尚未返回任务 ID 的窗口不能自动恢复；不尝试重发生成。没有远端上传续传保证。
3. PostgreSQL 存储验证不能代替完整 Server／Redis 业务或独立操作系统进程强杀验收。数据库托管凭据与文档路由账号的绑定来源须分别验收。
4. 不将媒体 credits 虚构为 USD/cash；现有不支持的 cash 媒体入口继续拒绝。
5. L3 的已声明本地实现与 L5 包交付可收口；L4 已完成核心实现及存储验证，完整边界仍未关闭。L1／L2／L4／L6 共 4 项未完全关闭，不按四个等量小任务估算。当前 4200 实例保持原样，未提交或推送 Git。

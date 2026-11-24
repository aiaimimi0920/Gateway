# R14：Luma 单任务查询正常网页联合验收

日期：2026-11-20（Asia/Shanghai）。承接[单任务查询与审计展示](2026-11-20-provider-channel-expansion-luma-task-lookup.md)，补正常 main 网页到真实管理 TCP 的联合证据。供应商端仍为不转发外网的 loopback 合成服务，不使用真实账号或生成额度。

## 本批范围

仓库新增 `src/upstream/luma_agents/task_browser_host.rs` 显式 opt-in 宿主和 `tools/luma/task-browser-check.mjs` 浏览器入口，复用原生产本地启动、公共媒体 pipeline、SQLite 审计、管理鉴权、上游 transport、正常前端构建及 loopback 页面代理。测试模块只在 `test` 与 Agents feature 同时启用时编译，没有新增生产端点或查询语义。

宿主先在合成上游收到 queued 后取消原公共生成请求，由现有取消 owner 持久化审计；浏览器正常登录后从运维详情复制任务 ID，再进入原 Agents 账号查询未知／0／12.5 credits。另检查供应商 401 不注销管理会话、429 和错 UUID 拒绝、无效输入不调用、关闭／账号切换清空及宽窄屏。新查询不会改写原 cancelled、queued、未知 credits 或本地媒体验证事实。

浏览器只允许本轮页面 origin，页面及管理 API 响应不作模拟替换。宿主独立核对六次查询 GET、精确账号 Bearer、无管理凭据泄漏，以及原审计全文、配置 revision 和普通 Key 余额不变。构造取消审计的 POST／GET 两次调用单列，不能把它们称为查询引起的生成。浏览器完成后关闭会话及代理，原子写入 finish／abort；宿主收到命令或到达等待期限后关闭资源，再判断验收结果。

## 验证记录

最终浏览器 `passed`，显式 Rust 宿主 **1 passed / 0 failed / 0 ignored**。六次管理请求返回 200／200／200／401／429／502，全部 `no-store`；六次查询均命中原 UUID 和原账号，没有生成、下载或重试。原取消审计逐字段不变、普通 Key 余额仍为 2、配置 revision 不变；正常登录和敏感授权均为 200，无页面异常或外网请求。1440px／390px 截图已检查，窄屏任务 ID 和滚动后的费用字段均可见。

首次浏览器因错误等待复制按钮改名而失败，宿主按 abort 关闭并报失败。读取共享 `CopyButton` 实现后将脚本改为检查按钮内的 `role=status`，实际剪贴板 UUID 一并校验；生产复制实现没有修改。原失败日志保留，后续通过结果另存。首次测试宿主编译 16m54s；最终同源码复用 Cargo 指纹编译 1.59s，这只是本次测量，不能外推一般提速倍数。

正常 main 构建、Rust formatter、Node 语法、规模检查器 **31 项测试**与 ratchet 通过。新增宿主 169 有效行；模块注册 120→122，仅增加 `cfg(test)` 声明。上一批 20 个范围源码中其余 19 个哈希未变，生产逻辑及前端原有 123／32 项回归、生产／feature-off 编译结果仍引用[上一批回执](../evidence/luma/gateway-luma-task-lookup-20261120/verification.json)，本批没有把它们写成重新运行。

本批命令、红绿日志、源码哈希、截图和资源核验见 [verification.json](../evidence/luma/gateway-luma-task-browser-20261120/verification.json)。

已归档 47 份文件、433,685 bytes，逐文件复制后 SHA-256 全部一致，见 [archive-index.json](../evidence/luma/gateway-luma-task-browser-20261120/archive-index.json)，计数不含索引本身。没有复制数据库、凭证状态、浏览器 profile 或完整构建目录；六个管理／页面端口、三次测试宿主及本轮浏览器均已结束。新增浏览器脚本最终为 143 有效行。

## 后续边界

持续入口和双终端命令见 [tools/luma/README.md](../../tools/luma/README.md)，不依赖旧测试 EXE 或临时资料；关键回执收进仓库后，清空 `linshi` 不影响接手开发。

本批不证明真实供应商任务／账单、自动追账、历史审计回填、强制崩溃／跨进程恢复、cash 结算或新原生包；MCP 创作完整工具合同和 Web 当前协议仍待补齐。R14、P50/P60/P70 和总渠道计划不因这个验收子批整体关闭。

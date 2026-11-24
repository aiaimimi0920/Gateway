# R14：Luma 单任务查询与审计展示

日期：2026-11-20（Asia/Shanghai）。承接取消审计事实保留和只读 MCP 网页验收。本批让用户查看已记录的 Luma 任务事实，并在已保存 Agents 账号内手工查询同一任务的当前状态／实际 credits；不运行真实供应商生成。

## 使用流程与边界

在“运维”的请求详情中，有合法 `lumaAgentsGeneration` 记录时展示任务 ID、模型、类型、供应商状态、记录时已知的 credits 及可用的本地媒体验证事实。可复制任务 ID，再进入“凭据池 → 原 Agents 账号 → 编辑账号 → Luma 任务查询”，粘贴 UUID 后点击“查询任务与费用”。也可输入在供应商侧取得的 UUID，权限仍由所选账号的 Key 决定。

每次点击只读取一次既有任务，不生成、不轮询、不下载资产。页面区分未知、0 和正数；供应商 `completed` 不等于本地媒体已经验证。查询结果不回填旧请求、不改变原取消／失败结果、不修改本地次数或 cash 账本；原审计展示的是当时记录，查询面板展示的是本次读取结果。账号切换、授权／配置／任务 ID 变化和关闭会取消等待、清空结果并拦截旧响应。

新增管理入口为 `POST /v1/internal/gateway/console/luma-agents-task`，输入只接受 `providerId`、`credentialId`、`expectedRevision`、`generationId`。管理身份／敏感授权／源站检查、运行生命周期、现有账号操作并发槽、精确已保存启用账号和 revision 前后校验均生效。上游固定为 `GET https://agents.lumalabs.ai/v1/generations/{UUID}`，30 秒总截止、64 KiB JSON 上限，使用供应商 Bearer；管理 token、grant 和 Cookie 不进入上游。

复用原生成任务身份／状态／费用校验，返回固定的任务事实字段，不透传提示词、资产地址或诊断正文。模型文字有长度及控制字符限制；调用的 UUID 必须与返回任务一致。无执行线构建返回明确的 `implementation_line_disabled`。

## 合同与验证

本批匿名联网读取[官方 Usage 文档](https://docs.agents.lumalabs.ai/guides/usage/)，HTTP 200；它明确同任务 GET 的 `usage.credits_charged` 为实际已入账金额，完成后仍可为 null，null 不能记为零。原文、抓取时间与 SHA-256 保存于本批归档。对一个未经确认的 retrieve 文档 URL 的探测返回 404，原记录保留；没有把该 URL 作为接口证据。

前端相关 **32 passed**，正常 main 构建与 TypeScript 通过。真实组件合成浏览器覆盖取消记录展示、未知→0→12.5、关闭重开清空、1440px／390px 布局；首次截图发现预览未引入运维及账号弹窗样式，补齐夹具导入后重新检查。该浏览器不是正常 main 到管理 TCP 的联合运行，管理路由与上游 transport 在 Rust 测试中单独验证。

Agents 定向 Rust 回归 **123 passed / 0 failed / 2 ignored**，其中新增单任务查询六项全部通过；两项 ignored 是需独立启动的 Files／媒体探测浏览器宿主，未冒充本批联合验收。生产 `runtime-defaults` 和 `--no-default-features` 编译均通过；formatter、规模检查器 **31 项测试**及 ratchet 通过。预览端口 4452／4453 已关闭，本轮浏览器已退出。

完整命令、计数、源码哈希、规模与资源收尾结果见 [verification.json](../evidence/luma/gateway-luma-task-lookup-20261120/verification.json)。长期脚本 `tools/luma/task-preview-check.mjs`、预览夹具及前后端测试均在仓库；清空临时目录后可依据 [tools/luma/README.md](../../tools/luma/README.md)复现。测试源码 273 有效行，单一职责为任务查询的 transport 与管理鉴权；其余新增源码低于 150，未变更有效行基线。

本批归档 45 份文件、470,882 bytes，逐文件复制后 SHA-256 全部一致，详见 [archive-index.json](../evidence/luma/gateway-luma-task-lookup-20261120/archive-index.json)；该计数不含索引本身。不归档临时数据库、浏览器 profile、凭证或完整构建目录，原临时资料未删除。

## 尚未关闭

自动追账、历史审计回填、强制崩溃／跨进程恢复、cash 结算、供应商真实账号／账单与新原生包继续待办。手工输入任务 ID 不证明该任务属于某条历史网关请求；本批因此没有自动关联或修改历史记录。MCP 创作仍需当前完整工具合同，Web 当前协议仍待恢复。R14、P50/P60/P70 和总渠道计划不因本子批整体关闭。

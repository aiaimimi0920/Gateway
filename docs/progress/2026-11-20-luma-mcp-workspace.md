# Luma MCP 已保存素材复用

## 范围与行为

本批接续 L1，完成 Local 模式下已保存任务的素材 ID 复用。用户可在结果点击“编辑此素材”“用作参考图片”“扩画此视频”，或填写来源任务 ID 和其 ready 素材 ID。支持 UNI-1 图片编辑／参考图片、GPT Image 2 reference medias、Ray 3.2 视频重绘／扩画；图片参考 URL 与素材 ID 共用已有数量限制。

管理入口在 prepare 阶段读取原 SQLite 任务，校验 provider／credential／revision、素材归属、类型和 ready 状态。无需公开下载 URL，不请求素材 URL，不接收任意工作区 ID。请求摘要包含选中来源与素材，准备后篡改选择会被拒绝。新任务持久保存原工作区 key 和 ID，在原工作区打开独立会话，使用新任务的预算和幂等键；供应商返回不同工作区时保留会话供结束，拒绝提交生成。

新任务在发送前持久化绑定，正常关闭／重启后可恢复提交。后代任务沿用根工作区，不错误地用父任务 ID 新建工作区；已准备任务不依赖祖先审计继续保留。旧任务缺少 workspaceBinding 时仍按原 task ID 解析工作区 key，旧请求没有 reuse 时序列化摘要保持不变。复用不修改源素材或源任务，不迁移数据库、不新增存储版本。

界面切换账号清空复用选择，选择结果开始另一任务时清空提示词和预算，要求重新确认费用。申请 secret access 成功后保留当前草稿，但不自动提交。只有 ready 素材提供复用按钮。运维详情仍是只读展示，可复制素材 ID，不触发新创作。

## 合同依据与边界

依据仓库归档的当前账号 [26 项工具合同](../evidence/luma/gateway-luma-mcp-live-contract-20261120/catalog.json)：`luma_session_open.inputSchema.properties.workspace_key` 明确重复 key 使用同一工作区；生成工具的 `RunMediaRef.asset_id` 明确允许工作区已有素材用于所有引用位置。本批只开放来自一个已保存任务的源素材和图片参考，不开放团队 library、任意上传、跨工作区混用、视频首尾／关键帧资产 ID。

本次同时核实了两处仍未足够明确的合同：分层 description 声明 index／label／bounds，但 `luma_run_await.outputSchema.$defs.RunAssetOutput` 未定义这些字段，`OperationResult` 是开放 object；不能移植 Agents REST 的布局当 MCP 已确认布局。`luma_run_respond.result` 为任意 JSON，review 只有在 pending 携带 `answer_format.verdict_schema` 时才有精确回答形状。未猜测其他题型的回答协议，未自动执行远端 custom tool。

其余模型、分层布局、run 交互、Server 持久身份、配置变更后的恢复和真实业务仍保留在 L1。L2 Web 的现有工具阻碍、L4 完整 Server／Redis 与强杀验收、L6 收费暂停不变。仍为四个未完全关闭的工作包，未增加新的必做工作包。

## 本批验证

验证回执归档在 [gateway-luma-mcp-workspace-20261120](../evidence/luma/gateway-luma-mcp-workspace-20261120/verification.json)。只运行修改路径及直接相邻创作测试；不循环跑其他 Luma 渠道、全仓测试或包构建。

- 前端三个相关文件：首轮 12 passed；复核修复 grant 更新清空草稿的问题后，仅重跑两个受影响文件，10 passed。去重共 13 个用例通过，覆盖复用选择、预算／确认、GPT 参考、视频扩画、账号清空、权限更新保留草稿、URL 与 ID 冲突及数量限制。未重复参数文件或浏览器已通过且行为未改变的路径。
- TypeScript 检查通过；真实组件的 Edge 合成预览已通过连续参考／编辑的八次显式请求、逐次确认、1440／390 宽窄屏及非 loopback 请求拦截，截图已检查。它不冒充正常管理 TCP 或真实供应商验收。
- Rust 定向创作测试：15 passed／0 failed，6428 个无关用例被过滤；新批五项覆盖素材／工作区绑定、未知／跨账号／跨 revision／类型错误／未 ready 拒绝、旧 journal 读取、正常重启与连续复用、工作区替换后不提交。只编译／运行一次，命令总耗时 973.44 秒，测试实际耗时 10.04 秒，其余主要为 Rust 库编译。
- rustfmt、31 项有效行检查器测试及 ratchet 通过；未修改 baseline。本批生产文件最大 261 有效行，新测试文件 279 有效行；没有扩张大文件债务。最终 diff／UTF-8 无 BOM 检查及源码哈希已归档。

持续测试代码、夹具和 `--reuse` 浏览器入口全部位于仓库；一次性日志／预览产物写入 linshi，关键证据复制进本仓库。没有真实收费、账号变更、运行实例更新、提交、推送或重新出包；现有 `gateway-devtest-20261120-luma-mcp-options` 不含此增量。

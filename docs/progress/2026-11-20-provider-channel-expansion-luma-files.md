# Luma Agents Files 小文件管理

本批将 Files 接入正常已保存 Agents 账号的编辑界面：上传小文件、分页读取、刷新文件状态、复制 ready 文件的 `file_id`，以及确认删除。对应 R14 的一个子范围；不关闭 R14/P50/P60/P70，也不减少 Magpie 67 项。

## 行为和边界

- 管理入口为 `POST /v1/internal/gateway/console/luma-agents-files`，JSON 包含 `providerId`、`credentialId`、`expectedRevision` 和 `operation`。只解析保存的 `lumalabs-api` 账号；禁用账号、跨 provider、过期 revision、无 secret grant 或外站 origin 在发送前拒绝。普通终端用户 Key 不能访问这个管理入口。
- 上传使用 `operation: { action: "upload", filename, purpose, data }`，data 为 base64；上游转换为固定 Agents origin 的 multipart。`purpose` 仅 input/reference。Gateway 本地限额为非空、≤8 MiB，JSON ≤12 MiB；部署自定义的全局请求限制如果更低，仍优先。文件名拒绝路径、控制字符、引号和反斜线，保留正常中文。
- `list` 固定每页 25 项，只回传 opaque cursor；`get/delete` 要求 UUID。201 上传结果必须镜像嵌套文件的 ID/状态，get 必须返回被查询的 ID，204 删除才确认成功。删除不可通过 API 恢复；不声称底层存储字节立即清除。
- `pending`、`ready`、`failed`、`deleted` 分别显示，只有 ready 启用复制 ID。查询失败会撤下旧记录，防止已失效的 ready 信息继续显示。复制的 ID 可用于已有媒体测试 `file_id` 输入；没有自动发起生成。
- UI 不在打开窗口、收到授权、切换账号或翻页之后自动发额外操作。账号、管理身份、grant、revision 或锁定状态变化会取消旧请求、清空文件与上传选择、撤销删除确认，并丢弃晚到结果。
- 所有操作不重试、不重定向、不跨账号回退。停止等待只取消本地等待；上传/删除可能已生效，提示先刷新核对。没有跨进程 receipt 或 exactly-once 保证。
- 鉴权与现有两个账号操作槽的准入先于 JSON 缓冲。读入限 30 秒；上游调用含响应读取限 60 秒；metadata 解码后总量 ≤256 KiB。响应只保留文件白名单 metadata，不返回任意 failure_reason、存储 URL、签名或原始错误。没有 detached 后台上传、文件落盘或配置/数据库迁移。

代码分属 `src/protocol/luma_agents/files.rs`、`src/upstream/luma_agents/files*.rs`、`src/http/routes/internal_console/luma_files.rs`；前端 API schema、hook 和 panel 分别位于现有 `api/console` 与 `features/console`。复用正常管理鉴权、现有账号编译结果与产品材料检查，未修改其他执行线。

## 公开合同

2026-11-19–20 日（本地时间）重新匿名读取官方 [Files guide](https://docs.agents.lumalabs.ai/guides/files/) 和 [Create File](https://docs.agents.lumalabs.ai/api/resources/files/methods/create/)。guide 与前批归档 hash 一致；create reference 明确 201、文件字段与可选 upload/filename。默认 Python User-Agent 读取索引返回 403，使用仓库既有研究用途 User-Agent 后读取公开文档成功；没有登录、执行网页脚本或调用真实 API。

新创建接口原文、时间及 SHA-256 保存在 [本批归档](../evidence/luma/gateway-luma-files-20261120/sources.json)。`tools/luma/fetch-contracts.py --page files-create` 已成为可重复入口。脚本、前后端测试都在仓库；临时日志和本地测试数据留在操作者指定 scratch，不作为持续开发依赖。

## 验证

前端累计 44 个不同测试通过，涵盖 Files、正常全局账号编辑接线及相邻媒体表单。四文件并行运行最初有一个既有视频关键帧用例超过 5 秒；该文件单 worker 重跑 5/5 通过，未改其断言或生产语义。新增禁用账号判断曾遗漏可空 initialValue，类型检查捕获后已修复；最终 TypeScript 与正常 main Web 隔离构建通过。

后端初次编译暴露误用未安装的 chrono，已改用仓库现有 time。第二轮 8/9 通过，真实 HTTP 断言发现连续 `.query()` 会覆盖已有 limit；现改为一次提交 limit/cursor，保留原断言核对完整查询。最终 `cargo test --locked --offline --lib --no-default-features --features runtime-defaults -j 4 luma_agents --no-fail-fast` 为 **104 passed / 0 failed / 1 ignored**，其中 Files 新增 9 项。覆盖 multipart 原始字节、Unicode 文件名、固定身份与 headers、分页 cursor、metadata 白名单、坏响应/HTML/大小/不重试/取消，以及真实 Local bootstrap→管理 router→loopback 上游的上传→pending→ready→删除→404。普通 Key 余额保持 2；跨账号、禁用、旧 revision、无 grant、外站 origin 均不发送。唯一 ignored 为需要独立浏览器会话的既有宿主测试。

生产 lib 的 `runtime-defaults` 与代表性关闭组合 `family-openai-compatible-official-api` 均通过 `cargo check --locked --offline --lib --no-default-features --features <组合> -j 4`。这是编译检查，不声称运行了关闭组合的测试 EXE。各自保留 warnings；没有为本批修复清理无关警告。范围 rustfmt 和 `git diff --check` 通过。结构化结果见[验证汇总](../evidence/luma/gateway-luma-files-20261120/verification.json)。

checker 31 项通过；最终 ratchet 通过。新增源码/测试各 ≤242 有效行；现有 `ConsoleGlobalDialogs.tsx` 477→483、管理 router 488→489、内部入口 451→453、transport 171→174，均未跨越 500。源码的 UTF-8 无 BOM 与 19 个范围文件哈希已记录到[源码清单](../evidence/luma/gateway-luma-files-20261120/source-manifest.json)。原始失败日志和后续成功日志分别保留在同一归档目录，不能把前者改称成功。

## 剩余工作

大文件 presigned/PUT/complete、TTL/user_id 扩展尚未实现。未使用真实 API Key，未上传或删除真实文件、未发生真实收费。Files 正常 main→管理 TCP 联合、真实摄取/同 client 文件隔离/生成复用/物理清除、Server/跨进程和原生 EXE 验收未执行；本批未出包、提交或推送。Luma 的费用取消 receipt、截止后追账、MCP 创作和 Web 当前 wire 等继续见[状态清单](../providers/luma-development-status.md)。

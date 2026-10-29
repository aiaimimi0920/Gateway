# Search / Fetch 工具接入与通用路由（2026-10-26）

## 完成范围与运行边界

本轮已完成源码修改与定向验证，未更新正在运行的 4200 实例。未重启服务、修改在线
凭据或数据库、生成新的手测分发包、提交或推送。测试均为本地单元测试或随机回环端口
的模拟上游，不代表真实供应商账号的 Fetch scope、额度或付费业务验收已经通过。

- 对外移除 `linkup-balance`、旧共享余额操作及其模型列表入口。公共余额接口明确返回
  `unsupported_search_provider_endpoint`；内部运维余额查询、用户计费、历史数据保留。
- 模型工具分类名称改为小写 `tools`，保留 `linkup-research` 等专用操作。
- 补齐 `you-fetch`、`tavily-fetch`、`websearchapi-fetch` 的 preset、请求转换、认证、
  响应校验、手动精确凭据探测与前端模板。
- 新增 `search`、`fetch` 通用工具模型，保留现有厂商专用模型。

接口、示例及官方依据见 [search-fetch-tools.md](../search-fetch-tools.md)。

## 路由与兼容性

路由快照从供应商已声明的操作和路径生成内存映射，例如 `search -> you-search`；
不改写持久化配置或扩大旧账号的模型权限。单目标映射复用严格凭据模型检查。
`search`、`fetch` 为保留池名称，不被模糊别名捕获，也不会回退到聊天供应商。

候选仍受 Key 权限、账号分组、具体凭据 supported_models、禁用、冷却、额度及协议
能力过滤。通用请求仅授权在已过滤的工具池内沿用现有健康排序、黏性与有限故障转移，
没有新增 AI 质量评估、自动付费探测或后台搜索。单 URL 供应商不接收批量 URL，不能
静默丢弃其余目标。通用 Linkup Search 补齐必需的 standard/searchResults 默认参数。

历史不传 model 的默认路由保持兼容；通用池需要显式传 `model: "search"` 或
`model: "fetch"`。响应保留供应商原生 JSON，并非统一结果 schema。普通聊天不会
因为出现这些工具名称而自动执行搜索或读取，上层应用仍需处理工具调用循环。

旧账号若只声明 `you-search`，通用 Fetch 不会绕过其限制；需要通过现有编辑器启用
对应操作。You Contents 还需要上游产品 scope，Search 成功不能作为 Contents 授权证据。
新增模板仍默认关闭自动测试，仅默认选取专用 Search，避免别名重复触发调用。

## 输入、响应与资源审查

Fetch 校验 HTTP(S) URL；最多 20 个目标，每个 URL 最多 8192 bytes，拒绝同时传
`url` 与 `urls`。读取 JSON 响应的上限为 8 MiB，错误摘要不回显上游正文或凭据。
新增三家接口的 HTTP 200 空结果、全失败、业务错误或无效 JSON 不算成功；Tavily
部分成功保留原生 results/failed_results。You 只有显式 metadata-only 请求才允许
仅 metadata 成功，不能用元信息冒充正文。其他旧供应商保留原生响应语义。

实现复用现有发送、鉴权和取消边界，没有新增动态认证目标、后台队列、数据库迁移或
凭据写入。回环契约服务仅使用合成 Key，任务结束执行 abort 并等待回收。

## 实际验证结果

| 检查 | 结果 |
| --- | --- |
| Rust 搜索、Fetch、协议、preset、路由定向单测 | 84/84 通过 |
| You、Tavily、WebSearchAPI、Linkup、Exa 现有搜索探测合同 | 5/5 通过 |
| 新增公共 Search/Fetch、精确凭据探测、故障转移、余额禁用合同 | 2/2 通过 |
| access_key_groups_contract 权限与分组回归 | 6/6 通过 |
| 前端六家模板、分类、目录对话框、模型卡片 | 9 文件，99/99 通过 |
| TypeScript typecheck | 通过 |
| 有效行 checker 测试 | 31/31 通过 |
| 有效行 ratchet、cargo fmt、git diff --check | 通过 |

Rust 使用 `--locked --profile devtest --jobs 4`，未运行全量回归。搜索定向命令包含
`--lib`、上述五个探测合同与新增合同，并使用 `-- search --test-threads=1`；
权限合同另以无名称过滤的完整 6 项运行。不能把被过滤的测试计为通过。

新回环契约最初因误用 Axum 0.8 路由语法返回 404；按仓库 Axum 0.7 修正后，进一步
去掉与动态路由冲突的 catch-all，改用明确的模拟接口路径。最终 2 项均通过，生产
源码没有为迎合该夹具错误作修改。编译保留仓库既有 unused/linker 警告，未扩大清理范围。

本轮新增及修改的相关源码均低于 500 有效行；没有新增超限文件、增长超过 700 行
的历史文件或申请例外。ratchet 报告中的较大文件属于已分类 immutable runtime
artifacts，`governed source >700=0`。文本使用 UTF-8 无 BOM。

未执行真实付费上游请求、全量 Cargo 回归、正式发布安全审计、桌面打包与原生手测。
工作区原有其他未提交改动保持，不将其归入本轮功能成果。

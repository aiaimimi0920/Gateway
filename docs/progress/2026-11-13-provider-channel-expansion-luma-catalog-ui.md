# Luma MCP 空池创建与只读工具目录入口

## 本批改动

- 正常服务商目录新增 `lumalabs-mcp` 模板，默认服务商 ID 为 `lumalabs-mcp-oauth`，与 Web、Agents API 分开。通过已有配置保存流程创建固定 MCP resource 的显式空账号池，不创建模型路由、默认账号、秘密、自动测试或调用流程。
- MCP 创建页只保留 Provider ID、固定 Base URL 和添加操作，不展示不支持的通用配置页签。构造函数拒绝外部地址、模型、Key、自动任务、调用适配器与存储等不适用输入；不通过放宽后端准入解决入口问题。
- 跳过通用号池 driver 自动选择。交叉审查发现存在唯一兼容 driver 时，先通过校验的 draft 会在提交前被注入 driverId，导致 MCP 创建失败；新增回归先复现，再修正，不允许该 profile 接入自动任务。
- 已保存 MCP 账号的授权界面增加“读取工具目录”。只有显式点击且敏感访问已获准才请求现有 `/console/luma-mcp-tools`；缺授权仅请求确认，grant 到达后不自动重放。
- 展示工具名、说明和 input/output schema；全部为只读文本，不执行工具、不跟随说明链接、不把工具目录登记为模型能力或健康证据。一次只呈现一个工具的详情，复用现有控件与滚动区域，没有新 CSS、依赖或主题。
- client、管理身份、grant、账号、revision 和禁用状态共同隔离结果；切换或关闭取消本地读取，旧响应不能覆盖新上下文。支持停止读取，错误不回显任意上游正文，不保存目录到浏览器存储。

## 验证

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-luma-catalog-20261113`。

- `red.log`：目录入口补齐前 4 项失败；`tools-red.log`：工具入口补齐前行为测试失败。`driver-red.log`：唯一 driver 自动注入的回归失败，均保留原失败记录。
- `final-focused-v2.log`：12 个文件 **66 项通过**，覆盖新增入口／工具目录、既有 Luma OAuth、Agents、全局 Dialog 和正常服务商目录交互。新增两个测试文件共 17 项。
- 较宽目录回归并非全绿：`related.log` 初次 8 项失败，其中本批 MCP 默认 ID 不满足既有 OAuth 后缀规范已修为 `lumalabs-mcp-oauth`；`names-retry.log` 的本批模板已通过，剩 6 项失败来自 Kiro 缺 profile、Cloudflare／LobsterAI ID 和 DIM／Nous 标签预期。另一个 health 默认测试同样缺 Kiro profile。这 7 项相邻失败保留，没有修改其他产品或放宽测试使之变绿。
- 前端类型检查、桌面与 BrowserConsole 真实入口构建通过，构建隔离在上述目录；没有覆盖现有 dist、生成完整 EXE 或发布包。最终记录为 `typecheck-v2.log`、`build-v2.log`。
- checker 测试 31 项通过；有效行 ratchet 通过。`providerCatalog.ts` 有效行 498→500，仍为原目录组合责任；Dialog 107→107、Form 78→78、OAuthPanel 54→56。新模块和测试均不超过 100 有效行，未调整 policy、baseline 或例外。
- 范围 UTF-8 无 BOM、空白检查与源码检查完成。React 检查重点为稳定 scope、请求清理、组件入口条件及共享可访问控件；只读交叉审查发现的 driver 问题已补回归修复。

## 浏览器观察及边界

使用生产 `ProviderCatalogDialog` 和 `LumaOAuthPanel`、真实主题及本地模拟 API，在 Edge 中观察显式目录读取、工具选择与只读长文本、Escape 关闭及焦点回到打开按钮；正常目录点击 MCP 后提交得到 `lumalabs-mcp-oauth`。这是组件级模拟验收，不是完整管理服务或真实账号联合。

本次 viewport override 未实际改变页面宽度，实际观察仍为 1036 像素宽；不宣称窄屏验收通过。已重置 override、关闭测试标签，停止临时 HTTP 服务并确认监听消失。截图显示存在渲染帧延迟，不以点击后立即截图作为新状态成功依据，提交结果以随后 DOM 回执为准。

## 剩余项

本批仅退役“目录创建 MCP 空池入口”和“只读工具目录 UI”两项本地实现缺口。产品化动态 client 注册、refresh、实际模型发现、生成／取消／结果／费用业务、远端 MCP session DELETE、真实账号正常 Gateway 联合、跨机器授权、窄屏与原生包仍未完成。用户仍须提供已登记的 client 与回调地址；不把调查 client 写入默认配置。

无真实供应商请求、生成、下载、积分消耗、凭据／数据库／运行实例修改；无提交或推送。Luma Web 与 Agents API 的其他剩余项不因本批减少，Luma 整个产品项仍未关闭。

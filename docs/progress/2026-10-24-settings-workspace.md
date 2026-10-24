# 设置界面整理（2026-10-24）

## 范围与交互

- 真实控制台入口 `ConsoleIndependentWorkspace` 接入 `ConsoleSettingsWorkspace`。
- 将当前浏览器／桌面客户端的主题、语言与当前 Gateway 的管理认证分为「界面偏好」和「管理安全」。
- 使用现有 Neuro 主题令牌；主题提供三种示意预览，语言使用紧凑原生单选控件，保留方向键与焦点提示。
- 偏好继续使用原 localStorage 键，选择后立即保存；不增加路由、协议、存储版本或管理接口。
- 管理密钥按需加载，不再在只查看外观时请求。密钥编辑区域不折叠；编辑、删除确认或请求进行中禁止切换设置内分类，并解释原因。
- 加载 effect 不再依赖翻译函数；切换语言不会丢弃草稿、重置 pending 或忽略已提交请求的结果。错误以稳定类别保存，在渲染时翻译。
- 管理密钥的身份确认、最后一把保护、隐藏明文、退出登录条件与原 API 均保留。明确区分改名与替换密钥。
- 管理列表通过容器查询适应双侧栏造成的窄内容区，不只依赖 viewport breakpoint。

分类保护仅覆盖设置页内部，不声称保护离开整个控制台工作区后的未保存草稿；顶层导航行为未修改。

## 验证

以下均为本轮实际结果：

- `npm run typecheck`：通过。
- 设置工作区、管理密钥交互、管理 API 与主题 Provider 四文件：24/24。
- `BrowserConsoleApp.shell.test.tsx -t "puts a settings button"`：真实控制台设置入口 1/1。
- `styles.test.ts`：14/14。
- 有效代码行 checker：31/31；最终 ratchet 通过，未修改 baseline 或 exception。
- `git diff --check`：通过；本次源码 UTF-8 无 BOM。
- 使用原 `rsbuild.config.ts`、真实 `main.tsx` 与现有锁定依赖完成 Web 编译。仅将输出改至 linshi、禁用 live-dist 发布／清理插件；未覆盖已有运行资产。这不是正式构建／发布凭据。
- Edge 内使用真实 `AppShell`、设置组件及 Provider 进行隔离组件验证。管理 API 由明确标注的测试数据代替，所有真实管理写入均未执行。
- 实际 CSS viewport 1280×900、853×600、540×800；管理列表分别在宽屏与窄屏完整显示。853px 窗口中的 442px 内容区从 `scrollWidth=508` 修复为 442；540px 窗口中的列表 `clientWidth=scrollWidth=502`。
- 深色中文、浅色英文、键盘主题切换、语言切换、编辑时分类禁用、Escape 取消及焦点恢复已检查。浏览器日志出现既有扩展的 EventEmitter 警告，不作为应用错误或零告警结论。

相邻 shell 整文件运行曾为 6 通过、1 失败：`renders non-codex providers in the same compact tree ledger layout` 期待 `data-group-id=""`，实际为 `"default"`。只读追踪确认 HEAD 已有 `withDefaultAccountGroup` 的 default 兜底，与设置接线无关；未修改该业务或断言，也未声称完整回归全绿。

## 有效代码行

| 文件（`apps/desktop/src/` 下） | 修改前 | 修改后 |
| --- | ---: | ---: |
| `features/settings/AppearanceSettings.tsx` | 101 | 87 |
| `features/settings/AppearanceSettings.css` | 新增 | 76 |
| `features/settings/ConsoleSettingsWorkspace.tsx` | 新增 | 50 |
| `features/settings/ConsoleSettingsWorkspace.css` | 新增 | 48 |
| `features/settings/ConsoleSettingsWorkspace.test.tsx` | 新增 | 116 |
| `features/settings/ManagementSecuritySettings.tsx` | 96 | 108 |
| `features/settings/ManagementSecuritySettings.css` | 21 | 36 |
| `features/settings/SettingsSection.tsx` | 38 | 50 |
| `features/console/ConsoleIndependentWorkspace.tsx` | 112 | 104 |
| `styles/settings-controls.css` | 156 | 167 |

## 证据与边界

本地证据：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-settings-20261024/`。
包括隔离预览源文件、Web 编译产物、源码计数与编码报告、宽窄屏截图。截图中的密钥名称为测试夹具，不是真实运行数据。

未生成 EXE 或 devtest 包，未重启或替换现有实例，未修改真实凭证、数据库或 `.ng`。未做付费上游测试、Rust 全量回归、正式发布审计、提交或推送。临时预览服务和浏览器标签已关闭，viewport override 已恢复。

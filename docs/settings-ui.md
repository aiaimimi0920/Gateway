# Gateway 设置 UI / UX 迁移

2026-11-02 开始将已讨论的 R21 HTML 设计迁入真实产品。
这是第一阶段界面迁移，不代表原型中的全部新业务能力已经实现。

## 外壳与信息架构

- `AppShell` 使用 16px 品牌标识、38px 品牌行、独立第二行侧栏切换。
- 展开侧栏为 212px，窄屏为 184px；左下保留单独设置入口。
- 收起时移除侧栏，工作空间进入顶部导航，设置分类横向显示。
- 设置六类为常规、网络与共享、隐私、可观测性、同步与备份、关于。
- 设置正文为连续面板，四边 10px；窄屏与大字号只在条目内换行。
- 页面根容器固定在视口内，长内容由正文或有界列表滚动。
- 刷新沿用控制台真实数据刷新回调，不使用页面重载替代。
- 尚无真实应用更新检查信号，因此不显示更新按钮，也不使用原型示例版本。

`SettingsNavigation` 保留 `backend` / `desktop` 两个内部原生桥接标识，
它们不是额外的公共设置分类。未连接的可信本地窗口仍只提供常规与网络连接设置。

## 已接入能力

| 分类 | 本阶段可用功能 | owner |
| --- | --- | --- |
| 常规 | 外观三段、中文/English | 既有 UiThemeProvider / UiLocaleProvider |
| 常规 | 文字大小、界面/代码字体与字重 | 新的本地 displayPreferences |
| 常规 | 退出/托盘、开机启动 | 可信本地子 WebView 的既有桌面 prefs |
| 网络与共享 | 当前地址、本机/远程、连接测试、显式应用 | 可信本地子 WebView 的既有连接命令 |
| 网络与共享 | 面板管理密钥列表、显示、添加、编辑、删除 | 当前 Gateway 管理 API |
| 可观测性 | 实际后端日志目录复制、保留时间、容量、诊断和日志导出 | 当前 Gateway diagnostics API |
| 关于 | 管理界面构建版本、仓库 MIT 主许可、项目 GitHub 入口 | 源码元数据 |

浏览器版连接当前站点，不开放原生网关切换或系统行为。
关于页版本不代表远程后端版本。MIT 信息不是第三方依赖许可汇总。

## 明确未接入的项目

这些条目保留已讨论的位置和紧凑控件形式，但禁用并说明不可用。
没有保存模拟状态、假连通成功、假脱敏、假同步、假下载或假更新。

- 常规：模型名称供应商展示、防止休眠。
- 网络：代理三态/认证/测试、裸模型 ID、繁忙模型排序、LAN 共享、Origin 列表、浏览器访问策略。
- 网关本机端口仍由桌面后台固定管理，只读展示；不能把只读端口当成已支持用户修改。
- 远程连接沿用原有 URL 校验，不新增任意反向代理路径支持。
- 隐私：请求脱敏引擎、本地模型、自定义规则、记录、预览、统计和分享服务。
- 可观测性：OTLP/HTTP、Langfuse、导出范围和请求正文采集。
- 同步与备份：全网关 WebDAV/S3/GitHub 同步、加密格式与导入导出。
  不能将已有 provider-scoped credential storage 冒充整机设置备份。
- 关于：更新检查、发行说明、自动更新偏好、签名下载/重启、实际配置目录打开和未确认官网。

后续实现这些能力需要各自的真实配置 owner、API、鉴权、失败语义和测试，
不能只把禁用属性去掉或直接复制 HTML 的内存模拟逻辑。

## 持久化与安全

- 主题与语言继续使用原有 `gateway-ui-theme` / `gateway-ui-locale`。
- `gateway-ui-display` 仅保存允许列表中的字号、字体 ID 和字重，不保存任何秘密。
  字体仅使用固定本地字体栈，未安装时由系统回退；不扫描、下载或执行字体资源。
- 不修改桌面历史日志字段的归属：日志策略仍由后端管理。
- 服务端页面不能直接调用原生 IPC。关闭窗口、开机启动和连接仍留在可信本地子视图。
- 子视图在管理密钥操作或设置搜索弹层打开时隐藏，避免覆盖父页面弹层。
- 管理密钥编辑移入 Radix Dialog；列表外框固定，数量增长只滚动列表。
  保留最后一项不可删除、16 项上限、当前密钥变更后注销、明文请求与自动隐藏规则。
- 设置搜索只索引静态标题、分类和关键词，不索引密钥、路径、账号名称或草稿。
  支持跨分类定位、上下键、Enter、Escape、清空和无结果反馈。
- 日志目录归当前 Gateway 后端，不使用浏览器或客户端文件系统冒充远程目录。

## 验证入口

定向单测覆盖六类导航、分段偏好持久化、字体允许列表、存储拒绝、跨类搜索、
不可用功能保护、原生隔离、密钥弹窗/会话/明文生命周期和日志 API。

```powershell
npm test --prefix apps/desktop -- --run src/features/settings src/features/shell/DesktopWindowChrome.test.tsx
npm test --prefix apps/desktop -- --run src/features/console/BrowserConsoleApp.shell.test.tsx
npm run typecheck --prefix apps/desktop
npm run test:effective-lines --prefix scripts
node scripts/effective-code-lines.mjs
git diff --check
```

最后一条 checker 与 `npm run check:effective-lines --prefix scripts` 使用相同 ratchet，
但不写 `scripts/artifacts/effective-code-lines.json`，用于保护工作树中已有的并行报告。

浏览器视觉验证必须使用真实产品构建及明确隔离的测试 API，不能用 R21 HTML 证明产品完成。
本轮构建与测试证据放在 `linshi/gateway-ui-implementation-20261102`，不连接日常实例或真实凭据。
前端构建不证明原生窗口、托盘、启动项或后端新能力已经人工验收；也不代表已生成 EXE。

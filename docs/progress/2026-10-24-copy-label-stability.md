# 密钥页复制成功反馈的布局稳定性

## 原因与最小修复

右上角 API 地址使用 `AccessWorkspacePrimitives.CopyButton`，成功后会将“复制”替换为
“已复制”；列表的 `AccessKeyCopyButton` 已使用勾图标和原文案，两者行为不一致。

共享地址/明文复制组件现在在成功后保持原有可见文案，仅切换勾图标及成功色，
约 2.4 秒后恢复。tooltip 与屏幕阅读器仍提供“已复制”反馈；失败提示保持不变。
没有扩大按钮、修改 CSS、改变复制内容或接触真实密钥、凭证、数据库和运行实例。

## 实际验证

- 先复现：新增回归用例在修复前 3 项失败、1 项通过，明确捕获中文、英文和自定义文案被替换。
- 修复后相关 6 个文件、29 项测试通过，TypeScript 类型检查通过。
- formatter 检查、有效行 checker 的 31 项测试、ratchet、scoped `git diff --check` 通过。
- 生产组件有效行 168 → 166，新测试 41 行，页头集成测试 40 → 44 行，文本均为 UTF-8 无 BOM。
- 隔离 Chromium 测量中文/英文、1600/900/540/390px 下复制前、成功态及恢复后的边界，
  复制按钮、地址、新建 Key 按钮和操作区的宽高与位置均相同，实际剪贴板内容也匹配地址。
  验证时使用 reduced-motion 并移出鼠标，排除现有 hover 上移和侧栏宽度过渡对测量的影响；
  初轮含这些既有动效的测量保留在 `browser-first-measurement.log`，未据此修改无关样式。

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-copy-label-20261024/`。
包含回归结果、定向检查、源码哈希/行数和浏览器尺寸记录，以及成功前后截图。
浏览器使用隔离 API 夹具；启动阶段未覆盖的非密钥接口 404 不代表整站接口验收通过。
新包使用官方 desktop devtest、Jobs 4 入口，完整性结果以 `package.log` 和构建凭据为准。
未执行付费上游、真实 PostgreSQL 端到端或原生窗口手测，未提交推送。

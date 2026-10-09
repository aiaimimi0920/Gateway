# 计划首行与多选下拉手测版

恢复会话 `01a10f2b-7e52-7823-b7bf-5498f4a1a294` 最后一条需求。
命名计划首行左侧为“计划 + 180px 名称输入框”，右侧为“子池类 / 账户类”
及带全选的下拉多选。下拉覆盖显示，不撑开表单，Escape 关闭并恢复按钮焦点。
范围变化保留正在编辑的策略；窄屏正常换行。旧单账户入口和三级策略编辑器不变。

子池全选保留原 pool 语义，覆盖未分类及未来新增账户；账户全选保存当前明确的
账户集合。前后端仅同步放开命名计划的多账户限制，保留同类型、去重、所属池和
128 个上限，不改执行及认证逻辑。删除“读取结果…”提示的上一轮修复也已包含。

## 手测入口

`C:/Users/Public/nas_home/AI/GameEditor/Neuro/release/Gateway/gateway-product-20261006-plan-picker-r3/gateway-ui.exe`

ZIP：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/release/Gateway/packages/Gateway-gateway-product-20261006-plan-picker-r3-windows-x64.zip`

SHA-256：`de1339d110c08bfcfd4fe8f1d844a3d7d0bf5f22d11ac40b168bc721f67c4c25`。
保留完整目录运行。这是 dirty source 本地候选版，未提交、推送、部署、覆盖旧包、
停止用户实例或修改生产数据。未刷新无关模型目录，不宣称正式发布。

## 验证

- 7 个前端测试文件共 24 项通过，TypeScript 类型检查通过。
- Rust 多账户合同 1 项通过：1/2/128 个账户有效，空、重复、跨池、混合类型和
  129 个目标被拒绝。
- 行数 checker 31 项、ratchet、Rust formatter、Neuro 规范契约通过。
- 官方 Windows build、package、UI artifacts、目录完整性和 ZIP 校验通过。
- 实际打包 Web UI + 隔离 SQLite 后端：多账户保存并重新打开，API 确认两个
  account scope 已持久化。宽屏、390px 窄屏、无子池空态截图已查看。
- 子池部分选择、全选、空选择禁用保存、Escape 和焦点恢复有组件测试覆盖。
- 未运行全量业务测试，未调用真实供应商，未为本轮启动原生 UI 实例。

有效行数：Dialog 95（不变），PlanHeader 67，Header tests 79，draft hook 53→55，
前端计划校验 87（不变），样式 58→96，Rust 计划校验 108（不变），Rust contract 41。
均低于 500，无例外或旧债扩张。逐文件复核保留文本转义、名称/范围边界、权限和
调用上限；下拉监听器关闭/卸载时移除，无新增请求、轮询或凭据日志。

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-plan-picker-20261006/`。
关键文件：`changed-source.json`、`steps.json`、`delivery-result.json`、
`final-verification.json`、`cleanup-result.json` 和三张 `plan-*.png` 截图。
checker 最初两次输出参数无效，改用合法仓库相对路径后通过。PowerShell HTTP JSON
中文显示异常未改动源码；浏览器及 UTF-8 文件正常。测试 helper 的 stdin 已关闭，
因此改用隔离后端 drain API 和精确父进程身份清理，没有停止其他实例。

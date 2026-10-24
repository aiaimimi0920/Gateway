# 站点健康多方案、独立认证与管理界面

本检查点接续 [第一阶段](2026-10-23-model-circuit-phase1.md)。本阶段是源码和开发侧验证，
不是新 EXE、部署、真实服务商验收或全量正式发布。

## 已接入

- 每服务商多检测方案，稳定 ID、名称、增加/删除和二次确认；最后一份方案删除后关闭监控，
  不静默启用旧的平铺策略。旧平铺配置继续可读，不升级存储版本。
- 多个本地凭证组可绑定同一方案并共享一次检测；也可只有一份全组共享方案。
  本地组与远端监控分类分开编辑，禁止重复绑定，未绑定组不继承别组结果。
- 自动识别有限公开模板；手选适配器严格只运行该模板。监控认证必须手选适配器，
  不把密钥广播给其他模板，不借用推理凭据。
- Bearer/API-key 认证复用已有敏感信息授权和 SecretPatch。普通配置保持 token 为 null，
  pending token 仅保留在短期内存中；授权失效、连接/服务商切换时清除。
- Keep/Replace/Clear 按 provider ID 与 profile ID 绑定；提交时才解析数组位置。
  origin、adapter、auth/header 或私网许可改变后不能携带旧 token；表单支持撤销 pending 操作。
- 各方案独立刷新、快照、覆盖情况和过期提示。关闭/切换时取消请求，旧响应不能污染新方案。
- 表单位于卡牌外的 Portal 弹窗，内部滚动，不扩大卡牌正反面。翻面恢复焦点使用
  `focus({ preventScroll: true })`，保留键盘可访问性而不滚动页面。

健康限制位于候选选择和软排序之前，恢复只解除自己的限制，不写人工 enabled 状态。
临时调用熔断仍独立存在，作用域和退避见第一阶段。

## 实际验证

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-health-v2-20261023`。

| 检查 | 实际结果 |
| --- | --- |
| 站点健康 Rust 合同（适配器、配置、多方案、分组 runtime、秘密身份） | 22/22 |
| 相邻配置/发送/智能路由 Rust 合同 | 116/116 |
| UI 定向测试（12 个文件，含翻面焦点回归） | 47/47 |
| TypeScript 类型检查 | 通过 |
| Tauri-target 前端生产模式 bundle，输出至 linshi | 通过；不是 EXE |
| effective-line checker | 31/31 |
| effective-line ratchet | 通过，受治理源码 >700 为 0 |
| 新健康组件/hooks/秘密草稿的 Prettier 检查 | 通过 |
| 相关 Rustfmt、`git diff --check` | 通过 |

后端记录为 `profiles-checks.json`、`profiles-final-rust.log`；UI 为 `ui-checks.json`、
`ui-tests.log`、`ui-production-build.log`、`ui-format-check.log`。格式化之后已重新完整执行
上述 47 项 UI 回归、类型检查和 bundle；最终结果以最近一次 UI 检查记录为准。

## 真实浏览器、离线接口夹具

运行的是仓库真实 Web 应用和原 e2e mock，不是另造页面。所有提交仅在夹具内存中：

- Plus 为 75%，Pro 为 99%，切换不串结果；Pro 手动刷新走精确 profile URL。
- 真实表单替换 Pro token 后自动提交；普通 document 不含 token，SecretPatch 指向
  `/providers/0/site_health/profiles/1/policy/auth/token`，包含既有 `x-secret-grant`。
- 初次连续翻面复现外层滚动偏移 68px，卡牌高度实际未变。加入 preventScroll 后连续 9 次翻面，
  本卡和邻卡均保持 `(x,y,width,height)` 不变；键盘焦点断言也由红转绿。
- 390×844 窄屏弹窗宽 358px、左侧 16px，无横向溢出，内容可内部滚动。
- Escape 关闭且焦点返回“站点健康”按钮。

证据：`browser-save.js`、`browser-final-r3.log`、`health-final-desktop.png`、
`health-final-mobile.png`。夹具未模拟的遥测存在 404 降级提示，不能把本测试描述为真实后端零错误。
本轮拥有的命名浏览器会话和 Web 开发服务器已关闭，未关闭其他浏览器或运行实例。

## 边界

New API/sub2api 官方源码调查见 [适配器证据](../plan/2026-10-23-site-health-adapter-evidence.md)。
其他四个模板只是兼容探测，不是官方 One API 通用保证；监控 401/403 是未知，不是模型 0%。

PostgreSQL 公开目录的补齐另记第三阶段，不能用上述 138 项后端测试冒充其新增逻辑的验证。
没有真实模型/付费上游请求，没有改 live `.ng`、凭据或数据库，没有提交、推送或生成新交付包。

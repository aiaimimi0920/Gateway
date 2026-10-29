# 添加服务商 UI 迭代检查点

本文件保留 UI 阶段的验证记录；后续运行框架结果见
[三适配器串行框架检查点](2026-10-25-provider-call-flow-framework.md)。

## 本轮交付范围

按已确认的 HTML 方案实现正式 React 页面和 provider-only 保存链。
三类适配器先分层显示并复用现有后端配置；逐模板真实适配、调用验收留给后续迭代。
没有新模板验收记录，`TEMPLATE_ADAPTATION_RECEIPTS` 为空，当前不将任何模板标蓝。

关键合同见 [provider-catalog-dialog.md](../provider-catalog-dialog.md)。
后续快速手测构建使用 4300 与 `.ng-devtest-4300` 默认隔离；正常 Release 仍保留原有默认值，
见 [devtest-build.md](../devtest-build.md)。本轮未生成或交付 EXE，也未提交、推送。

## 已通过的验证

| 检查 | 实际结果与边界 |
| --- | --- |
| 前端定向测试 | 9 个文件、53 项通过；覆盖新界面、provider-only 创建、旧账号兼容、真实配置字段、敏感字段目标绑定、自动保存和连接设置 |
| TypeScript | `npm --prefix apps/desktop run typecheck` 通过 |
| Rust 路由 | `cargo test --locked --lib routing::config --jobs 4 --no-default-features --features runtime-defaults`：90 项通过，含新空池/首凭据/删空回归 |
| Rust 关联 owner | 号池容量 5 项、可用性 1 项、console document 2 项通过；`credential_test_plan` 过滤器未匹配测试，不算额外覆盖 |
| 共享本地数据 | 独立 crate 常规测试 13 项通过：7 个单测、3 个路径、3 个启动配置兼容测试 |
| 手测编译分支 | `GATEWAY_DEVTEST_ISOLATED=1` 下共享 crate 的 devtest 专项通过，4300 与隔离目录分支可编译并验证 |
| Tauri 编译 | 手测开关下 `cargo check --locked --manifest-path apps/desktop/src-tauri/Cargo.toml --jobs 4` 通过；不等于桌面 EXE 实启 |
| 手测构建合同 | Python 6 项通过，包含三个 edition 的 DryRun、4300/数据目录回执、构建失败拒绝凭据、包类型隔离与哈希完整性；使用合成 fixture，不是本轮 EXE 包 |
| 前端产物编译 | 从实际 rsbuild 配置分别构建 Web/桌面前端，跳过发布钩子并输出到 linshi 独立目录；桌面产物本机地址为 `http://127.0.0.1:4300` |
| 浏览器布局/交互 | 实际产品组件配合合成 API mock，在 1440×900、1000×800、390×844 下通过 72 个断言 |
| 有效行门禁 | checker 自测 31 项及 ratchet 通过；本轮相关源码最高 495 有效行，没有新增超限文件或旧 >700 文件增长 |
| 文本与差异 | 本轮范围 UTF-8 无 BOM 校验、Rust 格式检查与 `git diff --check` 通过 |

新增界面测试、后端空池测试先验证预期失败，再实现并转绿。收尾检查还发现并修复了
WebDAV 密码缺用户名的前置校验，以及 `__proto__` 等合法模型名被误读为对象原型成员的问题，
两者均有红转绿的回归测试。

## 浏览器证据

测试仅访问本轮前端开发服务器 `http://127.0.0.1:18845/ui/`，没有访问当前 4200 Gateway、
真实凭据或付费上游。合成 API fixture 未覆盖遥测和自动 Secret Grant 端点，对应 404
属于 mock 缺项；不以此报告真实后端无错，也没有为了消除测试日志而放宽生产鉴权。

已验证分类切换保留草稿/页签、自定义适配器可选、无首账号/API Key、五页签切换时
外框和底栏几何稳定、无外层横向溢出、添加按钮可见、模板行高、键盘导航、Escape 和焦点恢复。
截图已人工目视核查，证据保留在：

```text
C:/Users/Public/nas_home/AI/GameEditor/linshi/provider-ui-browser-check.js
C:/Users/Public/nas_home/AI/GameEditor/linshi/provider-ui-official.png
C:/Users/Public/nas_home/AI/GameEditor/linshi/provider-ui-1440.png
C:/Users/Public/nas_home/AI/GameEditor/linshi/provider-ui-1000.png
C:/Users/Public/nas_home/AI/GameEditor/linshi/provider-ui-390.png
C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-provider-ui-validation/
```

源码体积抽查：`useConsoleController.ts` 为 493 → 495，`providerCatalog.ts` 为 468 → 490，
新分拆模块均低于 500；未改有效行基线或豁免文件。保留原有未提交的卡牌布局/滚动加载等改动。
安全复核保持 secret-only 补丁、精确目标绑定、原有认证与地址校验、内部滚动和输入/集合上限；
不增加后台任务、进程或上游请求。打开面板、配置测试集、存储或站点健康不会立即执行这些任务。

## 尚未执行与下一阶段

- 未运行新 EXE、未绑定 4300、未制作手测包或正式发布包。
- 未做各官方/Web/custom 模板的新三适配器真实上游验收；当前蓝色完成数为 0。
- 未改写 Web 执行器或实现三个任意可组合的全新 runtime 插槽；API 自动识别仍沿现有账号发现链。
- 未跑全量回归、付费模型调用或正式发布安全审计，不把本地定向测试当作这些验收。
- 下一阶段一次选择一个模板，在隔离手测环境完成开发与证据记录，再登记并标蓝。

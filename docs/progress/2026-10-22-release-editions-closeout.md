# 三种发行版与管理界面开关交付

## 范围

承接会话 `01a162af-d08e-71f1-b145-633630385e6d`，完成 API、Web、Desktop
发行能力、共享启动开关和测试产物过滤。保留 r10 的端口、流式超时和候选切换修复；
不替换运行实例，不修改凭证或数据库，不提交或推送。

## 交付语义

- API 使用 `--no-default-features --features runtime-defaults`，不编译网页资源。
- Web 使用带 `management-ui` 的后端，但不包含桌面启动器。
- Desktop 包含同一带 UI 后端和桌面启动器。
- 有效管理界面能力为编译能力与 `enable_management_ui` 的 AND。
- 关闭 UI 不关闭管理 API，也不取消推理 API 的鉴权。
- Desktop 关闭 UI 时不创建 WebView，Local 模式启动后端；Existing 模式报错退出，
  不接管远程或偷偷创建本地服务。
- 发布包排除测试目录、测试副本、缓存和截图产物，仓库测试保留。

## 本轮证据

- API 与带 UI 两种编译的 `http::management_ui` 定向测试均通过。
- 共享启动配置测试 3 项通过；桌面配置传递和稳定端口测试 2 项通过。
- 版本合同 4 项、Windows 图标合同 5 项、桌面发布合同、嵌套 worker 打包合同通过。
- 有效行数检查器 31 项通过，ratchet 通过；不重置历史基线。
- 真正的 API Release 编译通过；为控制内存使用，仅对本次构建覆盖
  `profile.release.package.gateway.codegen-units=16`。
- Web/Desktop 复用 r10 未改变的带 UI 后端及桌面启动器，不重复执行前端构建。
- 三版各自的开关 true/false 共六组隔离启动验证通过：健康接口正常；UI/SPA
  路由按开关返回 200 或 404；管理 bootstrap 保留；未认证模型查询仍为 401。
- Desktop false 实际通过启动器启动后端，并检查没有 WebView 子进程。
- Desktop false + Existing 实际拒绝启动，不影响远程服务。

预打包运行证据位于 `linshi/gateway-editions-20261022/runtime-prepackage/result.json`。
正式包运行证据使用同目录下独立的 `runtime-packaged/result.json`，仅在实际通过后生成。
Desktop true 的隔离矩阵验证后端 UI 路由，不额外启动与正在使用的 4200 竞争的桌面窗口。

## 操作入口

详见 `docs/release-editions.md`。三版统一以 `gateway-editions-20261022-<edition>-r11`
为不可覆盖的交付目录名。每个包包含 manifest、校验和及区分新编译/复用产物的 provenance。
测试使用新的临时数据目录和回环随机端口，没有调用真实付费上游。
按当前快速开发要求，本轮不执行全面 provider 回归；不将局部验证表述为全量验收。

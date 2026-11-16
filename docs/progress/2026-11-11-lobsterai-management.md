# LobsterAI 本地管理、授权与模型目录

本批已完成管理/UI/目录的本地切片验证。LobsterAI 的公共推理、Server 和真实账号验收仍未完成，Magpie 总范围仍有 **67 项未完整关闭**，不扣减工作项分母。

## 改了哪些内容

- 控制台新增 LobsterAI 授权入口，贯通创建会话、本机浏览器 callback、交换个人授权、Ready、显式读取模型目录并保存。取消、到期和 shutdown 会释放 listener/交换任务；已接受的保存脱离原 HTTP 请求，使用有界预算和可查询回执。
- 管理操作要求管理身份、secret consent、owner 和原配置 revision。会话/并发任务均有容量上限，HTTP 请求体限制为 16 KiB，响应 no-store，视图不返回 access/refresh token。保存复用 SecretPatch 和配置 CAS，保留其他凭据、稳定账号 ID 与人工元数据。
- 重授权校验相同个人身份和 installation。新增 optional reauthorizationId，在上游返回相同 refresh token 时建立独立续期链；正常 refresh 继承链标识。旧无此字段的材料与其他厂商的 family ID 保持原合同，旧 Sending/Published 记录不删除，不修改存储版本。
- 持久化 typed 模型目录，未知能力继续为 null，拒绝无效/重复/明确 inaccessible 的目录。账号编辑可单独读目录、应用到表单，保留手动模型，再经正常保存发布；未保存编辑会禁用目录读取。
- 复用现有 UI 组件和受控授权流程，增加新建/重授权面板、目录工具及 oauth 分类。前端和 Tauri 只接受固定 Lobster portal hash 路由、state 与规范 127.0.0.1 callback；没有全局放开带 fragment 的外部链接。
- 当前模板仍是无 preset 的 openai_compatible + lobsterai_session。公共推理尚未实现，在 plan、buffered、streaming、passthrough、native Codex 及通用探测/发现入口明确返回 lobsterai_inference_unavailable，避免误入通用协议并外发会话凭据。

## 本地验证

| 层级 | 实际结果 | 证据边界 |
| --- | --- | --- |
| Rust 联合定向 | 230 passed / 0 failed / 4 ignored；名称含 lobsterai 的 45 项通过 | 合成上游、本地 callback 与隔离持久化；4 个 Redis 用例需显式 disposable Redis URL |
| UI 定向 | 7 files / 56 tests passed | 授权/API/目录、GlobalDialogs 接线、模型表单及相邻授权链接/建池合同 |
| TypeScript | typecheck-final 通过 | 含本批最终测试源码 |
| 前端构建 | 真实 src/main.tsx 隔离构建通过 | 输出在 linshi/safe-web，未调用发布 hook 或覆盖运行包；1012 个输入文件复核无变化 |
| 桌面 launcher | 10 passed / 0 failed | URL allowlist、单参数无 shell 启动、进程退出/超时/取消测试；没有执行真实站点登录 |
| 工程检查 | rustfmt --check、checker 31 项、effective-line ratchet、scoped git diff --check 通过 | 未修改 baseline/exception，未升级依赖；本批最高 500 有效行 |

Rust 命令：

```text
cargo test --locked --lib -- lobsterai token_refresh:: console::refresh_grants:: console::secrets:: routing::config::refresh_state_tests:: --test-threads=2
cargo test --locked --manifest-path apps/desktop/src-tauri/Cargo.toml --lib connection::authorization_browser -- --test-threads=2
```

Gateway Cargo 本轮最终编译 11m37s，测试 51.63s，包装命令总耗时 749.50s；桌面测试命令总耗时 18.38s。没有据此推算冷构建、打包或全量测试耗时。

历史失败回执保留：cargo-r1 因 GatewayError 字段调用写法编译失败；cargo-r2 为 227 passed / 2 failed / 4 ignored，两项新测试夹具分别在请求后尝试 Arc::get_mut、构造 payload 缺 adapter。已修正责任层，cargo-r3 的最终 230/0/4 包含这些用例及新增真实推理入口拒绝测试。

## 浏览器验证

用临时 Edge profile 加载真实入口隔离构建，管理 HTTP 使用合成 fixture。实际走过添加账号→选择 LobsterAI→启动授权→Ready→显式保存→列表出现 1 个账号/1 个模型，保存请求恰为 1 次；随后编辑账号、读取目录并应用到表单。

编辑中的未保存修改会禁用目录读取。Escape 关闭编辑对话框后，焦点返回原编辑按钮；Enter 可再次打开。再次进入重授权 Ready 后，Escape 关闭会发送 cancel，账号数量保持 1，import 总数仍为 1。430px 窄屏下 dialog clientWidth/scrollWidth 都为 396，无横向溢出；宽屏、等待、Ready、目录和重授权截图均保留。

浏览器 Ready 由合成服务返回，不是实际厂商 callback；本地 callback→exchange→保存由 Rust 路由测试覆盖。浏览器有 8 条 console 404，来自两轮加载时各 4 个未模拟的 telemetry 接口，页面正确显示统计不可用；不能宣称零 console 错误。CLI 曾因尝试点击禁用目录按钮以及 PowerShell 空字符串/引号传参失败，随后按实际表单状态与键盘操作完成验证；没有据此改变产品语义。

## 审查与交付边界

已复核管理鉴权、秘密不进入视图、固定上游/回调、输入大小、任务容量、取消/关闭与 accepted save 所有权、CAS、重授权 lineage 兼容及目录手动模型保留。调用期间不持有同步锁进行网络等待。管理保存的超时/未知结果保留未确认状态，不能盲目重复导入。

本批最高有效行文件为 internal_account_groups/tests.rs，499→500，仅补新增 optional 目录字段；ConsoleGlobalDialogs.tsx 为 433→455。没有本批文件超过 500，有效行前后明细保留在回执目录。Rust 最终测试后全部 scope 源码哈希未变化；UI 构建输入最终哈希未变化。工作区有大量继承改动，wholeWorkingTreeProvenance=false，不以本批检查证明整棵工作树。

尚未验证：真实账号及上游接入资格、公共推理 401 信号与刷新衔接、Chat SSE/工具/usage/结算、Server/Redis、原生整包、部署和跨应用验收。本批没有读取真实 .ng、登录真实账号、付费生成、提交/推送、出包、部署或重启现有实例。临时浏览器与 fixture 服务均已关闭。

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-lobsterai-management-20261111`。主要回执为 `final-validation.json`、`browser-validation.json`、`browser-actions.json`、`source-r3.json`、`source-final.json`、`effective-lines-before-after.json`、`scoped.diff` 与对应测试 `.json/.log`。

# Google Web 专属网页会话 JSON 导入

## 实现范围

在此前独立模板及网页登录控件的基础上，为 `gemini-web`、`gemini-canvas`、
`aistudio-web` 的“添加账号”增加“网页登录／网页会话 JSON”选择。支持粘贴、选择
UTF-8 文件，再显式点击导入；保留已有网页登录及 ChatGPT Web 原有 JSON 格式。
三个 Google 模板不合并，模板蓝色登记也不因本轮合成测试而扩大。

格式、完整占位示例及使用方法见 [Google Web JSON 导入](../google-web-json-import.md)。
这不是从浏览器自动导出会话状态的功能，也不接受完整 storageState、origins 或本地路径。

## 保存与安全边界

- 复用现有管理认证入口；JSON 分支要求 management token、独立 secret grant、当前
  revision、匹配的服务商 preset 和 Google 官方 origin，拒绝默默改为网页登录。
- 原始材料最多 64 KiB，请求信封最多 512 KiB，Cookie 最多 64 条、约 32 KiB；
  严格校验域、路径、有效期、签名 Cookie、未知字段、注入字符和运行时单 Cookie 预算。
- Canvas／AI Studio 仅由正常 object-storage owner 保存最小 Cookie 对象，响应和普通
  配置保留对象引用。Gemini Web 通过已有 secret edits／patches 保存敏感字段。
- 输入仅在组件临时内存持有；提交、卸载、切换目标／方式、禁用或撤销授权时清空，
  防止迟到文件读取恢复旧材料。不写 localStorage，不向错误回显输入或 Cookie。
- 一次性响应不存入浏览器 session manager、不轮询；授权／修订 epoch、目标、family
  和生成草稿归属再次校验，拒绝旧请求、跨池结果、空结果及非终态响应。
- 草稿沿现有合并、模型路由及 revision/CAS 保存链，不丢失池内旧账号，不自动测试。
  默认模型只作为 `model_selection.manual`，不制造 fetched catalog 或 verified models。
- 本轮源码审查发现现有 Gemini Web HTTP bootstrap 不应用 `authUser` 索引。因此仅新
  JSON 格式明确要求 Web 的 `authUser` 为 `"0"`，非零值拒绝，不静默调用默认账号。
  没有借此扩写旧执行器；Canvas／AI Studio 继续使用已有支持数字索引的 HTTP owner。

## 验证记录

临时证据目录：

```text
C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-web-json-import-20261101
```

- 红灯基线：新字段组件尚未实现时解析 import 失败；JSON 响应保护测试首次运行有
  3 项失败，分别暴露跨池草稿、非终态轮询及空草稿被接受。对应问题已修复。
- 前端 TypeScript：通过；14 个定向测试文件、105 项通过，覆盖三家实际添加弹窗、
  API header/body、JSON 授权／生命周期、已有账号保留和保存、原网页登录、模板蓝标。
- Rsbuild 桌面 UI bundle：通过。产物只写证据目录 `ui-build`，未覆盖正常 dist。
- AI Studio Cookie 导入／HTTP 材料与 wire 合同：7 项通过，均为合成材料。
- Rust 解析器最终单测：5 项通过，包含三家草稿、作用域／有效期／大小、Web 非默认
  账号拒绝、wire 大小、未知字段及非敏感配置边界。
- Rust 管理 HTTP 合同首次编译暴露测试引用 crate-private `document()` 的错误，已改为
  正常管理 GET 读取配置，不扩大生产可见性。最终 4 项通过：管理认证与 secret grant、
  revision／目标／格式校验、三家 no-store 草稿与真实隔离对象写读、官方 origin 和
  请求信封上限；同时证明结果不进可轮询会话、草稿未提前改写 active 配置。
  最终 Rust 命令均为 `--locked --jobs 2 --no-default-features --features runtime-defaults`，
  串行执行 `--lib console::google_web_json` 和 `--test console_google_web_json_contract`，
  不将首次编译失败记为通过。原有编译 warnings 保留，没有无关修复。
- 行数 checker 自测：31 项通过；仓库 ratchet、`cargo fmt --all -- --check`、
  `git diff --check`：通过。没有修改 baseline、阈值或行数例外。

关键 effective lines：`useConsoleController.ts` 495 → 496；
`useGeminiManualAddSession.ts` 220 → 246；`ConsoleGlobalDialogs.tsx` 273 → 278；
`internal_console.rs` 352 → 387；路由挂载文件 271 → 272。
新解析 owner 190、Cookie owner 146、Rust 单测 171、HTTP 合同 202；
新字段组件 55。所有本轮源码文件不超过 500，有效行数和 UTF-8 无 BOM 检查记录于
`lines-before.json`／`lines-after.json`。

最终结果以 `frontend-final.json`、`frontend-typecheck.log`、`frontend-build.log`、
`node-material-contracts.log`、`rust-unit-final.log`、`rust-http-final.log`、
`checker-tests.log`、`ratchet.log` 和 `fmt-final.log` 为准。

## 未执行与交付边界

本轮是离线导入功能开发和定向合成验证，不是新 JSON 凭据的真实 Google 认证验收。
没有打开浏览器、重新登录、导入真实 Cookie、访问 Google、生成媒体或付费测试。
没有改动运行实例、现有账号、凭据、数据库或路由修订；不升级 storage version。
没有编译／替换桌面 EXE，没有提交、推送或发布。当前用户已打开的旧 EXE 不含本轮修改。

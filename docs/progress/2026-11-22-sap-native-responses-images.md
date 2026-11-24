# SAP AI Core：原生无状态 Responses 内嵌图片

## 接入范围

已在 `/v1/responses` 对显式配置的原生 SAP 部署接入用户消息中的内嵌 PNG／JPEG。新开关 `sap_native_responses_image_input` 默认关闭，只有同时开启 `sap_native_responses` 时才可用。它不复用 Chat 视觉声明，也不会根据部署 ID／公开别名推断能力。Local 凭据不能覆盖；Hosted 仅在与账号层的部署地址、资源组、API 版本保持相同时继承。控制台改动绑定或关闭原生 Responses 时会清除图片勾选。

沿用 SAP 图片准入的全请求预算（10 张、累计 4 MiB Base64 文本）、编码校验和 `detail` 约束。原生 wire 保持图片、文本及顺序，强制 `store:false`；远程地址、文件 ID、非法图片字段、非用户图片不进入 OAuth。没有添加 upstream 自动重放、图片下载、图片输出或其他媒体支持。见 [SAP 部署合同](../providers/sap-aicore-chat-contract-2026-11-10.md#显式开启原生-responses-内嵌图片)。

## 验证与边界

- 桌面表单红测复现缺少选项；实现后 SAP 表单回归 **10/10**、TypeScript 类型检查通过。测试验证默认关闭、原生 Responses 控制依赖、绑定变更复位和提交时只写池配置。
- Rust 公共 HTTP 红测在初始化配置时得到 `Invalid SAP AI Core service key, deployment or unsupported request.`，与旧版本没有新图片开关一致。修复后 SAP 定向 **56 passed／0 failed／3 ignored**，独立 `line-sap-aicore-chat` 的 lib／bins 检查通过。新增覆盖 JSON／SSE 原生线协议、各 5 tokens 的归属审计、无图片数据回显、OAuth 前拒绝以及 Local／Hosted 配置所有权。共享 Responses 回归 **60/60**，有效行 checker **31/31**、ratchet、定向 Rust 格式、UTF-8 无 BOM 和 `git diff --check` 通过。
- 未使用真实 SAP 租户或额度。原生账号提交／重启读回、包内双后端成功推理仍未验。管理 API 和源码回执不能替代上述验收。

## 复跑

```powershell
cargo test --locked --offline --lib --no-default-features --features runtime-defaults --config profile.test.package.gateway.debug=0 -j 2 sap_aicore -- --test-threads=1
cargo check --locked --offline --lib --bins --no-default-features --features line-sap-aicore-chat -j 2
npm.cmd run test --prefix apps/desktop -- --run src/features/console/SapAiCoreCatalogTemplate.test.tsx
npm.cmd run typecheck --prefix apps/desktop
```

常规 Rust 用例使用隔离 Local 测试存储与不会转发的 loopback peer；默认忽略需要专用 PostgreSQL／Redis 的 Hosted 用例。Hosted 图片配置归属在普通合并回归中检查，不等同于打包双后端或真实 SAP 生成。

## 桌面手测包

- 本地交付 `desktop / devtest`：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/release/Gateway/gateway-devtest-20261122-sap-native-responses-images-v1`，不是正式发布。manifest 与构建凭据的 `sourceTreeFingerprint` 均为 `94a404f73ff73e4fab64c3c640e14399198d872f0ff0f7811342dc628bd6baa5`，未包含便携 `.ng`。
- staging 与最终目录均通过包校验。`gateway.exe` SHA-256 为 `5404ead91cc729e0ab7d0dafe7e108f857be411f0f26ad4462ab41ec630a6a7b`，`gateway-ui.exe` SHA-256 为 `373390870116277823f67827c297a503f7302258339b89c20f921db732b25b38`。
- 包内后端随机 loopback 端口和隔离数据目录：通过 SQLite 就绪、耗尽拒绝、退款、Key 轮换、重启、正常 drain；另用合成 Service Key 通过管理 API 验证 CAS 保存、旧 revision 409、重启读回同一账号与原生图片开关，凭据显示 `configured=true` 且未回显。没有向 SAP 提交任何请求；这不是桌面窗口内的业务点击验收。

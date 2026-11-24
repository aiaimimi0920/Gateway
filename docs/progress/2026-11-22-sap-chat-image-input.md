# SAP AI Core：Chat 部署内嵌图片输入

## 范围

- 复用现有部署 URL、`client_credentials`、SAP sender 与 Chat JSON／SSE 校验。图片能力在部署级显式开启，仅支持 `2024-10-21`；旧配置继续走原文本路径。控制台新建服务商时不根据目录条目自动打开开关。
- 三个公共入口统一转 SAP Chat 用户图片块。PNG／JPEG data URL 有图片数与累计编码字节上限；不下载远程资源、不发送认证到新地址。保留文本／工具原有路径、预付结算、失败退款与不重放语义。图片不能走当前只接受有界文本的现金预付路径。
- 合同证据及运行限制见 [SAP Chat 合同](../providers/sap-aicore-chat-contract-2026-11-10.md#显式开启内嵌图片输入)。SAP 文档固定快照 `8fc8e1de` 的 SHA-256 为 `733a359449bb56837e1592dd4a9b5304fbf040acfd81ac7002b861add1a27f23`，SDK `3b791d54` 的 inference spec 为 `7afed2ffd044a9a1bdac857bcbd7b85e5c84200ab0545c4f7c323ab3256e741a`。

## 验收结果

- 红测证明旧实现拒绝图片配置；修复后 SAP 定向 **32/32**。六条成功链路分别验证三种公共请求各自的 JSON／SSE、准确的原生 Chat 图片块、终态、有凭据归属的用量审计及预付扣费；审计不含测试图片。拒绝路径验证 OAuth 与推理均零发送，余额不变。
- 前端 **34/34**，覆盖默认关闭、版本限制、部署变化不继承选择及保存显式配置；独立 `line-sap-aicore-chat`、TypeScript、定向 Rust 格式、有效行 checker **31/31**、ratchet、UTF-8 无 BOM 与 `git diff --check` 通过。
- 未连接真实 SAP 租户，未消耗额度；未验证真实部署的图片识别、原生桌面窗口实点或 Server Hosted 图片流程。

## 复跑

在 Gateway 根目录执行：

```powershell
cargo test --locked --offline --lib --no-default-features --features runtime-defaults --config profile.test.package.gateway.debug=0 -j 2 sap_aicore -- --test-threads=1
cargo check --locked --offline --lib --no-default-features --features line-sap-aicore-chat -j 2
npm run test --prefix apps/desktop -- --run --cache=false src/features/console/SapAiCoreCatalogTemplate.test.tsx src/features/console/useCredentialDialogEditor.sap.test.tsx src/features/console/ProviderCatalogDialog.test.tsx src/features/console/ConsoleGlobalDialogs.luma-agents.test.tsx
npm run typecheck --prefix apps/desktop
```

## 桌面手测包

- 非正式 Release：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/release/Gateway/gateway-devtest-20261122-sap-chat-image-input-v1`。源码指纹 `e1cb5991db987f8f38b7cf58ca3ac5e381e2b3b369d1a843302324472f46375f` 对应构建时工作树；此后仅补本段回执。
- 完整清单及双 EXE 校验通过：`gateway.exe` 为 `0886806103b1b28d96a262bbad78f57f6762ae330ae257829d9754574db50430`；`gateway-ui.exe` 为 `259e82e95e3d0cdf6e2717c12cb45452d60435667e74691e742beaf6ef2045fc`。
- 包内后端通过独立端口和数据目录的 SQLite 启动、重启、额度拒绝、失败退款、Key 轮换及正常退出；零模型调用，未连接 PostgreSQL／Redis。图片 JSON／SSE 成功来自源码合成回归，未冒充包内原生业务实点或真实 SAP 验收。

# ChatGPT OAuth 凭证保存修复

## 范围与修复

- OAuth 新增账号不再同时显示普通 API Key 表单和普通保存入口；编辑及非 OAuth 新增流程保持不变。
- 保存权限缺失或被服务端判定失效时，通过既有管理认证恢复权限，并在同一会话内继续用户明确发起的操作；失效响应最多自动恢复一次。
- 面板内显示权限恢复、保存进度及失败原因。取消、切换 Provider、卸载和服务端确认会话失效时不重放旧操作。
- 保留导入瞬时失败后的授权材料和提交成功但响应丢失时的轮询恢复。
- 没有读取或修改用户真实凭证，没有绕过管理认证。未使用用户真实账号重新执行 OAuth 授权，因此不将专项 mock 测试视作真实账号验收。

## 验证

- 先前续权回归已复现失败；修复后 OAuth 面板、CredentialDialog、BrowserConsoleApp.codex-actions 共 29 项通过。
- TypeScript 检查、有效行数 checker 31 项及 ratchet、git diff --check 通过。
- 前端全量测试：134 个文件通过、11 个文件失败；713 项通过、12 项失败，另有套件加载错误。未将全量检查报告为通过，未为消除失败而改动无关业务。
- 账号空状态测试期待没有权益组，实际 DOM 存在 default 分组；其余失败保留在完整日志中，未完成逐项基线归因。
- 正式构建的生产依赖审计、Web 构建、优化版 Rust 后端及 Tauri 构建通过。
- 发布包 1528 个文件 SHA-256 校验通过，隔离本地存储 smoke 通过；包中没有 .ng。

## 交付

- 包：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/release/Gateway/gateway-oauth-save-20261022-r1`
- 源码指纹：`50cf93add26b496b713796e6b4faa3d6ccbee56e6ca942eee6c0f86482b21d6f`
- gateway.exe：`96b3859276f0379a63381d335600c78d9fa0480cdcef6158bbbd60d437972e5c`
- gateway-ui.exe：`16171f2a9bf19e56e6b8a26affc9a32e0d99cc3aeb30e2f1cb4c685745f815e7`
- 构建及测试证据：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-oauth-save-20261022/`
- 此记录在打包后新增，不属于上述构建源码指纹。旧包及用户正在运行的实例未覆盖或停止。

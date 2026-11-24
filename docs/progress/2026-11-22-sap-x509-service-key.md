# SAP AI Core：X.509 Service Key

## 实现

- 按 [SAP Chat 合同](../providers/sap-aicore-chat-contract-2026-11-10.md#x509-service-key) 接入证书模式。客户端证书和 RSA 私钥只用于直连、校验主机名的 XSUAA Token 请求。部署目录、专属探测和推理仍经原有 Bearer 路径。原认证模式、绑定、缓存期限、不重放与 SecretPatch/CAS 不变；无数据迁移。
- 控制台使用原有 `Service Key JSON` 位置，新增对证书字段和域名形状的检查，不在路由草稿中保存私钥。目录查询用的凭据仍只在该次请求内使用。
- mTLS 夹具在内存中生成 CA、TLS 服务端证书和两套客户端证书，不托管测试私钥，也没有真实租户或公网转发。`btls` 和 `hyper` 只增加现有锁定版本的 dev 直连依赖，不新增 Rust 包或升级版本。

## 验证

- 新增证书导入红测在原实现失败。前端定向 35/35、TypeScript、独立 `line-sap-aicore-chat` lib/bins、有效行 checker 31/31、ratchet、定向格式和 scoped `git diff --check` 已通过。
- 本地 SAP 定向回归 **37/37**（含新增 X.509 5 项）。双向 TLS 服务端要求有效客户端证书；验证公共 Chat JSON／SSE、有凭据归属的真实合成用量、预付扣费、短期 Token 复用、轮换重取和只读部署目录。错误域名和 307 均不发送推理、不自动重放，失败后预付余额保留。
- 未访问真实 SAP 租户、未消耗真实额度、未生成新手测包；原生窗口和 Server Hosted／双实例未验。

## 复跑

```powershell
cargo test --locked --offline --lib --no-default-features --features runtime-defaults --config profile.test.package.gateway.debug=0 -j 2 sap_aicore -- --test-threads=1
cargo check --locked --offline --lib --bins --no-default-features --features line-sap-aicore-chat -j 2
npm run test --prefix apps/desktop -- --run --cache=false src/features/console/SapAiCoreCatalogTemplate.test.tsx src/features/console/useCredentialDialogEditor.sap.test.tsx src/features/console/ProviderCatalogDialog.test.tsx src/features/console/ConsoleGlobalDialogs.luma-agents.test.tsx
npm run typecheck --prefix apps/desktop
```

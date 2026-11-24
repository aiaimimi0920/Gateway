# SAP AI Core：o3-mini Chat 推理参数

## 实现

- 新建 SAP 池时，在手填 o3-mini 部署地址、资源组、`2024-12-01-preview` 后，可以显式启用推理参数。更换资源组、版本或指向其他部署时，不把原声明应用到新目标。Local 路由配置仅允许池级声明；普通 Chat 池默认行为不变，不迁移凭据或数据。
- 公共 Chat 的 `reasoning_effort` 接受 `low`、`medium`、`high`。将唯一的正整数长度限制转换为 `max_completion_tokens`；拒绝冲突、非法上限及显式不兼容的采样参数，而不是静默降级。数据面不发送本地开关或部署别名。
- Messages／Responses 的文本请求仍走 Chat；各自的长度限制有转换用例，原生推理、推理历史或摘要不在本切片。认证、域名、证书、超时、禁跳转和推理不重放的边界不变。
- 上游依据与使用方式见 [SAP Chat 部署合同](../providers/sap-aicore-chat-contract-2026-11-10.md#显式开启-o3-mini-chat-推理参数)。

## 验证

- 改动前 3 项 Rust 红测在 o3-mini 配置处失败，前端用例因缺少开关失败。改动后 **44 passed / 0 failed / 1 ignored**；新增 4 项均通过，其中公共 Chat 经真实 Gateway Local 路由命中只在 loopback 的模拟 SAP：JSON 与 SSE 各一次，取得一次合成 Token，实际 `reasoning_effort` 及上限正确，无重放；两条完成审计各 5 tokens，预付消息从 2 减到 0。未开启池、非法 effort、冲突／无效上限和不兼容采样控制零 OAuth、零生成，保留预付次数。
- 前端 SAP **16 passed**，TypeScript、`line-sap-aicore-chat` 独立 lib／bins 检查、定向 Rust 格式、31 项有效行 checker、ratchet、55 项 line manifests、UTF-8 无 BOM 及 `git diff --check` 通过。
- 忽略的用例是需启动隔离 PostgreSQL／Redis 的 Hosted X.509 用例，本轮没有重跑。新推理参数未打入 EXE、未做原生窗口／Hosted 专项，也未调用真实 SAP 部署或消耗额度；旧 Azure EU20 包不含此变更。

```powershell
cargo test --locked --offline --lib --no-default-features --features runtime-defaults --config profile.test.package.gateway.debug=0 -j 2 sap_aicore -- --test-threads=1
cargo check --locked --offline --lib --bins --no-default-features --features line-sap-aicore-chat -j 2
npm --prefix apps/desktop run test -- --run SapAiCoreCatalogTemplate useCredentialDialogEditor.sap --cache=false
npm --prefix apps/desktop run typecheck
```

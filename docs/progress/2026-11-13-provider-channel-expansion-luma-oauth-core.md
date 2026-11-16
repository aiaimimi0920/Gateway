# Luma MCP Rust授权底座

日期：2026-11-13。承接已成功的自有客户端授权与26项MCP工具目录调查；本批不重复真实登录或调用上游。

## 实施范围

新增 `src/protocol/luma_oauth.rs` 及职责分离的 authorization/material/transport 模块，注册到 protocol 模块。输入是操作者明确许可的已登记public client与精确redirect，不硬编码调查client、不自动动态注册。

- 固定issuer/authorize/resource为agents.lumalabs.ai；token为公开元数据确认的auth.lumalabs.ai/oauth2/token，不用issuer拼接推断，不放宽DIM/Grok的产品边界。
- S256、系统安全随机state/verifier、openid、显式资源参数、20分钟有限授权生命周期；仅规范数值127.0.0.1、显式非零端口和/callback。
- 回调核对精确redirect、重复参数、常量时间state、适用issuer、code/error互斥；拒绝错误前不泄漏callback或供应商文本。
- validate_callback不消费Authorization；未来listener必须先验证再取走授权。into_code_grant消耗Authorization，CodeGrant非Clone且exchange消耗self，失败/取消不返还授权码供重放。
- exchange单次send、固定token目的地、请求resource/PKCE/client/redirect绑定；禁跳转和client默认headers。调用方必须提供受控专用OAuth HTTP client，不能将默认header隔离扩大宣称为任意cookie jar/proxy/retry设置隔离。
- 一份最多15秒预算覆盖发送和读取，且不超过原授权剩余期限；已解码正文最多64KiB。没有producer、队列、后台重试或持久状态。
- token使用类型化解析，拒绝重复关键字段、错误类型/错误Bearer/非法秘密形状；expires_in有效时以发送时刻计算绝对期限，缺失保持None，不从未验证JWT猜期限/身份/团队。id_token与refresh_token不采纳，材料不实现Serialize/Clone，Debug脱敏。

这不是正常管理登录、保存、刷新或MCP执行链。当前没有新增模板、管理API或按钮；正常SecretPatch/Grant/revision/CAS、身份隔离、模型发现等仍须后续接线，不能把底座当成完整渠道。

## 验证与证据

目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-luma-oauth-core-20261113`。

- red.log：测试先加入，缺少新的Authorization/OAuthError/RESOURCE接口导致E0432，exit101。**这是新API不存在的编译红测，不是已执行测试或旧行为缺陷复现**。
- green.log：14 passed / 0 failed / 0 ignored，包含真实回环HTTP测试；测试编译25m05s（含依赖编译），测试运行0.42s，不视为纯热增量性能。
- production-check.log：非测试生产lib编译exit0，2m01s；保留113项warning，日志未指向本批Luma模块，不顺带修复无关warning。
- checker.log / ratchet.log：检查器31项及有效行增量检查通过，未改policy/baseline/例外。
- source-hashes.json冻结本批8个源码文件及1个复用未改的HTTP fixture；测试和生产编译完成后9项逐一核对一致，UTF-8无BOM验证通过，不将他人working tree修改算成本批。
- official rustfmt scoped check、仓库git diff --check及新增未跟踪文件的no-index空白检查通过。protocol/mod.rs本轮只增加luma_oauth一行，其余历史dirty模块登记保留。

测试复用既有Accio有界loopback HTTP fixture（该fixture未修改），不另建测试框架或生产目的地后门。覆盖授权参数/PKCE熵、非法redirect/client、callback重复/错state/错issuer/拒绝、期限；真实回环验证六个form字段及默认凭据header隔离、成功材料、未知期限、坏/重复token字段、重定向/HTTP拒绝不重试、解码预算、header/body deadline、取消释放及过期grant不发送。

定向命令：

```powershell
cargo test --locked --offline --lib --no-default-features --features runtime-defaults --config profile.test.package.gateway.debug=0 -j 4 luma_oauth -- --test-threads=2
cargo check --locked --offline --lib --no-default-features --features runtime-defaults -j 4
```

## 有效行与审查

protocol/mod.rs由118变119；新增入口48、authorization150、material76、transport78、授权测试111、材料测试36、传输测试194有效行，均无超限或例外。

独立只读审查未发现确定缺陷，并指出未来listener须遵守“先验证再消费”条件、HTTP client须受控。无新存储、命令/文件路径执行、可配置外部URL或账号合池。敏感材料不进入诊断文本，response分配在预算内，资源由future所有权与既有HTTP框架释放。

## 未执行和后续

未重新注册/登录、未读取真实凭据、未执行MCP工具、未生成/下载/读取余额，未改在线实例/数据库，未打包、提交或推送。无需用户再次授权本批本地工作。

下一步是正常管理会话及显式保存：复用原Grant/SecretPatch/revision/CAS，在Ready后明确import，保持稳定账号ID与私有材料受众；然后接只读模型发现。动态注册产品入口、OIDC身份/团队确认、refresh、生成/取消/费用与原生包仍未完成；Luma原定范围及P50/P60/P70不标完成。

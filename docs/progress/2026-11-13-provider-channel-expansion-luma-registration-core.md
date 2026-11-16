# Luma MCP 动态客户端注册协议底座

## 范围与官方合同

本批承接 MCP 空池及工具目录 UI，补充产品内动态注册所需的独立 public-client 协议代码，不借用调查客户端。2026-11-13 本轮匿名联网 GET `https://agents.lumalabs.ai/.well-known/oauth-authorization-server`，仍返回 issuer `https://agents.lumalabs.ai`、注册端点 `https://auth.lumalabs.ai/oauth2/register`、public client 认证方式 `none`、授权码及 S256。这里只读取公开元数据，没有执行注册 POST 或真实登录。

## 已实现

- `RegistrationRequest::new` 复用已有严格 loopback 校验，固定数值 127.0.0.1、非零端口和 `/callback`，不接受外部 redirect、查询参数或片段。
- 请求所有者不实现 Clone，注册消费 self；固定官方注册地址及 `Neuro Gateway` 名称，只申请 authorization_code／code、public-client `none`，不申请 refresh/offline_access，不注册多个 redirect。
- 无重试、重定向或后台任务；15 秒覆盖发送与正文读取，按实际解码字节限制 64 KiB，禁用客户端默认认证 headers。调用方仍须使用无 Cookie jar、无自定义重试的专用 HTTP client。
- 响应只接受 201 JSON；检查 client ID 形状和精确 redirect、grant、response、认证方式，拒绝 client secret。类型化反序列化拒绝重复身份字段；未知服务器字段不持久化。
- 成功值只保留公开 client ID 与 redirect；不保留 registration access token、秘密或服务器任意说明，错误不带上游正文。

## 验证记录

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-luma-registration-core-20261113`。

新增 7 项定向测试复用既有 loopback HTTP fixture，覆盖注册合同、请求 header 隔离、单次发送、跳转／HTTP 拒绝不重试、响应超限、headers/body deadline 及取消释放。`tests.log` 终态为 OAuth 相关 **31 passed / 0 failed / 0 ignored**，包含本批 7 项；构建报告 12m16s（包含共享构建目录等待，不能当纯编译耗时），测试运行 1.75s。源码与测试同批加入，没有单独保留测试先失败的红测回执，不虚称完成红绿循环。

有效行：原 OAuth 入口 54→56，新注册模块 116、测试 199；checker 31 项、有效行 ratchet 和官方 scoped rustfmt 已通过。独立只读审查未发现确定阻断问题。`scope-hashes.json` 记录本批三项源码；不改 policy／baseline／例外或其他服务商。

另行启动的非测试 `cargo check --locked --offline --lib --no-default-features --features runtime-defaults -j 4` 持续等待其他 Command Code 构建持有的共享目录锁，`production-check.log` 只有等待记录。本轮取消了自己的等待命令，未终止其他构建；非测试编译未完成，不将已通过的 test 构建冒充该检查通过。

## 尚未完成

动态注册还没有接入正常管理会话和 UI，不能宣称新用户已不需手填 Client ID。接线必须先持有精确 callback listener、获得显式注册批准，并持有操作回执；响应丢失、取消或超时不能当作“未注册”自动重放。已有客户端应复用，不重复注册。未知远端注册结果和跨进程恢复仍需明确产品处理。

本批不修改既有手工登记授权入口、账号或数据库；不注册真实客户端、不生成／下载／消费积分、不出 EXE、不提交或推送。refresh、实际模型发现、业务工具、远端 session DELETE、真实账号和完整包等剩余项继续保留。

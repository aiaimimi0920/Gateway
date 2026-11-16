# Kiro CLI 原生 Key 数据面参考

2026-11-04 只读联网核实。采用 [d-kuro/kirocc](https://github.com/d-kuro/kirocc/tree/7a2ba670286ca16ae89fe792ea935b3751179421)，固定 commit `7a2ba670286ca16ae89fe792ea935b3751179421`。
根 LICENSE 为 Apache-2.0，完整副本随本目录保留；固定 Git tree 中没有 NOTICE 文件。

采用的是公开 wire 信息与结构参考，不复制 Go 执行代码：

- `internal/kiroclient/client.go`：runtime 根 POST、bearer、`TokenType: API_KEY`、AWS JSON target/content-type，以及固定 CLI 2.10.0 SDK UA。
- `internal/kiroproto/types.go`、`internal/reqconv/build_payload.go`：`KIRO_CLI` origin、conversationState/current/history、可选工具/图片结构；不发送 IDE continuation 或 profileArn。
- Gateway 使用自己的配置编译、call-flow、安全准入、HTTP 发送及既有有界 EventStream decoder；不采用第三方重试/403 refresh、正文日志、模型别名表、token 估算、环境探测或 server-tool emulation。
- 原始参考快照位于 `C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-kiro-native-key-20261104/reference`。生产实现是 Rust 自行实现，不是该项目的官方客户端。

[官方认证说明](https://kiro.dev/docs/getting-started/authentication/#api-key-authentication-cli)证明 Key 的 CLI 用途和同用户 subscription credits 扣费，并没有保证 direct HTTP 契约。该渠道属于固定版本逆向兼容，不宣称官方通用模型 API、独立余额或账号资格。

首批仅准入参考实际默认的 `runtime.us-east-1.kiro.dev` 根；[官方防火墙表](https://kiro.dev/docs/privacy-and-security/firewalls/)确认该服务域名。未据域名表扩大到所有 Region；模型必须显式给定、保留原 ID，不从 IDE 别名或目录猜权益。暂不实现 Key 模型发现、reasoning 扩展或 profile 操作。

# CPA Go 传输隔离验证

## 当前结论

本轮完成实验性 Go 传输 worker、Gateway 通用 API 发送路径的显式接线及真实服务商抓包。**未完成可选 CPA 完整兼容模式，也未完成正式打包/自动生命周期集成。** 不应据此宣称 Gateway 对所有 CPA 行为完全透明。

固定参考 CPA：`20c9095cac089c6a8e40554debe4dd8a19d4c14f`；编译工具链 `go1.26.1 windows/amd64`。目标为用户授权的自定义服务商，未更改日常配置、未部署或提交。

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-cpa-profile-20261021`。凭证与私有配置留在隔离目录，不纳入仓库。

## 已通过的真实观测

- `reference-tls-01/tls/tls.json` 与 `go-worker-tls-01/tls/tls.json`：经过不终止 TLS 的 CONNECT 切片，解析后的 ClientHello 对象相等。记录长度 1518，cipher suites、扩展顺序、groups、signature algorithms、证书签名算法扩展、ALPN 和 key share 长度均相等。解析结果不含随机密钥材料；不是宣称所有加密字节或 TCP 时序相同。
- `go-worker-http-01`、`go-worker-http-bearer-01`：HTTP 切片确认目标 `/v1/messages?beta=true`，共享头的大小写和顺序一致；没有 `x-gateway-*` 泄漏。
- `go-worker-tls-01` 及上述两个 HTTP 样本均返回 HTTP 200，拼接 SSE 文本为 `CPA_OK`。
- 真实 CPA 的 CLI 模式参考样本返回 503，因此没有将其记作成功对话。其出站 HTTP/TLS 仍构成有效字段观测。

测试脚本此前以原始 SSE 字符串搜索 `CPA_OK`，会漏掉分成 `CPA_O`、`K` 的回复。已修复为按 `content_block_delta.delta.text` 拼接；历史原始 receipt 不覆盖，复核结果写入 `sse-reply-recheck.json`。

## 安全与回归

`go-worker-routing-02/receipt.json` 的五个本地合成凭证案例：

| 条件 | HTTP | 最终接收端请求数 |
| --- | --- | --- |
| CPA + 正确 worker | 200 | 1 |
| CPA + 错误 IPC token | 401 | 0 |
| CPA + 缺少 worker URL | 400 | 0 |
| CPA + 非 loopback worker URL | 400 | 0 |
| Auto + 无效 worker 配置 | 200 | 1，原通用传输 |

Go 测试 7 项通过，`go vet` 通过。覆盖逐跳/内部头剥离、正文保留、坏证书拒绝、目标准入、IPC 认证、取消释放、并发饱和、正文上限和超限响应中断。逐跳头红测试先复现请求/响应泄漏，修复后通过。

`cargo check --locked --lib`、Gateway 二进制构建通过。原 HTTP 窄修复相关 42 项测试通过；Go 接线后 CPA wire/profile/runtime 三组回归 9 项通过。额外 Rust IPC 单测 2 项通过，记录于 `go-worker-ipc-tests.log`。

行数 checker 测试 31 项与 ratchet 通过，`git diff --check` 通过。没有重生成 baseline。新增 Go 文件尚不在原 lexer 的语言范围内，分别只有 95、135、74、114 物理行，可证明都低于 150 有效行；该缺口显式保留，正式 Go 全仓扫描需要单独治理迁移。

已测试 worker SHA-256：`fcea2694442ff75faac7de1f9b2e984265320ae569af8c7057b42357c918e316`；与独立构建脚本的 provenance 一致。隔离启动器在 finally 中终止并等待自己的 Gateway/worker，不按进程名清理其他实例。

## 仍待完成

1. `cpa_full` 的配置/UI 与完整身份、会话、billing、currentDate、缓存和 beta 条件实现。旧 `cpa` 不自动启用这些正文修改。
2. 认证配置接线：底层 `ProviderAccountPayload.auth_mode=bearer` 的显式 Bearer 测试已通过，但真实管理 route config 尚未暴露/编译此字段。`go-worker-http-bearer-01` 只是一次设置尝试，抓包仍为 `X-Api-Key`，不能称为认证头已对齐。认证配置应独立修复，不能让调用风格暗改凭证协议。
3. 当前剩余 HTTP 差异：认证头、`Anthropic-Beta`、`X-Claude-Code-Session-Id`，以及身份/缓存正文导致的内容与长度差异。详见 `go-worker-comparison.json`。
4. 官方 Anthropic / ChatGPT 专用 transport、WebSocket、产品专用 adapter 不属于当前通用 worker 接线承诺；当前实现不能宣称它们已匹配或都通过了 worker。
5. 正式 exe 定位、worker 自动生命周期、发布 manifest/provenance 集成、全量 release gate、跨平台交付尚未完成。当前有独立构建脚本和隔离启动入口，不是发布版本。

运行参数及边界见 `workers/cpa-transport/README.md`。

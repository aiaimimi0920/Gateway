# 站点健康 Fake-IP 修复（2026-10-26）

## 现场与原因

调查开始时 4200 实例运行 `gateway-devtest-search-providers-4200-20261026-100740`；
用户续进时已手动切换到 `gateway-devtest-nvidia-catalog-4200-20261026-122100`，本任务不停止它。
8 份官方监控已启用，但系统 DNS 全部返回 `198.18.*`，旧 transport 在连接前拒绝。
NVIDIA 的真实公网 IP 直连也超时；本机已有环境/Windows 代理可正常取得官方 JSON。
因此只恢复 DNS 而继续强制直连不能解决本机的完整链路。

## 改了哪些内容

- 自定义站点维持 `no_proxy`、全地址校验及固定连接，未放行 Fake-IP 网段。
- 仅纯 Fake-IP DNS 回答可通过固定 `dns.google` / `8.8.8.8` 的有界 DoH 恢复。
  固定解析器复用可信代理；直连/bypass 固定 bootstrap。仅瞬态 I/O 最多再尝试一次，
  不重试 HTTP 拒绝、不安全或畸形答案；总恢复等待最多 8 秒并受外层 25 秒预算约束。
  A/AAAA 并行，校验 question、记录类型/IP family、公开地址、条目及响应大小。
  字面量 IP、本地名称、混合禁用地址不借此恢复；正常 TLS/SNI 不变。
- 官方客户端只接受固定 `HealthAdapter`，Gemini RPC 使用无参数固定入口。
  DNS 校验后复用已有代理及 bypass；代理负责其远端解析/出口，直连仍固定已校验 IP。
  不接受动态目标、不跟随重定向、不发送业务或监控密钥。
- 没有修改系统 DNS、代理、hosts、运行实例、凭据或自动账号测试策略。

生产文件有效行：`transport.rs` 219 → 203，`official.rs` 161 → 157，
`official/gemini.rs` 93 → 93；新 DNS 模块低于 200，新增测试模块均低于 150。

## 已验证与明确边界

- 旧实现真实联网回归：8/8 均在 HTTP 前失败，错误为
  `Site resolves to a prohibited network address`。
- 站点健康单元测试 24 passed / 1 ignored；代理合同 6 passed；健康/官方合同 9 passed。
- 修复后真实读取 NVIDIA、OpenRouter、Exa、ChatGPT、Jina、Tavily、Linkup，
  均为 `selected_template` 有值、probe `supported`、HTTP 200，不伪造模型百分比。
- You 已到达 HTTP 层，但官方状态页返回 403，独立 curl 响应头为
  `Cf-Mitigated: challenge`。最初要求八家都有健康观察的联网验收因此失败，
  **不能把它改称八家健康成功**；不尝试绕过挑战，不使用业务 Key 访问状态页。
- Fake-IP 专用回归只证明恢复到真实 HTTP 响应；业务验收另检查支持的观察和明确的上游阻塞。
  严格的全官方公开状态 smoke 保持独立、显式 opt-in，不纳入默认离线测试。
- 31 项有效行 checker、ratchet、scoped rustfmt、diff 与 UTF-8 无 BOM 检查通过。
  打包前再次检查源码；本次不做全量回归、付费生成测试或正式发布审计。

## 交付与后续证据

使用 desktop/devtest 正常构建入口生成新不可变目录，默认端口 4200，保留上一轮
NVIDIA 真实目录及账号模型池/路由功能；不覆盖旧包、不自动停止现用程序。
实际构建结果、manifest/hash/source 核验和隔离后台 worker 验收以以下目录内回执为准：

`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-health-fakeip-20261026`

隔离后台使用无账号配置，检查 8 个 HTTP 结果、7 家有效健康证据及 You 的外部 403，
并核实生成调用和账号测试均为零。原生桌面业务人工验收仍由用户切换新包后完成。

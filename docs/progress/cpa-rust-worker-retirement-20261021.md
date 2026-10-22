# CPA Rust 替代与 Go worker 退役验证

本记录接续 `cpa-hybrid-and-identity-20261021.md`，以实际新结果为准。
已完成隔离五轮对照、默认 Rust 构建和手测候选运行验收；不是正式发布验收。

## 修复与证据

- 完整模式 `cpa_claude_code` 与协议、认证独立；仅支持 custom origin 的
  Anthropic Messages API-key 路径，不开启官方 CCH/OAuth 生命周期。
- 会话标签由入口独立有界捕获，不转发为任意上游头，也不作为认证依据。
- 首轮 caller system 保留 top-level；后续按参考位置插入各自的 system 消息及缓存标记。
- explicit-cache/native 普通调用补齐 extended-cache beta；原生平台字段保留或按本机缺省，
  不再误用合成 MacOS/arm64 默认值。提供商显式配置/自定义认证的权威不变。
- 完整模式非流式调用不再强制 stream=true；Responses 正文键顺序按参考基础对象组织。
- Responses 转换器不再把协议完成误当 HTTP EOF。正常完成继续在原请求生命周期消费
  尾部，保留错误/取消的资源释放语义，不创建后台 drain 任务。

`linshi/gateway-cpa-rama-http-20261021/gateway_full_check.py` 使用真实 CPA 和 Gateway EXE、
本地合成凭证、实际 HTTP 服务端记录。七场景：plain、buffered、explicit-session、system、
explicit-cache、native、responses；各连续五轮，每轮把前轮 assistant 写回历史。
新增严格差异断言，并将 message_stop 延后发送，避免只测到提前缓冲的 EOF。

新 EXE `c548f4b8b12bdd9fa8d5e3455f66ea72446fd909c5388de54ea3a9c740f3e974`
对照全部通过：路径、头字段/顺序、正文语义/原始字节一致；两边均各五次 200、每场景一条连接。
仅排除隔离端口不同产生的 Host 差异。这是固定样例范围证据，不是所有 CPA 功能及网络调度
在任何服务商处都不可区分的保证。上一版 Responses 原始键顺序失败记录保留，不覆盖。

## 无 Go 的原生源码准备

`tools/prepare-cpa-bssl.py` 使用固定 `compat/objects.patch`，不再运行 objects.go。
三个生成对象表规范为 UTF-8/LF；与原 Go 生成树比较 1,602 个文件全部逐字节相同。
回执：`linshi/gateway-cpa-hybrid-20261021/no-go-source-parity.json`。
补丁 SHA-256：`d82bf52feaf7d93b0f6c869d21d2bca84b7a9a6b29888e3fab9852256edddd7a`。
归档 SHA、构建驱动与源码复用完整性继续校验；安全解压拒绝越界路径、链接与设备节点。

已通过：身份模块 21 项、TLS 模块 4 项（含两个 hybrid 组的 HRR/直接握手、畸形点及长度拒绝）、
Python 准备完整性/安全解压 2 项、行数检查器 31 项及 ratchet。

## 退役和删除状态

用户已明确回复“同意”删除 `workers/cpa-transport/` 和 `tools/build-cpa-transport.ps1`。
7 个文件已备份并逐一核验 SHA-256：
`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-go-worker-retired-20261021/manifest.json`。

此前执行工具拒绝复合及精确路径 PowerShell 删除，返回 `rejected: blocked by policy`。
没有换工具绕过限制。用户随后手动删除并回复“已删除”；本轮已独立核验两处路径均不存在。
已更新交付回执，手测 EXE 哈希仍与最终构建记录一致，无需因删除未参与构建的旧文件重编译。
运行接线已移除 Go IPC；CPA 默认 Rust，旧 `go`
或未知 transport 设置明确失败，无静默回退。普通非 CPA 风格不受该环境开关影响。
Docker builder 增补 Python/git 及固定源码准备脚本；未声称已执行 Docker 构建。

## 默认 Rust 最终构建与手测候选

- `cargo test --lib cpa_`：11/11；同一新测试二进制 Responses 模块 52/52，包含终止后
  drain、取消及 transport error 回归；旧 profile 选择及 WorkBuddy transport 聚焦测试通过。
- `cargo build --bin gateway`：默认 feature 已包含 Rust CPA transport，无显式 feature 参数。
  保留 24 个既有 lib warnings，未声称无警告或全仓 3,584 项测试全部通过。
- 最终 EXE SHA-256：`cb5a3d27e73d00f09613c1e79411fd9571a584334803edb90264a8c4901fea95`。
- `gateway-full-e2e-20261021-225321`：移除测试进程的 transport 环境覆盖，七组连续五轮
  严格差异/连接复用断言仍全部通过。
- `gateway-proxy-e2e-20261021-225342`：同样使用默认 Rust；HTTPS+Basic 与 SOCKS5h 各五次
  200，各一个代理连接，无代理认证头泄漏，TLS 证书验证开启。
- `retired-go-e2e-20261021-225454`：旧 go 配置真实请求返回 400 及明确迁移信息。
- UI focused 4/4、typecheck、scoped rustfmt、ratchet、`git diff --check` 通过。

候选目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-cpa-no-go-manual-20261021/`。
执行 `Start-Isolated-Rust.ps1`，访问 `http://127.0.0.1:4331/ui/`。
候选是 debug headless EXE + 内嵌浏览器控制台，不是正式 release/桌面安装包。
不携带真实凭证，独立 `manual-data` 不接触日常 `.ng`。
实际启动器 smoke：health、UI 及四个 JS 资源全部 200；独立 smoke-data，残留进程零。
候选回执：`build-receipt.json`、`smoke-receipt.json`、`five-turn-report.json`、`proxy-report.json`。
没有推送、部署、运行真实服务商计费调用，也没有把浏览器资源 smoke 当成人工 UI 验收。

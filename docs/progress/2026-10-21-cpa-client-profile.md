# CPA 调用风格专项补齐

## 范围与参考

在已有 `request_client_profile: cpa` 配置上补齐，不新增协议、认证类型或存储版本。
参考联网固定的 [CLIProxyAPI 20c9095](https://github.com/router-for-me/CLIProxyAPI/tree/20c9095cac089c6a8e40554debe4dd8a19d4c14f)：

- `internal/runtime/executor/codex_executor_request.go` 的 `applyCodexHeadersFromSources`、
  `applyCodexCloakingHeaders`：Codex UA/originator、Version 透传、Accept 和 keep-alive。
- `internal/runtime/executor/helps/claude_device_profile.go`：Claude 固定软件身份默认值。
- `internal/runtime/executor/claude_executor_request.go`：Claude 软件头和流式传输协商。

CPA 有不同 executor、caller-owned、cloaking 和认证模式，并非唯一网络指纹。
本套件明确实现现有产品中的 **CPA CLI 身份兼容选择**：Claude 模型使用 CPA Claude CLI
参考身份，其余模型使用 CPA Codex 参考身份；不声称复制 CPA 所有 executor 或可选模式。

## 修复

- CPA 身份独立模块，不随本机 Codex/Claude CLI 快照同步。
- Codex UA 为 `codex-tui/0.154.0 (Mac OS 26.5.2; arm64) iTerm.app/3.6.11 (codex-tui; 0.154.0)`，
  originator 为 `codex-tui`。真实 `Version` 保留，缺失时不伪造。
- Claude UA 为 `claude-cli/2.1.280 (external, cli)`，设备默认 MacOS/arm64，SDK 0.112.1、
  Node v26.3.0；不被入站本机 CLI 身份覆盖。
- Codex 按实际发送模式选择 JSON/SSE Accept；Claude 第三方流式采用 SSE/identity，
  非流式或官方 HTTPS 主机采用 JSON 和已支持的 gzip/deflate/br/zstd。
  官方主机判断使用 URL 解析和精确主机匹配，不用子串判断。
- 保留显式服务商传输头配置、自定义认证的更高优先级；软件身份由所选 CPA 套件统一管理，
  不被旧 preset 的 UA/originator 覆盖。认证由 adapter 生成，不模拟 OAuth
  权限、不复制调用方凭证、不额外添加第二份认证。
- CPA Claude 分支清理 Codex 会话别名和专有软件/续传头；这些状态不能在 Claude
  协议中直接使用，也不应把安装、窗口信息当作 Claude 身份转发。
  CPA Codex 分支保留真实 Codex session/turn state，不虚构缺失会话。
- 通用发送器对已知 CPA 身份关闭浏览器默认 HTTP 头，不误走 WorkBuddy TLS 配置，
  CPA Claude 不再复用原生 Claude CLI 专用发送器。

## 明确保留的差异

- 风格不是协议转换策略：目标 URL、协议和认证接入仍独立配置。
- 不复制 CPA 的凭证派生会话兜底、设备缓存、OAuth 专有 beta/身份、完整工具翻译和辅助请求。
  缺少真实会话标识时保持无会话状态，不按 key 或消息内容伪造。
- 保留业务提示词、工具和生成参数；未把 CPA 的完整业务转换算法混进风格模块。
- 本轮不实现或宣称 Go TLS/HTTP2 指纹等价。关闭浏览器默认 HTTP 头不等于改变 TLS。
- 显式 Accept/压缩配置可覆盖风格默认，软件身份遵守现有套件所有权；不宣称与 CPA 所有
  cloak 配置的优先级相同。

## 验证

证据目录：`linshi/gateway-cpa-profile-20261021`。修复前新增合同测试 1 项通过、3 项按预期
失败，分别确认 Claude 身份、Version 缺省与透传缺口。

真实调用 `live-codex-01/verification.json`：使用原已授权凭证、`claude-opus-5`，通过正常管理
API 导入隔离实例并选择 `cpa`；真实 Codex 首轮与 resume 两轮均 exit 0、完成事件、assistant
消息和 HTTP 200。凭证 HMAC 一致，字段断言全部通过，无传输错误。入站 Responses、出站
Messages 的协议桥接仍为显式配置，不由 CPA 风格决定。

本轮没有启动 CPA 原程序做完整三路比较：参考值来自固定官方源码，运行证据来自 Gateway
出站抓包；不能把两者表述成 CPA 原程序和 Gateway 在所有网络层完全等价。

回归中发现官方 Codex 专用执行入口的旧 preset Originator 优先级不能改变，已保持所选
软件套件优先；同时在该入口按真实发送模式修正 Accept，覆盖 forced-streaming 场景。

最终源码构建后再次执行 `live-codex-final/verification.json`，两轮实际回复和全部字段断言
继续通过，两轮 HTTP 200、无传输错误。验证二进制 SHA-256：
`54b0e572856784f24323e4524bc01edee66275c09d3c41f403df87d0b04b25c6`。

源码与证据均保留在当前工作区；没有提交、推送、正式部署或修改日常 CLI/DSH 配置。

最终本地验证：

- `verified-build.log`：`cargo build --locked --bin gateway` 成功。
- `verified-tests.log`：七组定向合同测试共 39 项通过、0 失败，含新增 CPA 5 项；
  runtime 合同执行五种 profile × 三种模型类别 × 三种协议表面 × buffered/SSE，共 90 次
  实际回环 HTTP 发送，其中 CPA 18 次。该矩阵不是 90 次真实服务商调用。
- `verified-gates.log`：effective-lines 自测 31 项、ratchet、rustfmt、diff whitespace 均通过。
- `lines.json`：新 CPA 模块 134 有效行，新合同测试 150 行；修改文件均低于 500 行，
  没有触发超限例外，文本为 UTF-8 无 BOM。
- `source-hashes.json` 记录本轮十个源码/测试文件，真实复测后核对未漂移。
- 隔离测试 Gateway 已退出；未停止用户已有 CLI/服务。

仍有仓库现存编译警告；本轮未执行全仓所有测试或发行打包门禁，不作为正式发布验收。

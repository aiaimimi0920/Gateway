# 官方健康适配器实际代码接入

## 范围

六个官方适配器已接入 Rust 探测、分类观察、缓存准入及实际 Web/桌面共用设置页面。
选择即填充固定入口和无需认证；旧持久密钥仅通过原有授权 Clear 流程删除。
只有 ChatGPT 的 Codex API 状态统一继承到检测方案覆盖的所有模型。
DeepSeek 使用已核实的精确组件与模型身份；其他官方组件不冒充逐模型观测。
接口、限制和命令见 [官方站点健康适配器](../official-site-health.md)。

## 已执行验证

- Rust 站点健康单元测试：9 passed。
- 六组 Rust 站点健康集成合同：20 passed；1 个显式联网测试默认 ignored。
- 前端七个相关测试文件：28 passed。
- 前端 typecheck、build:web 通过。
- 有效行 checker 测试：31 passed；ratchet 通过，无新增超限源码。
- 定向 rustfmt 检查和 `git diff --check` 通过。

## 联网验证边界

已通过公开网络读取核实六家入口及不同响应结构。另行执行新 Rust 实现的
`official_public_status_smoke` 时，首次探测因系统 DNS 返回保留地址而失败：
`Site resolves to a prohibited network address`。
随后逐个查询确认六个域名均被解析到 `198.18.0.177` 至 `198.18.0.182`。
没有绕过 SSRF 校验，没有修改网络配置，不能将本次 smoke 记为通过。
日志保存在 `C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-official-status-smoke-20261023.log`。

## 未执行及保留项

- 未替换现有运行 EXE，未生成手测包或正式发布包，未部署、提交或推送。
- 未做完整桌面人工验收；当前网络环境下六家 Rust 实际请求的端到端验证仍待完成。
- OpenRouter 本轮读取官方站点组件状态，不扫描各模型的上游 endpoint uptime。
- Gemini 使用官网内部只读 RPC；若官方结构变化，报告未知而不是假定正常。
- 保留工作区既有改动、运行实例、数据库及凭据。

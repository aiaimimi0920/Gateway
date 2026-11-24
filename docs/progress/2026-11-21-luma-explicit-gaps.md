# Luma 明确缺口对账与补齐

## 本次范围

用户允许确因额度不足而无法测试的内容暂不开发。现有 Web 单次提交的 `402 / Not enough credits` 证据继续有效，不再次生成或尝试充值；不把跳过改写为通过。

对账后直接进入开发的是 5 个功能点，归属 2 个修改范围：

1. MCP `approve` 显式答复。
2. MCP `confirm` 显式答复，保留不可撤销提示。
3. MCP `text_input` 显式答复。
4. MCP `select` 选项展示及显式答复。
5. Web 已保存凭据的 `Keep` 归属保护，防止保留 Cookie 时改变网站、realm、UA、到期时间或认证上下文。

前四项根据已经归档的 26 项工具合同：`PendingGateOutput.interaction` 明确包含这四种类型，`luma_run_respond.result` 是开放 JSON。沿用现有手工 JSON 答复，不猜测新的布尔值／对象包装协议，不注册或执行任意客户端工具。显示上游问题、选项、数量限制和已校验的公共媒体链接；仍要求逐次确认、精确原任务／invocation／fingerprint 和持久发送回执。选择题的候选资产 ID 数组按已知合同检查数量、成员和重复；`session.waiting` 仅与同一原任务的完整问题匹配，不从不完整摘要猜测题型。

Web 现有导入路径已检查产品和目的地，但 `KeepBindings::protected_product` 未识别 Web Luma，已有凭据保存时未执行完整目的地比较。本次复用现有 Keep owner，并补 realm 别名、UA、有效期投影，不创建新存储或登录机制。

## 证据边界

仍有 2 项合同待证，不混进上述可直接实现的 5 项：

- MCP 分层布局：输入合同已知，但当前 `RunAssetOutput` 没有说明描述中提到的 `index/label/bounds`，开放 `result` 不能证明布局形状。Agents REST 的分层已经实现，不能将它的结果结构当作 MCP 的结果。
- Web 会话续期：现有产品支持材料导入、到期信息与失效探测；`lumalabs-session-worker.mjs` 没有接入正常刷新 owner。续期／轮换协议需要当前浏览器证据，不能因 Web 生成额度不足而宣称该问题也由额度造成。OAuth/MCP 原有显式刷新不属于此项。

2026-11-21 匿名联网读取官方 [MCP 工具页](https://docs.agents.lumalabs.ai/mcp/tools/)和 [MCP 与 API 对照页](https://docs.agents.lumalabs.ai/agents/mcp-and-the-api/)，均返回 200。前者列出 `luma_generate_layering` 和 `luma_run_respond`，后者给出响应工具到 REST tool-response 的对应关系；这两页没有补出 MCP 分层的布局字段，不能据此关闭该待证项。

Web 成功提交、完成事件、媒体绑定及依赖已受理任务的远端取消，保留为额度阻断的支持边界。已有 L4 追账、Server 管理任务、强杀恢复、L6 包完整性和隔离启动证据不重复执行。任意客户端工具、无合同断点续传、媒体 credits 到 USD 换算不扩成本次必做。

## 验证结果

前端新增 4 项在修改前全部失败，原有 11 项通过；修改后补入选择数量、重复和范围校验，与创作面板相邻回归共 **24 项通过**，TypeScript 类型检查通过。

Rust 修改前的 Luma 过滤批次为 **334 passed / 5 failed / 2 ignored**；5 项失败均来自新增问题和 Keep 用例。修改后只运行 `console::secrets::` 与 `protocol::luma_mcp::creation`，结果为 **58 passed / 0 failed / 4 ignored**。覆盖四种原问题的正常管理请求、确认前不发送、精确原 run／invocation、单次持久答复、会话等待关联、非法选择和媒体拒绝，以及 Keep 目的地／realm／UA／新消费者拒绝与普通模型编辑兼容。4 个 ignored 是已有显式浏览器／Server／子进程入口，不计为 passed。

官方 rustfmt 检查、**31 项**规模检查器测试、仓库 ratchet、范围内 `git diff --check`、UTF-8 无 BOM 与空白检查通过。主要已有生产文件有效行：Keep owner 321 → 325，review 协议 239 → 258，前端 review 64 → 76；新问题协议 117 行、Web 绑定 58 行。未修改规模 baseline 或 exception。

红灯 Rust 编译 25m30s；绿灯使用仅对 Gateway 测试包的 `profile.test.package.gateway.debug=0`，编译 13m55s、执行 21.31s。未改优化等级、断言、功能或仓库配置；不同输入及编译配置不作为可推广的提速倍数。长期运行命令见 [tools/luma/README.md](../../tools/luma/README.md)，脱敏回执与源码哈希见[验证记录](../evidence/luma/gateway-luma-explicit-gaps-20261121/verification.json)。

本轮 5 个明确功能点的源码与定向验证完成，剩余 2 个合同待证项如上。真实供应商端是否接受各答复、原生点击和新包验收未在本批执行。

未执行供应商调用、刷新凭据、替换运行实例、提交、推送或重新打包。原 `gateway-devtest-20261121-luma-handoff` 不包含本次源码增量。

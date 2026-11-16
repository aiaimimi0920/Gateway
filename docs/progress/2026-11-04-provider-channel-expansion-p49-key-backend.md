# P49 后续：Kiro CLI 原生 Key 后端子链

日期：2026-11-04。工作区：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`。
本批是后端调用子链，不是总计划 P50、W02 整卡、前端模板或真实账号验收完成记录。
总目标保持执行中，不要求现在录制凭据。

## 1. 改了哪些内容

采用后端 preset `kiro-api-key`、auth mode `kiro_api_key`，复用 `kiro_compatible` executor 和有界 EventStream decoder，
不增加第二套解析器、CLI helper 或服务。已有 `kiro` OAuth/IDE 路径仍保留其旧设备、模型映射与业务合同；
新 Key 不继承 IDE machine/profile/client 注册，不产生 refresh owner。

- 配置原声明在合并前锚定，拒绝切换协议/受众、OAuth context、refresh、generic discovery、session、浏览器执行、keepalive、
  自定义 header/body 和替代路径。空池可编译保存，空 Key 仍可清除但不可发送；不修改存储版本或既有凭据。
- 只向 `https://runtime.us-east-1.kiro.dev/` POST，固定 bearer、`TokenType: API_KEY`、AWS JSON target/content-type 和 CLI 2.10.0 SDK UA。
  最终发送再次校验 plan，并由 Key owner 设置最终 headers；不跟随重定向，不做 403 刷新或隐式重提。
- CLI body 的 current/history 使用 `KIRO_CLI`，不含 IDE continuation/profileArn；模型显式且原样保留，无默认/猜测目录/IDE别名。
  复用既有 system/history、function tools、data-image packing；reasoning、强制工具选择、server-side tools 和未知扩展明确拒绝。
- 流式/非流式复用现有 decoder，响应必须是 EventStream，HTTP 200 错误 JSON 不伪装成成功。
  Chat Completions、Messages、Responses 的文本响应转换保持 Key 的精确模型 ID。
- 正常管理 Secret Grant 下的显式模型测试使用选定账号和普通 sender，不经过路由/fallback/refresh；禁用、未配置模型、
  无授权或没有显式测试时不发送。成功要求完整非空 text-only 回复；超时/8192字符上限/审计沿既有流程，返回不回显 Key。

主要生产 owner：

- [产品身份与最终 plan 边界](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/kiro_api_key.rs)
- [配置编译](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/routing/config/kiro_api_key.rs)
- [CLI wire 与发送](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/kiro_api_key.rs)
- [精确账号模型测试](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/provider_runtime/kiro_api_key_model_probe.rs)
- [固定参考与完整许可](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/kiro_api_key/REFERENCE.md)

官方只证明 CLI/headless Key 和同用户 subscription credits 扣费，**不证明官方 direct HTTP 接口或与 OAuth 独立余额**。
采用 `d-kuro/kirocc @ 7a2ba670286ca16ae89fe792ea935b3751179421` 的有限 wire/结构参考，自行用 Rust 实现；
完整 Apache-2.0 许可保留，固定完整 Git tree 无 NOTICE。未复制其 Go 执行逻辑、重试、正文日志、目录假设或模型权益表。
`max_tokens/max_completion_tokens` 与旧 Kiro 输入兼容但没有已证的 CLI wire 映射；token usage 仍为旧 decoder 估算语义，
不是实际 subscription credits。合成工具/图片 packing 通过不等于真实账号具备相关模型权益。

## 2. 验证与失败保留

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-kiro-native-key-20261104`。
Cargo 使用 `--locked --jobs 4 --no-default-features --features runtime-defaults`；测试进程单独设置
`RUST_MIN_STACK=16777216`，TEMP/TMP 指向本批 linshi。不运行默认 management-ui 构建 hook。

| 检查 | 实际结果 |
| --- | --- |
| 行为红测 | `red-key.log`：0 passed / 3 failed，复现 AI_EDITOR origin、TokenType 被转发头覆盖、泛用 adapter 未被拒绝 |
| 首次编译检查 | `check-v1.log`：failed；新增流包装遗漏 `From<ProtocolStreamError>` 约束，测试误用 `RequestPlan::clone()`；均已按真实类型修复 |
| 第一轮定向绿测 | `green-key-v1.log`：12 passed |
| 扩大请求边界后定向绿测 | `green-key-v2.log`：14 passed / 0 failed / 0 ignored；新增拒绝强制工具/未知扩展/过长模型，以及 function tools/data-image packing |
| Kiro 相邻实现 | `kiro-related.log`：104 passed，包含上述14项，不叠加为118 |
| preset / call-flow | `preset-related.log`：50 passed；`call-flow-related.log`：9 passed |
| generic discovery / management probes | `discovery-related.log`：32 passed；`console-probe-related.log`：11 passed |
| 共享 packing / EventStream | `kiro-decoder-related.log`：55 passed / 0 failed / 0 ignored；覆盖本批改过的共享 packing、流与非流式 decoder |
| 代表性关闭 Kiro feature | `kiro-key-feature-off.log`：2 passed / 0 failed / 0 ignored；材料纯校验仍可用，实际传输明确拒绝；完整 feature 列表保留在 `feature-off-list.txt` |
| 静态检查 | Rustfmt、有效行 checker 31 项、ratchet 与最终 `git diff --check` 通过；未运行全仓 Rust 回归、UI 构建或正式发布门禁 |

按完整测试名称去重，六组主配置相关回归共 **250 项通过**，0 failed / 0 ignored；14 项新增测试已包含其中，
不重复叠加第一、二轮绿测。关闭配置另有 2 项通过，其中只有 feature-off 拒绝测试是主配置没有的名称。
分组原始结果与去重清单保留在 `test-summary.json`，不把两种 feature 配置合并为同一运行结果。

当前 33 项 Rust 输入逐项匹配 `source-green-input.json` 的 SHA-256，UTF-8 无 BOM 全部通过；
`audit-scope.py` / `source-audit.json` / `scoped-source.diff` 记录相对本批写前副本的变化，不冒用工作区历史 dirty diff。
本批涉及源码最大 469 有效行，新文件最大 168；没有超过 700 的增长。既有共享 packing owner 从 320 到 350，
只提取 origin/精确模型的共享选项，不改 OAuth/IDE 旧入口语义。检查原始退出码见 `related-exits.json`、
`decoder-exit.json`、`final-static-exits.json` 与 `closeout-exits.json`；源冻结前的红测和首次编译失败均保留。

## 3. 后续接力与未验证边界

1. 接正常前端 `kiro-api-key` 模板、Key 添加/编辑/清空及精确模型策略，不能复用 OAuth 登录按钮或自动模型发现。
2. 补普通 SecretPatch/Keep、错误池导入、稳定 ID/重启和完整前端交互；folder parser 必须分辨 Key 与旧 Kiro OAuth，不能按 adapter 相同就构造注册。
3. 补公开完整 pipeline 的路由/JSON/SSE集成与正常 UI→保存→调用闭环；本批 sender/管理接口回环证据不冒充该完整集成。
4. 新 Key 目录、更多 Region/profile、reasoning/输出预算等能力只按后续固定参考或真实录制证据扩展，不使用冲突的第三方目录假设。
5. 全范围无凭据开发完成后再做 P50 devtest 来源/载荷/隔离启动，随后 P60 集中录制与 P70 精确渠道真实验收。

累计新增前端模板记录仍12，没有新增第13个模板或完整 offline_ready 卡。Kiro旧route/DB自动升级、最小scope/profile/订阅、
跨应用 receipt、Vertex OAuth/Express、Luma Agents、Grok余项、其他Web/媒体完整生命周期和Qwen OAuth受限待决项仍保留。
未读取 `.ng`/历史凭据、登录、真实推理、创建 Redis 容器、打包、替换在线实例、提交或推送；本批没有删除 linshi 外内容。

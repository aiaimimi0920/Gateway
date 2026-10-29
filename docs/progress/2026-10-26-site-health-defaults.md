# 模板默认站点健康与当前实例启用（2026-10-26）

## 源码改动

`defaultCatalogWorkflow` 现在按真实适配能力默认开启站点健康：20 个已匹配官方适配器的
标准模板，以及 1 个自定义公开接口识别模板。默认 advisory、未知 allow、300 秒刷新、
900 秒有效，不发送业务凭据，不启用账号自动测试。用户显式关闭仍生效。

没有匹配健康来源的六种产品保持关闭：ChatGPT Web、OpenAI Platform、Gemini Web、
AI Studio Web、LongCat、WebSearchAPI。不能把只覆盖 Codex 的 `openai-status` 直接当成
OpenAI Platform 健康，也不能用付费搜索或生成请求代替状态读取。

模板默认值仅影响新草稿，不自动迁移既有站点。没有修改健康解析、传输安全、数据库版本、
依赖锁或运行程序。主源码有效行 98→98，新增回归测试 37 行。

## 当前 4200 实例

用户已授权开启现有服务商监测。先以 SQLite online backup API 保存私有备份，完整性为 ok，
再通过正常管理 API 的 validate、secret Keep、revision/CAS 保存；没有直接编辑 YAML 或 SQL。
revision 从 `r5-a715ecbd19bf` 更新至 `r6-21ad1c944498`。

已开启 NVIDIA、OpenRouter、You、Exa、ChatGPT、Jina、Tavily、Linkup，分别使用各自的官方
适配器。当前 WebSearchAPI 未确认官方健康源，保持未配置，不添加假正常监控。
配置逐字段复核仅 `site_health` 改变，所有业务凭据、调用 Key 的身份/加密秘密/权限保留。

真实后台 worker 已自动运行八个方案，但八个快照均为
`network_policy_or_detection_error`、`selected_template=null`，不是成功健康证据。
独立系统 DNS 核对将八个官方状态域名解析为 `198.18.0.154` 至 `198.18.0.161` 的 Fake-IP；
安全 transport 明确拒绝该保留网段。没有放宽 SSRF/IP 检查，也没有修改系统 DNS、代理规则或 hosts。
因此“已启用并调度”已验证，“实际取得健康状态”仍被本机网络配置阻塞。

所有 provider/credential 的账号自动测试仍关闭；本次工具未发送生成、搜索、模型发现或账号测试。
NVIDIA 审计仍为 19、账号测试结果仍为 0。运行中其他普通客户端流量继续写入全局审计，
不声称整个实例的全局请求计数未变化，也不将该增长当作健康监测调用。

## 验证与交付边界

- 新回归测试先红：43 项中 42 项因默认 false 失败；修复后前端 13 文件、86 项全部通过。
- TypeScript typecheck 通过；有效行 checker 自测 31 项、ratchet、全仓 `git diff --check` 通过。
- checker 首次报告路径使用了不支持的仓库外绝对路径而退出，保留失败证据；改用官方
  `node scripts/effective-code-lines.mjs --mode ratchet` 后通过，未改写已有 dirty report。
- 本轮没有 Rust 改动、付费上游回归、EXE 构建、正式发布、提交或推送。当前 EXE 未替换；
  八家已保存配置即时生效，新的模板默认值需要后续构建后才进入分发 EXE。

公开脱敏证据目录：
`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-site-health-defaults-20261026/`。
包含红绿测试、typecheck、checker、配置保存回执和实际 worker 快照；`private/` 中备份不可发布。

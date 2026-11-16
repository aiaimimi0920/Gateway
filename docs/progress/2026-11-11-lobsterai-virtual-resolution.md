# LobsterAI 虚拟模型解析与双协议推理（2026-11-11）

本批接续普通模型公共推理，覆盖 `yd-fast`、`yd-ultra` 按请求解析为普通 OpenAI Chat 或 Anthropic Messages 模型。完整工作项仍为 67 项未关闭；此批只形成本地合成合同证据。

## 实现范围

- 每个公共请求在现有取消／总超时／容量 owner 内执行一次 `POST /api/models/{preset}/resolve`，携带 Gateway 本次请求 UUID 和 `requiresImage:false`。不引入跨请求会话缓存；工具续轮各自解析。官方 QuickAnswer 同样每次生成 UUID，客户端持久会话绑定不在本切片中实现。
- resolve 与推理均使用固定官方目标、本人 bearer、专属无重试连接池，不继承外部头、不跟随跳转。目录与已解析推理仅声明 `virtual-models-v1`，没有声明 Kimi、thinking 等未实现能力。
- resolve 响应有 15 秒／现有更短预算、64 KiB 与重复 JSON 字段限制。未知格式、再次返回虚拟模型、不可访问、特殊 runtime profile、Kimi 以及无法保留的请求语义明确拒绝。未返回成功合同不发送推理。
- 解析后重读当前 credential owner／generation／access，再复查发送资格。真实 HTTP 401 唤醒既有唯一持久 worker；不在原请求内刷新后重发。解析期间发生 policy reload 时，后续推理 401 使用新捕获的 owner。
- 该竞态测试只覆盖 reload 发生在 resolve 等待期间。ticket 捕获后若再次替换路由 wrapper，仍沿用既有“退役 wrapper 的迟到 401 不再触发刷新”规则；本批未宣称任意 reload 时点的无缝转交，也不改变旧代次隔离规则。
- 原 `yd-*` 路由与冻结报价不变。审计增加 `upstreamModelResolution` 记录 preset、具体 modelId、apiFormat；成功请求的目标协议按真实结果记录。buffered 与 streaming snapshot 都保留这些事实。
- Messages JSON 与 SSE 使用原生准入和结束校验：错误尾包、截断、未闭合块、未知内容和坏工具参数不能产生成功。文本增量输出，工具参数有界累计；不借用普通 Chat 对缓存字段的约束。
- 原生 Messages usage 保留 exclusive input 和 cache read/create；现金结算副本和公共 OpenAI 用量使用包含缓存的输入总数。公共 Messages 继续保留原生口径。最终输出用量缺失时保持未知，现金 reservation 为 unresolved，不能按起始的零输出结算。
- 同 family 的 `max_completion_tokens`／`max_output_tokens` 也归一化到 Messages `max_tokens`，避免按 32 预扣却实际发送默认 4096。支持情况不由 caller 自定义目标、头或 metadata 改写。

## 官方合同与证据

本批匿名联网读取官方固定 SHA `5d6621a0aed22adb53439e290234cc2d64318caf` 的六个文件，URL、时间、SHA-256 在本批 `contracts/sources.json`：

- `src/main/libs/modelPresets.ts`：resolve 路径、sessionId、requiresImage、返回格式。
- `src/shared/modelPresets/constants.ts`：虚拟模型 ID。
- `src/main/libs/openclawTokenProxy.ts`：resolve 与 `/api/proxy/v1/*` 路径分工。
- `src/shared/providers/modelRuntimeProfiles.ts`：特殊 profile 边界。
- `src/main/main.ts`：普通会话与 QuickAnswer 的不同 sessionId 生命周期。
- `src/main/libs/claudeSettings.ts`：客户端模型配置分工。

官方源码是协议依据，不是第三方账号资格或自有版本头可用性的真实验收。

## 验证

最终 Rust 定向联合测试：**134 passed / 0 failed / 0 ignored**，其中名称含 `lobsterai` 的 71 项通过。命令：

```text
cargo test --locked --lib -- lobsterai protocol::commandcode:: protocol::openai::chat_stream protocol::anthropic::to_anthropic cash_billing:: routing::config::refresh_handoff_tests --test-threads=2
```

覆盖两个虚拟 preset、两种具体协议、三公共入口、两种 stream 设置；实际响应 Content-Type 反转、两回合函数工具、失败／取消退款、resolve HTTP 401、policy reload 后推理 HTTP 401、缓存现金结算、输出上限别名、最终 usage 缺失保持 unresolved、坏工具／错误尾部／截断均有断言。相邻 CommandCode、Chat 守卫、Messages 转换、现金账本及 refresh owner 交接回归包含在同轮中。

代码行检查器 31 项通过；ratchet、定向 rustfmt、UTF-8 无 BOM 和 scoped diff 检查通过。最终源码 SHA-256 与测试启动时 `source-r3.json` 一致。全部本批新增／修改的 Rust 文件低于 500 有效行。

前批 UI／桌面 launcher 证据未重跑；测试通过不代表 EXE／Server／真实账号已验收。

初始红测确认旧实现会在发送 resolve 前拒绝虚拟模型。r1 为 96 passed / 2 failed：新测试夹具把工具响应与 SSE 参数顺序写反，已修正。r2 为 126 passed / 1 failed：竞态夹具修改显示名称并不会替换 owner，已按真实 owner 规则改为认证池策略变更；没有放宽断言。r3 同时验证最终用量、坏工具和公共 OpenAI 缓存计量修正。每轮日志和源码快照独立保留。

## 未验证边界

真实账号／订阅权限／消费、自有版本与 capability 头接受性、跨请求的持久模型绑定、企业模式、特殊 runtime profile／Kimi、reasoning／媒体、Server／Redis、独立 feature 构建、原生 EXE 仍开放。没有真实登录、付费上游、读真实 `.ng`、出包、提交、推送、部署或重启。

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-lobsterai-resolve-20261111`。基准 HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`；工作区有大量继承改动，以本批 before 备份、SHA-256 和 scoped diff 定界，`wholeWorkingTreeProvenance=false`。

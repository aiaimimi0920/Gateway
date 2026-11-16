# MiniMax M Plan 专属控制台 Messages 文本测试

## 本轮范围

MiniMax 原测试已通过生产 dispatcher 发送，并不存在 Agnes/SenseNova 的发送绕过问题。
缺口在通用 Anthropic SSE 测试解析：缺失 usage 初始化为零，部分解析错误被忽略，
回复被截到 8192 字符而非明确拒绝。这些行为不足以证明 M Plan 文本测试结果完整。

本轮只为 M Plan 增加严格非流式 Messages 测试，不改普通 Anthropic SSE parser，
不新增 HTTP sender、不改变公共三入口的流、工具或 reasoning 行为。

- `minimax/probe.rs` 使用生产 `UpstreamClient::build_request_plan` 与
  `send_profiled_plan`，保留国内/国际固定目标、精确账号 Bearer、禁跳转和身份头清理。
- 64 KiB 正文、唯一键 JSON、合法 Messages envelope、`end_turn`、非空且不超过
  8192 字符的文本；拒绝工具、未知 block、坏 JSON 和截断答案。
- 合法 thinking 与文本可共存，但只把文本作为答案；thinking-only 不算文本成功，
  不将 thinking 内容写入答案或审计。这里不是多轮 reasoning 验收。
- usage 缺失/null 时保持未知；存在时要求合法 input/output，拒绝负数、字段缺失、
  总数溢出和非法缓存计数。Key 在预览、评分及保存前脱敏。
- 通过已有 Anthropic probe 的单次 60 秒预算、audit/health 和取消 owner 执行，
  不复制审计生命周期；普通 Anthropic 仍走原路径。
- M Plan 加入已有配置模型/别名准入条件；默认无测试选项不发请求。

## 验证

执行 `cargo test --locked --offline --lib -- minimax anthropic_model_probe --test-threads=2`：
**22 passed / 0 failed / 0 ignored**，原有 MiniMax 15 项及新增 7 项全部通过。
本次筛选没有额外命中普通 Anthropic 测试，不能将其算作普通 Anthropic 集成回归；
`tests/console_anthropic_probe_contract.rs` 本轮未重跑。
测试执行 17.54 秒；完整命令 1370.58 秒，包含共享构建锁等待与编译，不作为纯编译耗时。

官方 formatter、checker 31 项、effective-line ratchet 通过。9 个 Rust 文件最终哈希
与测试前快照一致，UTF-8 无 BOM 检查通过；其他模块已有 warning 保留，未做无关清理。

新增 7 项用例复用现有 MiniMax loopback peer、LocalRuntime 和管理夹具，覆盖精确
账号/别名、脱敏、用量与未知用量、无效回复、错误不重放、取消释放、授权与计划
结果读取、禁用账号和错误模型。管理计划对国内/国际两个 preset 分别执行；thinking
和文本混合回复不泄漏内部块，只有 thinking 的回复拒绝。

有效行：控制台分发 499 → 500，Anthropic probe 178 → 186，MiniMax 模块
255 → 256，登记模块 108 → 109，测试登记 178 → 180。新增生产模块 69/23 行，
新增测试 174/164 行，无例外或 baseline 更新。

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-minimax-probe-20261112`。
TEMP/TMP 指向其中的 temp，未读取真实 `.ng`。保留修改前源码、scoped diff、行数和
SHA256、编译测试及门禁日志/receipt。

## 未验边界

真实 M Plan Key、套餐额度、费用、模型权限、Server/Redis、多实例、自动调度到期、
原生包及独立 feature 未验。MiniMax Code Agent OAuth、余额发现和完整产品验收仍开放。
本轮没有修改 UI、调度器或存储格式；不出包、部署、重启、提交或推送。

# Agnes AI 国际站专属控制台模型测试

## 实现范围

修复 Agnes 显式模型测试落入通用 OpenAI 探测发送器的缺口。旧路径没有复用 Agnes
生产最终 sender 的身份清理与禁跳转边界，且其通用回复检查弱于本次文本测试要求。
本轮不通过旧路径向真实上游发送合成 Key，不读取真实账号来复现。

- `src/protocol/agnes/probe.rs` 使用现有 request plan 与 `send_profiled_plan`，保留
  Agnes 固定目标、账号 Key、模型限制及无 ambient credentials/redirect 的发送规则。
- 仅测试 Chat 文本，不把测试成功当作原生 Responses 或完整媒体能力通过；公共
  Chat/Messages/Responses 推理实现未改变，Agnes Code 和国内站未混入此范围。
- 64 KiB 正文上限、唯一键 JSON、共享 Chat admission、一条非空 stop 文本、
  8192 字符答案上限；拒绝 refusal、工具结果、坏用量及不完整生成。
- 在预览、评分、保存前脱敏 Key；错误不反射上游正文；未知用量保持 null。
- `src/provider_runtime/agnes_model_probe.rs` 复用取消安全的审计与健康状态 owner，
  单次 60 秒总预算，失败不重放、不切账号、不额外扣普通访问 Key 的预付次数。
- 控制台仅增加 Agnes 分发与已有严格 Key 产品准入条件：默认无选项不发请求，
  显式测试要求已配置模型或合法别名，再按精确账号的模型范围选择。

## 验证

执行 `cargo test --locked --offline --lib -- agnes --test-threads=2`：
**27 passed / 0 failed / 0 ignored**，原有 Agnes 20 项与新增 7 项全部通过。
测试执行 6.12 秒，完整命令 791.55 秒；Cargo 报告编译 13 分 04 秒，不能把整条命令
耗时当作测试耗时。本轮未重复 SenseNova 或 Command Code 的测试。

官方 formatter、checker 31 项、effective-line ratchet 通过；8 个 Rust 文件最终哈希
与测试前快照一致，UTF-8 无 BOM 检查通过。已有其他模块的编译 warning 未顺手清理。

新增 7 项测试覆盖：精确账号/别名、回复脱敏与用量审计、未知用量、无效及重复键
JSON、超大正文/答案、截断结束/refusal/工具/坏用量、HTTP 错误不重放、取消释放与
取消审计、管理 Secret Grant、外来账号拒绝、单次/临时/命名/保存策略及结果读取、
禁用账号和未配置模型不发送。复用 Agnes 原有 loopback peer、LocalRuntime 和管理夹具。

有效行：控制台分发 495 → 499，产品模块 227 → 228，模块登记 107 → 108，
测试登记 183 → 185。新增生产模块各 64 行，新增测试 131/157 行；没有超过 500
行的新文件，没有例外或 baseline 更新。

证据：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-agnes-probe-20261112`，
包含本轮修改前备份、scoped diff、源码哈希/行数、构建测试及门禁日志/receipt。
临时数据通过 TEMP/TMP 放在本证据目录下，不读取真实 `.ng`。

## 未验与非目标

本轮未改 UI、调度器、存储版本或凭据格式。没有真实账号/订阅额度/费用验收，
没有重做浏览器视觉、Server/Redis、多实例、独立 feature 或原生 EXE 验收。
未变化的 Gemini feature 裁剪阻碍沿用原记录，不重复构建。
不出包、不部署、不重启、不提交、不推送；本切片不等于 Agnes 整卡完成。

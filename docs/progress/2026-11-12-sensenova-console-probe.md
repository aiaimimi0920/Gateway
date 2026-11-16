# SenseNova Token Plan 专属控制台模型测试

## 本轮改动

修复显式模型测试落入通用 OpenAI 探测发送器的缺口。该旧路径未经过 SenseNova
生产最终 sender 的固定目标、身份头清理、禁跳转及专属响应校验，不应以普通 Chat
协议兼容性代替产品边界。未使用旧路径向真实上游发送合成 Key 来复现。

- 新增 `src/protocol/sensenova/probe.rs`：复用 `send_profiled_plan` 及现有 SenseNova
  finalizer；仅使用本账号 Key、固定 Chat 目标，不另建 HTTP sender，不重放失败请求。
- 回复最多 64 KiB，使用唯一键 JSON 解析和共享 Chat admission；只接受一条非空、
  完整 stop 的文本回复，拒绝 refusal、工具结果和超过 8192 字符的答案。
  在返回、评分及持久化前脱敏 Key，错误不反射原始上游正文。
- 新增 `src/provider_runtime/sensenova_model_probe.rs`：60 秒总预算，复用取消安全的
  audit/health owner；未知 usage 保持 null，不伪造零用量或消费成功。
- 控制台分发仅增加本产品分支，默认无测试选项时不发请求；显式测试必须使用配置
  模型或合法别名，继续按精确账号模型范围准入。与 Command Code 共用这一准入条件，
  未改变后者的 sender，只将两条拒绝提示改为产品中立措辞。
- 新增两个测试文件，复用本产品现有 loopback peer、LocalRuntime 和管理 API 夹具，
  不新增测试框架，不修改 UI、调度器、凭据存储版本或公共推理路径。

## 验证记录

执行 `cargo test --locked --offline --lib -- sensenova commandcode --test-threads=2`：
**57 passed / 0 failed / 0 ignored**，其中新增 SenseNova 专属测试 7 项全部通过。
Command Code 因共用准入条件纳入相邻回归；名称筛选也包含此前跨产品调度的 3 项测试，
它们不是本轮新增功能。测试执行 80.69 秒，完整命令 659.84 秒；Cargo 报告编译
9 分 38 秒，不把完整命令耗时当成纯测试或纯编译时间。

官方 formatter、checker 31 项、effective-line ratchet 和 scoped `git diff --check`
通过。8 个 Rust 文件最终哈希与测试前 `source-r1.json` 一致，UTF-8 无 BOM 检查通过。
已有其他模块的编译 warning 保留，未借本轮清理无关源码。

测试覆盖：精确账号与别名、秘密脱敏、真实回复用量、未知用量、无效/重复键 JSON、
超大正文及答案、不完整结束、refusal、工具结果、错误用量、HTTP 错误无重放、取消
释放响应及取消审计、Secret Grant、外来账号拒绝、单次/临时/命名/保存策略与结果读取、
禁用账号和未配置模型不发送。

源码有效行：控制台分发 490 → 495；产品模块 222 → 223；模块登记 106 → 107；
测试登记 178 → 180。新增两个生产模块分别 53/64 行，新增测试分别 131/160 行。
没有新文件超过 500 行，不申请例外、不改 baseline。

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-sensenova-probe-20261112`。
包含修改前备份、`scoped.diff`、源码哈希/行数、Rust 日志和 receipt、formatter、
checker 与 ratchet 回执。TEMP/TMP 指向该目录下的 temp，未读取真实 `.ng`。

## 未验边界

这只是控制台文本测试切片，不是 SenseNova 整卡完成。真实 Key/订阅权限、费用和积分池、
控制台 OAuth、真实上游响应、完整 reasoning/media、Server/Redis、多实例、原生 EXE
以及独立 feature 构建仍未验。本轮不重复此前未变化的 Gemini feature 裁剪失败。
未出包、部署、重启、提交或推送；未重新进行 UI/浏览器视觉验收。

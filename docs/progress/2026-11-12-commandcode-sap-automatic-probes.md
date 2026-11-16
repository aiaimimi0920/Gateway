# Command Code / SAP 自动调度本地验收（2026-11-12）

状态：自动测试策略的本地定时执行与持久化子项通过；两项产品仍为 partial_pass，不代表真实账号、Server/Redis 或 EXE 验收。

## 改了哪些内容

新增 scheduler 产品测试与独立 LocalRuntime 夹具，仅注册测试模块，不修改生产调度、发送、认证、计费或存储语义。

- 运行原生产调度循环，使用真实 60 秒 timer 和真实墙钟到期槽；不暂停时钟、不改写数据库 next 时间。
- 同时配置 Command Code 与 SAP，每个产品包含账号策略和命名计划。首轮 4 次调用，真实到期后第二轮累计 8 次，精确账号、各自结果键和 mode=automatic 均校验。
- 未到期立即扫描跳过；关闭并重开 LocalRuntime、新建 AppState 后，结果和间隔槽仍保留，不立即重放。此为进程内重新建状态与存储重开，不冒充原生 EXE 重启。
- SAP 复用已有 token cache，部署由固定 URL 选择，请求体不发送 model；Command Code 使用本账号 Bearer Key 和请求体模型。
- 401 失败保存 failed，无虚假用量或答案评分；无隐藏重试，立即再扫描不重复执行。
- 关闭自动策略、禁用账号及已请求关闭的 worker 不发起生成；保存结果不含合成秘密。
- 复用既有真实 loopback peer、生产发送器、LocalRuntime 和审计机制，不访问外网或真实凭据。

## 验证

最终命令：`cargo test --locked --offline --lib -- commandcode sap_aicore provider_credential_probe_scheduler --test-threads=2`。

**60 passed / 0 failed / 0 ignored**：原 Command Code 30 项、SAP 26 项、调度器已有身份键用例 1 项、新增跨产品调度用例 3 项。测试执行 79.79 秒；完整命令 1355.84 秒包含共享构建锁等待和编译，不能将其当成纯编译耗时。

checker 31 项、ratchet、定向 rustfmt、scoped diff 与 UTF-8 无 BOM 检查通过。两个新增文件为 230、114 有效行；调度器入口为 168 → 174，有效增长仅测试注册。3 个受影响 Rust 文件与最终编译前 source-r6.json 哈希一致；不声明整个 inherited dirty 工作树来源。

原始失败与中断记录保留：r1/r2 为测试夹具依赖及类型错误；r3 暴露重复账号 ID 与测试线程栈不足，改用合法全局唯一 ID 和项目已有 16 MiB 线程夹具；r4 被中断且无最终回执；r5 为 58 passed / 2 failed，两项均错误要求 SAP 请求体含 model，已按生产部署合同修正断言。没有放宽生产校验或改变发送语义。r6 是最终权威结果。

## 尚未完成

真实账号/消费/上游响应、Server/Redis、多实例调度、原生程序启动接线与进程重启、EXE、部署、完整产品验收仍开放。上轮独立 feature 裁剪的 S06/Gemini 阻碍未在本批修改或重验。未新增 UI 行为，不重复已经通过且未变更的 UI 检查；未提交、推送、出包或重启现有实例。

证据：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-commandcode-sap-scheduler-20261111`。该目录从 11 日调查沿用至 12 日收尾，包含源码快照、日志、执行回执与最终验证记录。

# LobsterAI 自动测试调度

## 本批完成

新增三项定向测试，复用原有完整 LocalRuntime 夹具、refresh worker、产品 sender 和测试策略调度器。未修改生产逻辑或既有夹具，仅注册新测试模块；全部凭据与上游均为合成、不转发的 loopback 数据。

1. **真实 timer 两轮**：自动账号策略与命名计划各执行一次，立即再次 sweep 因持久到期槽未到期而跳过；等待未修改的 60 秒 timer 与真实墙钟到期后再执行第二轮，共四次推理。不暂停时钟、不直接修改到期时间。请求停机后等待 scheduler 返回。
2. **独立结果与真实审计**：账号和命名计划分别保存结果，mode 为 automatic，policySource 分别为 account／plan:named，标准答案评分正确；四次请求固定产品路径、精确账号、配置别名、非流式及 256 tokens 上限，清除 ambient cookie。完成审计均有真实 5 tokens、精确 credential，无普通接入 Key ID；普通 Key 余额及 route revision 保持不变。
3. **失败与禁用**：HTTP 500 分别保存两份失败结果，审计用量保持未知，不泄漏 access token 或上游私有错误；同一间隔再次 sweep 不重放。关闭自动策略时零调度目标，启动前已停机时 scheduler 直接退出，两者均零推理、零审计，余额不变。

## 验证与来源

命令：`cargo test --locked --offline --lib -- lobsterai_scheduler --test-threads=2`。

**3 passed / 0 failed / 0 ignored**，测试执行 60.76 秒，确实包含实际 timer 等待；完整命令 1666.72 秒，包含共享构建锁等待及编译，不能当作纯编译耗时。

官方有效行 checker、ratchet、scoped rustfmt 与差异检查通过。模块注册 201→202 有效行，新增测试 179 有效行；未修改 baseline、例外或其他产品 lane。源码测试前后 SHA256 一致，UTF-8 无 BOM。本批 before 快照及 scoped.diff 在证据目录留存，不声明整个共享工作区的来源一致性。

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-lobsterai-scheduler-20261112`。

## 尚未验收

本批不是实际供应商、真实订阅费用、浏览器 UI、Server/Redis 或原生包验收。LobsterAI 完整进程重启后的调度槽恢复、运行中修改策略／禁用账号、自动调用中的授权轮换与刷新组合仍需独立证据；本批 500 不替代 401 刷新测试。未通过管理 API 编辑保存策略，仍由隔离配置装载。

全范围仍有 67 个工作项未完全关闭，不关闭整卡。未读取真实凭据，未提交、推送、出包、部署或修改运行实例。

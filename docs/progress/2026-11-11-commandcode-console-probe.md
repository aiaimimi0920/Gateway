# Command Code 专属控制台模型测试（2026-11-11）

状态：专属控制台模型测试子项本地通过；Command Code 整体仍为 `partial_pass`，不等同真实账号或完整产品验收。

## 改了哪些内容

- 控制台显式模型／提示词测试分发到 Command Code 专属入口，临时、命名和账号保存策略沿既有管理路径执行。
- 默认无提示词、无测试策略的连通性检查仍拒绝，不生成、不访问目录。保留管理身份、secret grant、禁用账号和网络保护。
- 仅允许已配置模型或别名，并复用账号模型准入及别名解析；使用精确账号自己的 Provider API Key，经生产 sender 发送到固定 Chat 目标。没有私有接口、隐式重试或跨账号回退。
- 响应体上限 64 KiB，总预算 60 秒；要求唯一 JSON、完整非空文本、stop、无工具／拒绝，文本最多 8192 字符。沿共享 Chat admission 检查结构及用量。
- 回答进入预览、评分与持久化前脱敏本账号完整 Key；错误仅返回固定本地诊断，不回显上游错误正文。
- 复用取消安全审计和模型健康记录；缺失用量保持未知，不扣普通接入 Key 余额。这不代表上游免费或取消退款。

## 验证

Rust：**63 passed / 0 failed / 0 ignored，其中 Command Code 30 项、SAP 26 项**；包含新增 7 项 Command Code 测试。本批 9 个 Rust 文件与编译前 `source-r1.json` 哈希一致。命令：`cargo test --locked --lib -- commandcode sap_aicore provider_runtime::model_probe protocol::openai::chat_admission --test-threads=2`。

新增 7 项用例覆盖：实际 loopback sender／精确账号／别名；回答脱敏与审计用量；坏 JSON、重复字段、空文本、length、refusal、工具调用、矛盾用量与超限；未知用量；307/401/403/429/500 不重放；取消关闭连接和取消审计；管理授权、外部账号、默认／禁用／错误模型不外发，以及单次／临时／命名／账号保存策略与结果读取。

UI：使用项目原有 `npm run test --prefix apps/desktop -- --run` 入口，CommandCodeCatalogTemplate、CredentialTestDialog、CredentialTestManager 共 3 个文件、15 项通过。首次错误使用 `npm exec --prefix` 从仓库根目录启动，未加载桌面测试环境，出现 `localStorage is not defined`；改正执行入口后通过，未修改 UI 源码或测试语义。

checker 31 项、ratchet、定向 rustfmt、scoped `git diff --check` 及 UTF-8 无 BOM 检查通过。新增生产模块分别 44 和 64 有效行，新增测试模块 131 和 157 有效行；控制台分发文件 471 → 489 有效行，仍在 500 以内。

## 边界

真实账号／费用／真实响应、自动调度的 Server／Redis 与原生程序验收、Server／Redis、独立 feature 裁剪、原生 EXE、真实浏览器视觉，以及原生 Responses/Messages、Jev、reasoning/media、ZDR 等仍开放。没有读取真实 `.ng`／凭据，没有提交、推送、出包、部署或重启。manifest 不提升为 live acceptance，67 个工作项完整关闭分母不变。

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-commandcode-probe-20261111`。保留任务前 dirty 文件备份、范围源码哈希、原始日志及执行回执；不声明整个工作树的来源。

补充（2026-11-12）：本地真实定时循环及 LocalRuntime 存储重开后的间隔保留已验证，见 [自动调度验收](2026-11-12-commandcode-sap-automatic-probes.md)；不代表 EXE 进程重启或 Server／Redis 验收。

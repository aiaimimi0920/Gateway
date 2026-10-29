# NVIDIA / OpenRouter 官方模型目录与模型池修复

## 范围与现场

用户反馈两家服务商的模型数量只有一个。本轮仅修复代码并做定向验证，不生成新手测包，
不替换、重启当前 4200 程序，不修改在线账号、凭证、启停状态或配置。
现场配置仍为 `r9-fdc20f255316`；账号保存的是种子模型，没有 `model_catalog`。

根因不是卡牌截断：卡牌统计账号 `supported_models` 的并集。
NVIDIA 存量账号没有经过新目录同步；OpenRouter 专用 `openrouter-openai` 模板又漏接目录入口，
原通用识别只接受 `openai` 旧形态。不能直接放开通用识别，因为该路径包含真实生成探测。
另外，原后端虽验证目录元数据，却没有将其投影到运行时模型池。

## 改了哪些内容

- 后端增加 `POST /v1/internal/gateway/console/openrouter-model-catalog`，复用 NVIDIA 的
  管理认证、Secret Grant、结束后授权复核、双槽并发、无缓存返回及显式账号定位。
- 两家目录读取统一在 `provider_discovery/model_catalog.rs`：仅固定官方 HTTPS `GET /models`，
  不接受自定义目的 URL、不跟随重定向，不调用生成端点。总时限 30 秒，单请求 25 秒；
  NVIDIA 2 MiB、OpenRouter 8 MiB 响应上限，模型最多 2048 项，ID 最多 256 字节。
  错误不回显上游响应正文或 Key。
- 前端共享 `officialModelCatalog.ts` 负责来源校验和合并；保留原 NVIDIA 导出供旧调用方兼容。
  OpenRouter 专用模板和官方地址的旧 `openai` 形态均使用纯目录读取。
- 新账号导入前读取目录；旧的种子账号保留 Key 保存时也能补目录，已有账号可显式刷新。
  继续沿用取消、会话、权限、草稿和 revision fence，失败不覆盖 last-good 目录。
- 同步账号、服务商并集及精确模型路由，保留手工模型、其他账号/服务商、路由优先级和禁用策略。
  官方目录刷新替代旧的生成识别意图，移除该账号遗留的 `discovery_job`，不启动新的生成探测。
- 配置编译时将经验证的目录 ID 合并到运行时账号和服务商模型池，覆盖仅保存元数据的兼容入口；
  不改变原始文档、Key、账号启停、授权及既有 manual fallback 策略。
- 模型卡牌使用两家目录证据，未调用项标为“目录已发现”，不伪造 `verified_models` 或健康成功。

## 联网目录核实与离线回放

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-catalog-pools-20261026`。
公开 metadata GET 均返回 200：NVIDIA 85 项、OpenRouter 467 项。未携带认证、未发生成请求。
目录数量是该次联网响应，不是生产代码常量，也不证明账号有全部模型权限或全部生成成功。

随后直接加载生产 TypeScript 合并器，离线回放上述真实目录：两家全部 ID 均进入测试账号、
服务商模型池和对应精确路由；重复合并幂等，手工模型、禁用账号状态保留。
这不是在线配置迁移，也不是桌面原生手测。

## 验证记录

- 首个失败回归证明专用 OpenRouter 模板没有请求目录；接入后通过。
- 交叉审查发现旧 `discovery_job` 未清理；增加失败回归后修复，共享合并器负责清理。
- 最终前端定向 16 文件、92 测试通过，TypeScript 通过。
- Rust 目录相关单元筛选 43 测试通过，NVIDIA/OpenRouter 管理接口合同各 1 项通过。
  公共目录 transport 的 mock 断言 POST 次数为 0，覆盖重定向、来源、响应预算和错误保护；
  配置重载覆盖多模型投影、手工模型、Key、禁用凭据及禁用路由保留。
- 上一轮工具 API/别名审计合同 4 项、实际 attempt 归属单元 1 项补跑通过；修正了旧合同测试的
  `summarize_request_audit_rows` 模块引用，未为此更改生产行为。
- Rust scoped formatter、scoped diff、UTF-8 无 BOM 检查通过。有效行 checker 测试 31 项通过，
  ratchet 通过；本轮涉及源码均低于 500 有效行，没有添加例外或重建基线。
- 编译存在 unused/dead_code 等警告，但没有编译错误；未为清警告扩改其他功能。
  最终 `*.receipt.json`、日志及 `source-final.json` 保存在上述证据目录。
- 首轮后端目录筛选 41 过、1 失败：新增测试错误地假定没有静态匹配就不允许 legacy manual
  fallback。没有改变现有 fallback；测试改为验证禁用凭据永不被选，保持目录不是权限证明的边界。
- 共享工作区其他 AI 同期扩展了 Poe 的后端目录类型及端点，本轮保留其改动。
  本记录的业务范围与公开目录回放仅为 NVIDIA/OpenRouter，不代表 Poe 已完成真实验收。

## 生效边界

未执行全量回归、批量模型生成测试、真实凭证逐模型验收、原生桌面手测或 EXE 打包。
当前旧 EXE 与在线配置未更新；新版使用现有账号刷新目录或保存后，经正常保存/CAS 链生效，
不需要删除服务商、重建账号或更换调用 Key。

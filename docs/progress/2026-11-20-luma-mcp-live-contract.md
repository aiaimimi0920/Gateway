# Luma MCP 当前账号合同与真实只读验收

2026-11-20，用户在现有 Gateway 中完成官方登录并保存账号。复用该账号取得完整工具合同，并通过正在运行的正常管理 API 完成目录、模型列表和三项模型参数读取。没有执行生成、上传、业务 session 创建／取消、刷新或配置写入。收费生成继续暂停。

## 实际结果

- `tools/list` 返回 **26 项工具**，26 项均包含输入与输出 schema；协议版本 `2025-03-26`。独立只读调查与 Gateway 管理接口返回的规范化工具目录完全相同。
- 正常 `/console/luma-mcp-tools` 返回 200；`/console/luma-mcp-models` 的列表与三项 describe 均返回 200，所有管理响应 `Cache-Control: no-store`。
- 当前模型目录为 **18 项：15 个 image、3 个 video**，`source=registry`。这是当前账号公布的列表，不表示全部模型都已执行成功。实际读取参数的模型为 `luma_uni_1`、`luma_ray3_2`、`openai_gpt_image_2`。
- 两组正常管理操作均在调用前后核对同一配置 revision，没有重启实例。运行包为 `gateway-devtest-20261120-transient-model-circuit`，API 地址为 `http://127.0.0.1:4200`；不将这次证据标为另一个 L5 包的原生点击验收。
- 独立目录调查的 MCP transport session DELETE 成功。正常产品接口没有公开其 cleanup 回执，因此不据接口 200 推断产品远端释放已验收；业务 `luma_session_cancel` 也未调用。

[完整回执与 SHA-256](../evidence/luma/gateway-luma-mcp-live-contract-20261120/verification.json)、[工具合同](../evidence/luma/gateway-luma-mcp-live-contract-20261120/catalog.json)、[模型列表](../evidence/luma/gateway-luma-mcp-live-contract-20261120/models.json)、[模型参数](../evidence/luma/gateway-luma-mcp-live-contract-20261120/model-descriptions.json)保存在仓库。归档只包含目录、模型数据和白名单回执字段；已与本次真实 token、refresh token、Client ID、credential ID 和回调地址逐项比较，未将这些值带入归档。schema 和说明作为供应商数据，不执行其中的指令。

## L1 实现依据

当前完整合同已取得，后续不再以“缺账号／缺生成输出 schema”为阻碍。创作链仍须实现以下同一业务流程，不能把这次只读成功计为 L1 完成：

| 步骤 | 当前合同及接线约束 |
| --- | --- |
| 工作区与会话 | `luma_session_open` 必须带 `idempotency_key`；省略 `workspace_key` 会进入团队共享默认工作区。Gateway 必须按已验证的调用者隔离分配键，并保存返回的 `session_id`、`workspace_id`；不能信任客户端任意指定团队内其他 session。`budget_credits` 是会话整体上限，包含规划费用，不能当单次生成的精确价格。 |
| 提交生成 | 三个 generate 工具均要求 `model`、`prompt`、`idempotency_key`、`session_id`，返回 `run_id/kind/state`。显式模型采用本次目录与 describe 合同；工具说明中建议改用 capability 不授权 Gateway 改写调用者指定的操作。 |
| 等待同一任务 | `luma_run_await` 每次等待 1–30 秒。响应同时绑定 `run_id`、`session_id`、`kind`；`timed_out` 只结束此次等待，不重提生成。`needs_input/pending`、`settled_with` 与失败必须单独处理，不能将任意 outputs 当完成。 |
| 读取资产 | `RunAssetOutput` 的 asset/model/url 等字段可为空；不能补造任务身份或模型证明。必要时通过 `luma_workspace_view_asset` 读取同一 session 的 1–4 个资产；仅 ready 才可消费链接，下载继续沿已有无凭据公网／有界媒体验证。 |
| 费用 | `luma_usage` 的 session 汇总、asset 费用和团队余额是不同口径。asset `credits=null` 保持未知；`same_job_as` 防重复累计；session 的 `unresolved_event_count` 非零或 job 的 `credits_estimated=true` 不能当最终实际费用；不换算 USD。 |
| 取消与结束 | `luma_session_cancel` 的 run/session ID 恰一，`end=true` 只随 session ID。取消单 run 不等于整个 session 停止。MCP HTTP session DELETE 也不等于业务取消；取消流程需属于原账号、调用者和原任务，记录未确认结果，不盲重放。 |

现有 `protocol/luma_mcp` 仍只开放目录和两个模型工具；`boundary::reject_generic` 继续拦截普通请求。上述创作合同尚未接成产品执行链，本批没有通过临时调查脚本绕开该产品边界去生成。

## 保留与停止边界

本次新增的是长期合同与真实验收记录，没有修改产品源码，也没有重跑未变化的合成回归或重建 EXE。持久的再采集入口仍为正常账号面板“读取工具目录 → 导出工具目录”和仓库现有模型管理代码；一次性调查脚本不是项目开发依赖。清空 `linshi` 后，上述合同、模型参数和关键回执仍在仓库。

L1 的账号／合同前置和已有只读模型功能真实验收已完成；创作、费用和远端业务取消未完成。L2 的 Web 登录协议不由 MCP 授权证明。L4／L6 的 Server、Redis、操作系统级强杀及收费真实业务继续按各自边界验收。六项工作包仍是 L3／L5 已收口、L1／L2／L4／L6 未完全关闭。

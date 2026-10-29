# Linkup 标准模板真实验收（2026-10-26）

## 结论与边界

Linkup 已按 You、Exa、Tavily 的同一新流程完成 Search 模板真实验收，并登记
`flowVersion: 1` 蓝色完成标记。通过真实源码 UI 创建空凭据池、导入用户提供的 Key、
运行精确账号搜索测试，再完成普通 Gateway query/q 搜索、Fetch 与 Balance 查询。
保留原有 `linkup-research` 路由，但本轮未创建 Research 付费异步任务，也不声称
Research/Tasks/Extract、全部参数组合、自动轮询或长期稳定性已通过。

后端仅绑定 `127.0.0.1:4300`，源码前端为 `127.0.0.1:1435/ui/`；未停止、接管或修改
用户的 4200 实例。Key 仅存受限 private 文件和隔离数据根，没有写入源码或公开证据。
原 `.ng`、`.ng-devtest-4300` 中的 Linkup provider 与 routes、仓库 routes 哈希保持不变。
没有生成桌面 EXE 分发包、正式发布、提交或推送；源码完成不代表用户已打开的旧 EXE 自动升级。

## 红测与最小修复

原模板仍继承聊天测试的 `expectedAnswer: OK`，后端也没有将 Linkup 接入搜索 probe。
前端红测捕获错误默认策略；后端红测准确返回 Unsupported，而非拿旧实现请求真实上游。

- 前端默认 Selected + `linkup-search` + 无标准答案的“搜索连通”，不计算聊天 IQ 分数。
- 搜索 probe 增加 Linkup preset，映射后仍只接受精确 `linkup-search`，不测试其他操作 alias。
- 复用 prepare/planner/headers/profiled-send、Bearer、`POST /v1/search`；`query` 规范化为 `q`，
  固定 `maxResults: 3`、`depth: standard`、`outputType: searchResults`、`includeImages: false`。
  planner 移除本地路由 model，无新增重试、跨账号或跨服务商 fallback。
- Linkup 顶层 `results` 使用 `name` 而非 `title`；只有非空 name 与 HTTP(S) URL 才算有效结果。
  保留 60 秒超时、1 MiB 响应上限；空/坏/错形状/超限或包含错误对象的 HTTP 200 不算成功。
  认证、额度、限流等失败仅返回安全摘要，不回显上游错误正文、网页片段或凭据。
- 固定 Base URL、三适配器、健康源保持原合同；模板只建空池，Key 从独立账号界面导入。
  原有 Fetch/Research/Balance 配置不变，不向所有 endpoint 注入搜索必需参数。

本轮已联网核对官方 [Search 合同](https://docs.linkup.so/pages/documentation/endpoints/search/reference)、
[Fetch](https://docs.linkup.so/pages/documentation/endpoints/fetch/overview)、
[Balance](https://docs.linkup.so/pages/documentation/endpoints/account/balance) 和
[错误合同](https://docs.linkup.so/pages/documentation/platform/errors)。

## 真实 UI 与精确请求证据

UI 路径：添加服务商 → 标准模板 Linkup → 添加 → 添加账号 → 保存 → 测试账号 → 运行测试。
空池 revision 为 `r1-d3904d523f9f`，导入后为 `r2-3b32c7cfbcc6`；回读 Key 与用户材料完全一致。
provider 为 `linkup`，credential 为 `linkup-cred-0`，账号名称为“Linkup 验收账号”。
没有以 SQL 或直接管理 API 代替 UI 创建和导入。

| 验收项 | 实际结果 | 归属 |
| --- | --- | --- |
| UI 搜索连通 | Passed；3 条有效结果；0 失败、0 未执行；1552 ms | linkup / linkup-cred-0 / linkup-search |
| POST /v1/search，query | HTTP 200；1 条有效 name/URL；1828 ms | 同一账号 / linkup-search |
| POST /v1/search，q | HTTP 200；1 条有效 name/URL；1672 ms | 同一账号 / linkup-search |
| POST /v1/fetch | HTTP 200；5235 字符非空 markdown，包含 Rust；797 ms | 同一账号 / linkup-fetch |
| GET /v1/credits/balance，前后各一次 | 两次 HTTP 200，均为有效非负数字 | 同一账号 / linkup-balance |

UI 测试 `checkedAt=2026-10-25T18:49:18.0558535Z`，审计
`probe-41528f1c-73f6-4a55-9229-73f3fd37b6a0`。`gradedCount=0`、`score=null`、
`expectedAnswer=null`、`healthRecovery=applied`；只保存“Search returned 3 web results.”摘要。

普通 query/q/Fetch/Balance-after 的响应 request ID 依次为：

```text
a2e14045-6642-4f55-9048-69b94fdf002c
7f4c6306-99ec-4d92-8e8b-d101cda28f8e
0b599865-01aa-4832-b0f2-24bef8cb66e0
b61d4915-772e-4419-bdbf-c13b0da0674e
```

对应审计依次为 `gw-audit-c2eda439-bf78-4e56-b166-d23b75accbc6`、
`gw-audit-01e3f7ad-0d58-43bb-a7bd-8b68e047e514`、`gw-audit-ee915dc2-4c9a-4a2f-981c-9fe0a5937763`、
`gw-audit-da132fcf-5f7b-492f-8bf0-68fc87955aad`。6 条验收审计全部 upstream 200、completed、
`protocolFamily=search`、`realCredentialRef=linkup-cred-0`；5 条普通请求全部
`routeTrace.routeAttemptCount=1`、`candidateCount=1`、`attemptedProviderIds=[linkup]`、
`selectedPipelineMode=direct_http`、`selectedProtocolProfile=linkup`，没有借其他账号兜底。

Balance-before 于旧 Tavily API 构建执行，request ID 为
`1d110c5c-0342-4fde-8df7-23e5b30d1e94`，审计为
`gw-audit-171f999c-dbc4-498a-b3c6-65ccb75b453c`；其余 5 条来自本轮修复版 EXE。
前后观测余额下降 `0.016`，绝对余额只保存在 private。此差值不是精确请求计费，
无法排除同 Key 其他活动，不能把它换算成确定费用。通用 quota owner 未得到可识别单位，
UI 额度观测仍为 unavailable，未虚构 consumed 或 USD 费用。

## 定向验证与来源

- Rust Linkup、Tavily、Exa、You 四个搜索合同各 1 项全部通过。Linkup 覆盖 16 次精确 loopback
  出站，校验路径、Bearer、q/必需参数、移除 model、401/402/403/429/400/500、安全错误、
  空/坏/超限/非法 URL/错误 title 字段/错形状、禁用账号、模型 alias、无评分、审计和健康恢复。
  测试使用合成凭据、随机回环端口和 linshi 临时目录，不调用付费上游。
- 前端 7 文件、24 项、TypeScript、定向 rustfmt、31 项有效行 checker 测试、ratchet、
  `git diff --check` 在修复后通过；完成登记后再次执行对应检查，日志单独保留。
- 官方 `tools/build-gateway-devtest.ps1 -Edition api -Jobs 4` exit 0；Cargo 构建报告 10m10s，
  保留既有 warning，来源前后指纹与产物哈希核对通过。不是正式 Release。
- API devtest EXE 为 `target/devtest-editions/api/gateway.exe`，SHA-256：
  `403a53bce835a5b4e9f528bc6e1d85119b2591ae834b1850142b942496609fa1`。
  构建源码指纹：`5007684a80663950e241f57382fdb179bb05ba3faa8b96e2b48454836eb2b747`。
  蓝色登记和验收文档在真实成功后才写入，不把旧构建凭据冒充最终桌面分发包凭据。
  Balance-before 的旧 EXE SHA-256 为
  `9478be57b27e7b8002346f8b3b7a6a8bd27611cbfeec8e242aa8a75f5a3e38b0`。

有效行：共享 probe 202→214，workflow 94→94，registry 41→46，完成列表测试 93→94；
新增 Linkup UI 测试 42、Rust 合同 236。preset 207 和搜索 planner 418 均未修改。
无新增超限文件、无超限历史文件增长，也未申请例外。未执行全量 Cargo 回归、
其他服务商付费上游测试或正式发布安全审计。

公开脱敏证据目录：
`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-linkup-adaptation-20261026/`。
包括红绿测试、构建凭据、请求摘要、审计、完整性、UI 截图与机器验收回执。
隔离数据库及私有导入文件保留，不得公开 private 目录。

## 收尾复核

完成登记后，前端 7 文件、24 项，TypeScript、定向 rustfmt、31 项 checker 测试、有效行
ratchet 和 `git diff --check` 再次全部通过。真实源码 UI 中 Linkup 已呈蓝色并具有
“新调用流程已验收”可访问说明；截图 `ui-template-complete.jpg` 中的 `linkup-2` 只是再次
打开目录后的防冲突预览草稿，已关闭且未保存，隔离配置仍为单一 provider/credential。
`ui-live-success-final.jpg` 保存该账号搜索 3/3、Fetch 1/1 和整体 6/6 的成功总览。

`acceptance-verified.json` 核对 6 条成功审计、精确凭据、无评分、普通请求单次路由、
SQLite integrity=ok、原数据和 routes 完整性。窄改动审查确认没有新增认证目标、
动态路径拼接、后台循环或不受限队列；新增分派与结果字段选择沿用现有安全发送和取消边界。
本轮 8 个源码/文档文件均为 UTF-8 无 BOM，并检查其与公开文本证据未包含本轮 Key、
管理 token 或调用 token。未回退、清理或提交其他未提交内容。

隔离后端通过管理 drain 返回 HTTP 202 后退出；核对精确 PID、EXE/脚本身份后关闭本轮
UI helper。`2026-10-25T18:55:11Z` 复核 4300/1435 无监听，用户 4200 `/healthz` 为 HTTP 200，
原有两个 4200 listener 的 PID 保持 2768/13020。保留隔离数据、证据和 API devtest 产物，
没有覆盖用户桌面包，也没有清空任何用户数据。

# Gemini Web 模板复核与隔离验收（2026-10-29）

## 当前状态

**本轮标准流程真实验收已通过。** 用户完成专用窗口登录及巴黎验证后，复用本轮保留状态，
已在正常账号库 UI 自动采集并保存账号，精确账号 UI 测试、普通非流式与 SSE 调用全部通过。
三条调用审计均为同一服务商、同一凭据的 completed / HTTP 200；两个普通 relay 均只有一次路由尝试。
已据此登记 `TEMPLATE_ADAPTATION_RECEIPTS` 蓝色完成标记，不需要再次登录或重放生成请求。
最终安全汇总为证据目录中的 `acceptance-final-safe.json`；后文保留失败历史，各时间检查点不代表当前状态。

## 合同与模型边界

- 模板保留 `gemini-web-chat` preset，三项策略为 `auto / session / gemini_web`；新建时为
  `explicit_credentials_only: true`、空账号、空模型，不合成可调用默认账号。
- 认证沿现有专用浏览器采集器取得 Google 会话，再由已有 Gemini Web HTTP 执行器完成
  app bootstrap、StreamGenerate form POST 和响应转换；不是 Google 官方 API Key 通道。
- 成功导入后的 `gemini-web-chat` 是**捕获会话的本地调用别名**，不是 Google 在线模型目录，
  也不证明底层实际模型版本或全部模型权限。导入不写 `model_catalog` 或 `verified_models`。
- 本轮联网读取了 [Gemini 网页](https://gemini.google.com/) 和
  [Google 官方使用说明](https://support.google.com/gemini/answer/13278668?hl=en)。
  公开说明不构成对内部 StreamGenerate 协议稳定性的承诺。

## 真实 UI 复现与修复

1. 在独立数据、后端 `127.0.0.1:4300`、源码 UI `127.0.0.1:1435/ui` 中正常选择模板、
   查看三项只读策略并创建 `gemini-web` 空池，revision 为 `r1-9e27c41e83e5`。
2. 正常打开账号库、“手动添加”，实际却显示 `目标族 gemini-canvas-chat`。根因是合并
   Gemini 卡片时覆盖了所选主服务商的采集协议；现保留 `primary.manualAddFamily`，同时
   保持历史 Canvas 合并池的既有主服务商行为，不拆卡片、不迁移账号。
3. 修复后在同一真实 UI 重验，已显示 `目标族 gemini-web`，打开正确的专用 Edge 窗口。
   登录仍由用户完成，采集器要求发送“法国的首都在哪里”，并检查成功响应含“巴黎”。
4. 导入原先仅增加凭据，未声明模型或路由。现仅对 Web preset 在缺少已有模型选择时声明
   会话别名，沿现有 `applyModelSelection` 同步账号、服务商及精确路由；保留已有模型来源、
   显式空选择、禁用路由、其他账号、其他服务商和别名，重复应用保持幂等。
5. 精确账号测试原先在本地返回 `unsupported`。现接入已有生产 dispatcher 和 Gemini
   prompt probe，不重新选路或回退账号；沿原有授权、审计、健康恢复与 60 秒超时。
   要求 canonical 非空文本且 `finish_reason == stop`；返回前额外脱敏 Web 会话字段。
6. 实际采集后 UI 刷新稀疏，进一步定位到轮询依赖 `updatedAt`：等待状态不变时，第二次读取后
   不再安排下一次读取；网络读取失败也会停止轮询。已用失败回归复现，并按每次有效读取结算重排
   1.5 秒计时器，保留关闭、卸载、旧会话及手动完成的 generation/sequence 防护。
   新源码 UI 的真实请求列表已观察到同一 waiting session 连续超过 20 次成功轮询，不靠状态变化续排。

顶部通用“添加账号”表单未扩展为 Web 登录入口，本轮沿账号库既有“手动添加”流程。
没有修改 Gemini upstream/protocol owner、日常 4200 实例、其数据或凭据。

## 首轮本地验证及失败历史

- UI family 回归改前 2 项失败、改后通过；模型别名回归改前失败、改后通过。
- 首轮前端 10 文件 **53/53** 通过，包括重复导入保留性、旧 Canvas/Business 登录流程及轮询修复。
  两个旧页面测试的按钮断言同步为现行 `-web` 显示名，未改动生产命名规范。
- Gemini Web 精确账号回环合同与相邻 Gemini API 合同 **2/2** 通过。Web 合同验证 Cookie
  身份、bootstrap/form、401、空白、畸形、未完成回复、禁用账号、未知模型、无显式选项的
  单账号入口及公开结果脱敏；观察到的请求序列无账号回退或隐式重试。
- 首轮 Web 红测为 `unsupported`；第一次 green run 的最后一项误将无 prompt 的账号列表
  发给严格 provider 输入合同，取得 400。已改为真实单账号无 body 入口后重验通过，未放松输入校验。
- 既有 Web exporter Node 测试 **4/4**、TypeScript、scoped rustfmt、有效行检查器 **31/31**、
  ratchet、`git diff --check`、修改源码 UTF-8 无 BOM 检查通过。
- 本轮改动的源码均不超过 500 有效行。主要 owner 前后为：账号分组 `435→435`、导入
  `79→93`、probe 分派 `334→336`、Gemini probe `119→136`；新增前端测试 82、Rust 合同 144。
  轮询 owner 为 `213→219`、对应测试 `142→182`。
- 首次正确 Web 采集 session `d2794942-3483-4035-80f2-af2df88c58de` 最终为 failed、0 草稿，
  错误为等待精确验证问题超时；用户已告知看到正确回复。该证据不能说明 Google 登录失败，
  也不足以确认验证请求为何未被采集，不能仅把它归因于 UI 轮询。失败摘要已保留。

上述首轮本地检查均为 UI、离线及回环证据，本身不等于真实 Google 账号调用成功。Gemini Web 现有流式执行器
先读取完整原生响应再生成下游 SSE；本轮不能声称逐 token 透传，也没有独立证明所有原生终止/截断情况。

## 首轮构建与当时未完验收（历史检查点）

已用 `tools/build-gateway-devtest.ps1 -Edition api -Jobs 4` 完成构建，Cargo 实测耗时
13 分 32 秒。冻结 API devtest 为 80,133,632 bytes，SHA256：
`8d9d3d6a885558db8cee9f874564e928f158f3cf0d724027bf5bc810d813026f`。
构建源码指纹为 `b04362a6b7b612733f01c478f426e4b01d949e4f480cce5b90e1cce931864799`，
builtAt 为 `2026-10-29T08:56:20.7479834+00:00`；构建早于本证据文档和随后追加的前端轮询修复。
API 版不嵌入管理 UI，轮询修复由独立源码 UI 提供；不能宣称该修复已嵌入桌面 EXE。
清单、来源与冻结副本 hash 已核对，不是完整桌面手测包或正式 Release。

在确认旧采集进程已退出、失败状态已保留且隔离配置仍为空池后，仅停止本轮基线 4300，
切换到冻结修复版，`2026-10-29T09:00:56Z` healthz=200，数据仍是独立 `private/runtime`。
第二次正确 Web session 为 `67b9dc38-5ed6-4b09-a059-3c138f58b24b`，最终也超时且没有草稿；
用户已完成登录和问题，不能归因为用户登录失败。日常 4200 只读 healthz=200，未停止或重启。

该检查点当时仍需：使用本轮已登录状态正常自动导入 → 在新冻结二进制上执行精确账号 UI
测试 → 普通非流式与 SSE 各一次 → 对账真实 provider/credential、终态与上游尝试 → 再决定登记完成。
真实调用脚本保留独占 started guard；恢复时先对账，不重放已开始的请求。

证据根目录：`linshi/gateway-gemini-web-template-acceptance-20261029-r1/`。
关键文件为 `ui-red.json`、`models-red.json`、`frontend-final.json`、`polling-red.json`、
`polling-green.json`、`auth-session-initial-status.json`、`probe-red.log`、
`probe-green-r2.log`、`exporter-tests.log`、`typecheck-green.log`、`ratchet-final.log`、
`source-review.json`、`build-verified.json`、`runtime-fixed.json` 和真实 UI 快照。管理材料、隔离数据和冻结 EXE 位于
`private/`；不应发布运行数据或凭据。没有提交或推送代码，也没有替换日常 4200。

## 新网页传输与 Cookie 补齐（11:48 UTC 检查点）

- 真实网页问题改走 `geminiweb-pa.clients6.google.com/v1/processSession` WebChannel，
  有界 `req1___data__` Base64 数据中可确认精确问题；原采集器只等旧 `StreamGenerate/f.req`。
  这是两次已看到巴黎但没有导入的直接采集缺口，不靠延长超时解决。
- 新 `gemini-web-capture-compat.mjs` 只把新传输当触发条件，仍须向固定 Gemini origin 的
  旧 StreamGenerate 接口进行一次真实验证；只解析 `wrb.fr` 的助手候选，不接受 HTML 或错误中的巴黎。
  新 `gemini-web-capture-session.mjs` 拥有监听、超时、关闭及至多一次验证生命周期，禁止隐式重试和重定向。
- 实际登录路径为 `/u/0/app`，初版恢复漏掉 `/u/0/_/BardChatUi/data/batchexecute`。
  已补根路径及可选数字账号前缀的限定识别与回归，不假设固定账号索引。
- 仅两项主次 Cookie 的恢复尝试失败，底层原因当时未保留，不能声称某项 Cookie 已独立证明必需。
  改为保留目标域可用的 Google Cookie 集合后，独立修复采集器实际 exit 0：HTTP 200、助手候选含巴黎、
  `captureMode=legacy_http_validation`，主 Cookie、完整 Cookie 头和 bootstrap 会话字段均已取得。
  证据为 `capture-full-cookie-safe.json`，原始材料仅存本轮 `private/`，不输出或提交。
- Cookie 集合限定 Google/Gemini 域、合法名称和值，最多 64 项和 16 KiB。Rust 解析保持旧格式可读，
  通过 `headers/Cookie` 的 secret replace patch 传递，不放入普通 credential headers 文档；
  前端合同、schema 和 draft 应用同步支持该字段。生产 header owner 本身已支持 supplemental Cookie，未改动。
- Node 定向测试 **11/11** 已通过；Cookie secret patch Rust 回归已先真实复现失败，修复后 auth manager
  定向测试 **14/14** 通过（本次编译 14 分 22 秒）。Web 和相邻 API probe 合同 **2/2** 通过；
  移除新测试的无用 import 后 Web 合同再次 **1/1** 通过。前端最终相关检查 **27/27**、TypeScript、
  scoped rustfmt、检查器 **31/31**、ratchet 和 `git diff --check` 通过。
  新采集模块有效行 79/89，auth manager 498→500、draft 179→191、parser 254→263、测试 304→322；
  全部本轮源码不超过 500 有效行，UTF-8 无 BOM 检查通过，详见 `source-review-cookie-r2.json`。
- 显式 `GEMINI_WEB_CAPTURE_STORAGE_STATE_PATH` 仅供本轮隔离恢复，默认未启用。
  恢复仍做真实 HTTP 验证，不把已有 Cookie 当作成功；无需用户再次登录。
- 4300 当前仍是旧冻结 r1 EXE，不包含新 Rust Cookie 接线；下一步须构建、冻结、核对新 API devtest 后
  仅切换本轮 4300，同时刷新仅本轮源码 UI。未发普通 buffered/SSE 调用，未登记蓝色 receipt。
- 回复体 1 MiB 检查发生在 Playwright 读取后，不宣称预分配内存准入；本轮不扩展为新 WebChannel 执行器。

## stdout/stderr 解析与错误脱敏补充

- r2 官方 API devtest 构建成功，Cargo 耗时 15 分 21 秒；SHA256 为
  `ac3eae1688c4bbdcc659a3ffa0e1e2cc366d7b23aa0ccdcf16997c61b6258af9`。
  首次失败是原生源码缓存目录缺失，已由仓库锁定归档恢复，未更新依赖或删除旧缓存。
- 在真实 UI 账号库点击“手动添加”，session `0f91ae76-def4-4e2a-90ac-51ecf13e008f` 的
  helper 已返回成功 JSON，但后端把 stdout 与 stderr 诊断拼接，无法提取结尾 JSON，最终 failed、0 草稿。
  本次 stderr 来自为诊断采集器而附加的本地 inspector；不能把此次失败归为用户或 Google 登录失败。
  inspector 已随本轮 worker 结束，后续不附加调试器干扰验收。
- 原失败消息还会回显原始 stdout，存在暴露会话片段的风险。本轮只修 Web owner：分别从 stdout、stderr
  提取 JSON（stdout 优先），解析失败返回固定错误，不再回显原始输出、payload 或 serde 类型错误原文。
  原始材料不得进入进度文档、普通 receipt、版本库或交付包。
- 新 `gemini_web_capture_parsing_tests.rs` 覆盖有效 stdout 加 stderr、畸形材料脱敏、Cookie CRLF/大小限制、
  worker 失败不能导入。使用隔离小型 Rust harness 直接加载生产 parser、原样 capture types 和该测试文件，
  所有依赖版本与仓库锁文件一致且 offline：先 **2 失败 / 2 通过**，修复后 **4/4** 通过。
  这是定向 production-parser 源码合同，不冒充重新执行整个 Gateway lib 测试；下一步官方 API 构建覆盖完整编译。

## 最终导入保存边界修复

- r3 官方 API devtest 构建通过，Cargo 实测 **6 分 59 秒**，包含 Cookie 接线与本地 helper
  stdout/stderr 分离、错误脱敏修复。冻结 EXE 为 80,113,152 bytes，SHA256：
  `cf58d556cd8b4ca421002657c76f5160939eb2049bbc34bd2760df6f112a2751`。
  builtAt 为 `2026-10-29T14:31:43.4436648+00:00`，构建源码指纹为
  `5a1a4e9172b73956aea80d04596434791a40723f479597ecc674530d3b946e41`。
- 在 r3 上，session `eecd98c9-5a7f-44fc-bcb5-6a7f5256f0a3` 采集 succeeded，保存却返回
  HTTP 422 / `secret_value_must_use_patch`：普通草稿仍包含空 `api_key`、`auth_token`、Cookie 占位，
  `extra_body.accessToken/sessionId` 也未走 secret patch。这不是网页登录失败。
- 新增 Web 专用 `geminiWebDraftSecrets.ts`，提交前移除普通文档的敏感字段/占位，将两项会话字段
  交给既有 secret edits；不修改原始 auth response，不改变 Canvas/Business 导入行为。
  回归先红后绿；相关六个测试文件 **37/37**、TypeScript 通过。
- 该前端修复晚于 r3 API 构建；最终真实 UI 使用 `127.0.0.1:1436/ui` 的新源码快照。
  不能把 r3 称为包含最新 UI 的完整桌面手测包，也不能声称当前 whole working tree 仍匹配其构建指纹。
  本轮没有执行完整桌面打包或正式 Release。

## 最终真实 UI 与普通调用验收

- 正常 UI 建空池后，沿账号库“手动添加”自动采集并保存；成功 auth session 为
  `3f1cd81d-2297-4d7d-9b23-f87fe68a728d`，1 个草稿，保存 HTTP 200，活动 revision
  `r2-d0b555648b26`。服务商 `gemini-web`、凭据 `gemini-web-manual-1793285135`。
  普通提交文档已移除敏感占位/字段；五项 credential secret patch 为 `api_key`、`auth_token`、
  `headers/Cookie`、`extra_body/accessToken`、`extra_body/sessionId`，不记录其值。
- 验收模型均为本地会话别名 `gemini-web-chat`，并未伪造在线目录或底层模型版本证据。

| 验收入口 | 实际结果 | 安全对账信息 |
| --- | --- | --- |
| 精确账号 UI 测试 | passed，非空，精确 `OK` | `probe-c556b3d8-6cea-41a5-809f-ce0749c14989`；14:47:33Z–14:48:09Z |
| 普通非流式 | HTTP 200，3 字符非空完整回复，`finish_reason=stop` | 47,266 ms；`gw-audit-92ba0af4-d220-4791-bfe4-ee0a3733eef2` |
| 普通 SSE | HTTP 200，精确 `OK`，`finish_reason=stop`，`[DONE]` | 36,047 ms；`gw-audit-82c8e628-0569-4b0b-bc66-7dab33b74341` |

- 非流式安全结果中 `exactOk=false`，没有保留回复正文；它通过的是非空完整回复条件，不能称其也精确返回 `OK`。
- `audit-final.json` 恰好三条标准验收调用，全部 `completed`、`upstreamStatus=200`，精确身份一致。
  两个普通 relay 均 `routeAttemptCount=1`、`attemptedProviderIds=["gemini-web"]`；UI probe
  的这两项为 null，不伪造尝试计数。采集器另有独立 HTTP 兼容验证，“三次”不代表全轮 Google 请求总数。
- SSE 客户端 chunk id 与 audit responseId 不同；按唯一 stream 请求、时间窗口、精确身份和已消费
  started guard 对账，不宣称两个 id 相等。buffered responseId 与对应 audit responseId 一致。
- 最后一次完成为 `2026-10-29T14:51:21.5147534Z`。只据已有证据生成安全汇总并登记 receipt，
  不重新触发 UI probe、buffered 或 SSE。原始 Cookie/token、profile、管理材料与运行数据始终留在 `private/`，
  不进入文档、普通 receipt、版本库或交付包。
- 登录/采集依赖专用浏览器；生成调用使用已有 Rust HTTP executor。SSE 先聚合再桥接，
  不是逐 token 透明转发，也未验收所有模型、全部工具、多模态、会话刷新或长期稳定性。

最终证据另含 `ui-import-and-probe-final-safe.log`、`ui-probe-passed.png`、两种 `relay-*.json`、
三项 `*.started.json`、`audit-final.json`、`acceptance-final-safe.json`、
`frontend-secret-separation-final.json`、`parser-diagnostics-locked-red.log`、`parser-diagnostics-green.log`、
`build-verified-r3.json` 与 `runtime-cookie-r3.json`。日常 4200 未替换、未重启，未提交或推送代码。

## 收尾验证

- 验收标记回归先 **9 通过 / 1 失败**（缺少 Gemini Web receipt），登记真实证据后，七个直接相关前端
  测试文件 **47/47** 通过；包含蓝色状态、技术字段只读、Web 导入 secret 边界、轮询、Canvas/Business
  及相邻 ChatGPT 材料保护。TypeScript 通过。
- Node 采集器定向 **11/11**、scoped rustfmt、有效行检查器 **31/31**、ratchet、`git diff --check`
  通过。ratchet 扫描 3,621 文件，governed source >700 为 0；分类后的 immutable runtime artifacts 单独计量。
- 最终 31 个本轮及相关源码/文档通过 UTF-8 无 BOM 检查，源码最大 500 有效行。auth manager
  `498→500`，parser 最终 251，新 Web secret helper 27、parser 回归 44；验收登记 `116→121`、
  目录 UI 测试 `114→115`。没有修改 baseline 或制造超限豁免，详见 `source-review-acceptance-final.json`。
- `2026-10-29T14:59:44.6028178Z` 只读核实 4200、4300 的 `/healthz` 均为 200，未运行 Cargo/Rust
  构建进程。本次收尾没有重发真实生成，没有替换日常实例，也未执行全量回归或正式发布审计。
- 新增安全文件：`frontend-acceptance-final.json`、`receipt-red.json`、`typecheck-acceptance-final.log`、
  `node-acceptance-final.log`、`rustfmt-acceptance-final.log`、`checker-acceptance-final.log`、
  `ratchet-acceptance-final.log`、`source-review-acceptance-final.json` 与 `runtime-acceptance-final-safe.json`。
  1436 源码 UI 已关闭 watch，蓝色 receipt 由本次真实 UI 组件测试验证，运行中的旧快照不会自动刷新该标记。

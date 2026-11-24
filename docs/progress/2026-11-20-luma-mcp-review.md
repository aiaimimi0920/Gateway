# Luma MCP 显式 review 答复

本批推进 L1 的用户交互子项：Local 管理台可显示原任务的 review 内容、图片／视频样本、回答 JSON schema 和 revision targets；用户填写 JSON 并逐次确认后，向原 run／invocation 发送一次答复，随后按送达回执的 seq 读取会话进展。未调用真实 Luma，未刷新账号，未运行收费生成；当前 4200 实例和旧包未更新。

## 实现与合同

合同来自已归档的 [26 项工具快照](../evidence/luma/gateway-luma-mcp-live-contract-20261120/catalog.json)：`luma_run_await.pending`、`ReviewInteractionOutput`、`luma_run_respond` 与 `luma_session_await`。`verdict_schema` 是任务返回的开放对象，不包含固定的通过／修改字段，故本批不虚构按钮对应的回答结构。管理台原样展示 schema 与可修改目标，按用户输入发送 JSON；具体 schema 和目标的业务合法性由 Luma 校验。本地不加载远程 schema、不执行 schema 内容或任意 client tool。

- 新增 `respond` 动作，沿用管理鉴权、secret grant、当前 revision、原 provider／credential 身份连续性和 Local 限制。请求必须绑定已存任务、原问题 invocation ID 及完整 pending 内容的指纹。
- review 最多 24 KiB、50 项展示素材；每次按 4 项调用原工作区的 view asset，核对返回 ID、媒体类型和 HTTPS 公共地址。样本与最终生成资产分开存放，不能因为展示了样本就把任务判为成功。所有要求展示的素材均就绪且有链接后才开放答复。
- 提交前使用原 SQLite CAS lease 保存意图、独立幂等键和答复摘要，不保存答复正文。每个 invocation 最多一次提交，每任务最多 32 条回执；没有背景重试。HTTP waiter 断开不丢弃已受理操作。
- 送达回执必须匹配原 session／run，先保存 delivered 与游标，再读取后续进展。seq 为空则从 0 读取；working 不推进游标；has_more 页未读完时不允许再次答复。每次显式查询读取一页，不创建无限轮询。
- 提交失败／超时／错身份回执保留 unconfirmed。即使重启后仍看到原问题，也不重发已消费 invocation。后续查询失败不能撤销已保存的 delivered。过期 responding lease 转为未确认；此测试是持久状态故障注入，不等同操作系统强杀验收。
- 前端纯文本展示供应商 JSON，不执行 HTML；问题切换清空答复与确认，账号／revision／grant 变化沿用已有隔离，迟到响应不能回填另一账号。图片使用 no-referrer，视频不自动播放。素材链接不携带管理或 MCP 凭据。
- 新字段均默认兼容旧任务，没有数据库迁移、存储版本变更或新的持久任务系统。持续脚本、夹具、测试均在仓库，清理 `linshi` 不影响开发入口。

## 验证

前端 Review／Panel／Recovery／Reuse 四个相关文件首轮 **24 passed**；补充会话结束分支后只复跑 Review，**11 passed**，合计 25 个不同用例有通过证据。最终 TypeScript 与正常 Web 入口构建通过。独立 Edge 合成组件浏览器检查通过，覆盖内容／图片展示、无效 JSON、逐次确认、一次答复、后续查询以及 1440px／390px 布局；后续会话结束分支用组件测试验证，未重复无关浏览器流程。它与管理 HTTP 合成验证分开，不冒充正常网页到真实供应商的联合验收。

Rust 首轮创作过滤器 **33 passed / 1 failed**；修正后仅复跑 review 模块，最终 **9 passed / 0 failed / 6454 filtered**。加上首轮未受修正影响的 26 项创作用例，共 35 个不同 Rust 用例有通过证据，不声称最后一轮跑了 35 项。首轮编译 15m32s、测试 26.52s；修正编译 17m36s、测试 6.44s。最终结果与源码 hash 见[本批验证记录](../evidence/luma/gateway-luma-mcp-review-20261120/verification.json)。没有运行全仓库回归、正式发布审计、真实账号或收费验证。

首轮 Rust **33 passed / 1 failed**。失败夹具在 shutdown 已阻止 follow 时仍预留对应响应，后续 read 消费错位响应，最终尝试重复答复时触发 JSON 解析断言；使用首轮测试二进制只复现该失败并保留 backtrace，确认调用位置。修正响应队列后只复跑 review 模块。同时修正实际边界：会话报告 ended 不能阻止读取尚余事件页和原 run 的最终资产，新回答则仍被禁止。

有效行 checker **31 passed**、最终 ratchet、Rust formatter、UTF-8 无 BOM 和 `git diff --check` 通过。新增协议模块 179 行、管理交互模块 122 行；新增管理链测试文件 276 行，职责为审阅管理路由和持久回执验证，未修改规模 baseline。合成 Edge 已关闭，预览服务正常退出且 4451 无监听。

## 保留边界与停止点

本批是有 `answer_format` 的 review 管理入口，不是所有 run 交互完成。`approve/confirm/text_input/select` 的精确回答形状、consent／connector action、任意 client tool 及只存在于 session waiting 的其他 run 问题仍未接入；不自动代答或代执行。未知题型只能显示可识别问题并查询／取消。session 事件目前以原始 JSON 展示，未实现所有事件的媒体解析。

供应商拒绝答复时仍保留已提交／未确认状态，尚不提供可证明未消费后的改答入口；避免把所有失败都当安全重发。预算沿用既有会话上限，不增加费用授权或承诺零费用。真实 review、视频播放和远端处理仍未验。

L1 仍有分层布局、其他交互类型、Server 持久身份和真实业务边界；L2 Web、L4 完整 Server／Redis／强杀、L6 真实业务保持独立未完。按工作包仍为 4 项未完全关闭；没有用本子项冒充整包关闭。L5 旧 devtest 包不含本批和素材复用／全部模型／配置恢复的后续增量。本次不重新出包。

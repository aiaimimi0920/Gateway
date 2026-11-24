# Luma MCP 显式图片与视频创作

后续状态：[参数与编辑子批](2026-11-20-luma-mcp-options.md)已扩展三个模型的参数与编辑操作，并交付 `gateway-devtest-20261120-luma-mcp-options`。下文“未打包”保留为本基础子批当时的事实，不代表最新交付。

本批把已归档的当前 MCP 合同接入正常账号管理入口。支持 Local 模式下 UNI-1 图片和 Ray 3.2 视频的文字生成、同 run 查询、资产链接、会话费用、取消 run 与结束 session。真实收费调用仍暂停，当前运行中的 4200 实例未更新。

## 实现

- 在已保存 MCP 账号内填写提示词和会话预算，先准备任务，再单独确认使用 credits 提交。预算遵循供应商会话上限语义，包括规划；不承诺单次精确报价。
- 准备操作只写本地审计行，不调用供应商，不持久化提示词。任务 ID 在付费提交前已交给界面；输入摘要绑定原请求，每个任务只允许一次生成提交。请求响应丢失后查询原任务，不重放生成。
- 每个任务使用独立 workspace key、user_id 和两个独立幂等键。保存原 provider、credential、revision、session、workspace、run；读取和取消不能指向其他账号或任意上游任务。
- 所有 MCP 调用使用固定工具、固定生产 endpoint、有界响应、不跟随重定向、不重试。业务 session 与每次独立 HTTP MCP session 分开；DELETE 仅关闭传输，不能充当业务取消。
- SQLite 原审计行承载任务，不新增数据库版本或平行账本。CAS 领取防止同任务并发操作；一次操作最长 80 秒，四个并发槽，已受理操作持有本地 drain guard。HTTP 窗口关闭不会丢掉已经受理的生成回执。过期领取只允许查原任务，未确认提交不能重提。
- 等待只调用原 run，timeout／needs_input／settled_with 均不当成功。接受资产前核对 session/run 和 asset 集合，只有 ready 资产暴露通过 HTTPS URL 边界检查的链接；不携带 MCP 凭据访问资产。此路径交付链接，不声称已下载、解码或播放二进制媒体。
- 费用来自该 session 的 rollup，未拿团队余额或各资产重复计价代替。未取得费用时为未知；未决事件、估算 job 或仍在运行的任务不能标为已结算。结束会话后仍可再次读取晚到费用。
- 运维请求详情展示 MCP 任务 ID、复制入口、费用和资产链接。原只读 profile 的名称和禁止通用推理／后台探测约定保留；创作只经新增显式授权接口。

入口：`POST /v1/internal/gateway/console/luma-mcp-creation`，动作 `prepare/start/read/cancel/end`。后端沿用管理鉴权、敏感信息 grant 和配置 revision 检查；生成前再次检查 grant、revision 与 drain。所有响应 no-store，错误不带供应商正文或凭据。

## 验证

最终 MCP 定向回归 **36 passed / 0 failed / 1 ignored**；忽略项是需要显式启动的旧只读浏览器宿主。共享资产校验相邻回归另 **1 passed**。新用例覆盖正常管理 API → SQLite → 合成 MCP 的图片流程、费用确认、重复提交拒绝、timed_out、错账号／错 session、原 run 取消、未知／估算费用、URL 边界、提交响应丢失、HTTP waiter 断开后的回执保存、同目录重启查询视频任务，以及 CAS 陈旧写入和缺少可用资产时不能显示成功。

前端五个文件 **34 passed**，覆盖逐次费用确认、提交不明后的原任务查询、取消／结束分离、账号切换后的迟到结果隔离和正常运维详情恢复任务 ID。TypeScript、正常 Web 构建、Rust formatter、`cargo check --lib --no-default-features`、UTF-8 无 BOM 检查通过；有效行检查器 **31 passed**、ratchet 通过。没有扩大到全仓库回归或真实上游。

首轮 Rust 编译因新测试误用未安装的 `tempfile` 失败，已改为现有临时目录与清理机制，没有引入依赖。第二轮 **28 passed / 4 failed / 1 ignored**：四项业务断言通过后，清理器拒绝了不带 `gateway-local-` 前缀的新夹具目录；修正目录名后完成上述成功回归。前端第一次 runner 使用 Windows 不可执行的 `npm` 名称返回 127，改用 `npm.cmd` 和新回执目录执行。原始失败保留为失败。

源码及持续回归均在仓库，临时运行数据位于 `linshi/gateway-luma-mcp-creation`；关键回执与源码 hash 见[长期验证记录](../evidence/luma/gateway-luma-mcp-creation-20261120/verification.json)。清空临时目录不影响这些测试和结论的追溯。

## 剩余边界

这是一条可验证的窄创作流程，L1 尚不能整体关闭：第三方模型参数、图片／视频编辑与分层、需要用户回答的 run 交互未接入；Server 模式缺少适用的管理任务持久身份，本接口明确拒绝该模式。账号或配置 revision 改变后旧任务拒绝跨版本查询，不能声称已实现重新绑定／自动刷新恢复。

供应商受理但尚未返回可验证 session/run 的窗口保持未确认，不自动重发；进程强杀与真实远端取消仍未实测。结束 session 不删除 workspace/资产。任务沿现有审计保留策略保存，不保证超过该保留期的恢复。原生新包、正常浏览器全链、真实生成与账单未在本批验证。

L2 Web 当前登录协议、L4 完整 Server／Redis 与强杀、L6 真实收费验收仍独立未完。之前的 devtest 包保留，不包含本批新增产品代码；不以旧包替代新源码交付。

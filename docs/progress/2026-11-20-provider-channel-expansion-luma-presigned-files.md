# Luma Files 签名上传、TTL 与用户标记

日期：2026-11-20（Asia/Shanghai）。范围：R14 Agents Files 管理子链，不是 Luma、R14 或渠道扩展计划整体完成。

## 实现

正常 Agents 账号的 Files 界面支持最大 **64 MiB** 文件。超过 8 MiB，或设置了过期时间／用户标记时，使用新签名上传链；其余小文件沿用原 multipart 上游入口。64 MiB 是 Gateway 的本地内存边界，不是官方文档所述 5 GiB 存储上限；本批不开放 64 MiB 以上上传。

前端把元数据和二进制放入同一个 multipart 管理请求。API client 保留原管理 header、同源校验、禁止重定向和 no-store，并让浏览器生成 multipart boundary。上传不经过 base64 展开或临时磁盘。管理端先授权并取得原有两个账号操作槽，再读取有界正文：metadata ≤4 KiB、文件大小与声明严格一致、仅接受一个文件；文件总上限 64 MiB，接收限 60 秒。JSON 小文件入口仍限 12 MiB 请求正文。

同一个 future 顺序执行 `POST /v1/files` → storage `PUT` → `POST /v1/files/{id}/complete`，总上游预算 180 秒。创建与完成响应必须匹配原文件 ID、名称、MIME、大小、用途及可选 metadata。PUT 失败不调用 complete，各写步骤均不自动重试。超时、取消或配置变化可能留下远端已接受的文件；用户先刷新文件列表核对，不把断开连接称为远端撤销。

签名 URL 只由固定 Luma API 的响应取得，不接受调用者提供的 URL。复用公开资产 URL／公网 DNS 校验与固定解析结果，独立客户端禁止 proxy、Cookie、默认凭据及跳转；仅发送允许的 Content-Type／完整性校验 headers。Authorization、Cookie、Host、copy-source 和其他未支持 header 拒绝。URL、签名、上游任意错误正文不返回 UI。尚未确认的存储 header 组合保持拒绝，不暗中省略签名要求。

`expires_at` 使用未来 RFC 3339 时间，UI 将本地输入转换为 UTC；`user_id` 是长度有界的不透明归因标记，不能代替账号内终端用户 ACL。管理操作仍绑定已保存的启用账号、secret grant 和 revision，没有开放公共终端用户 Files API。

## 官方合同

本批匿名联网读取以下三个官方页面，均 HTTP 200；原始 HTML、正文、抓取 UTC 与 SHA-256 在本批证据目录保留：

- <https://docs.agents.lumalabs.ai/guides/files/>
- <https://docs.agents.lumalabs.ai/api/resources/files/methods/create/>
- <https://docs.agents.lumalabs.ai/api/resources/files/methods/complete/>

持续抓取工具新增 `--page files-complete`，放在 `tools/luma/fetch-contracts.py`，不依赖临时目录中的脚本。

## 验证

前端五个文件共 31 项测试通过；TypeScript 与正常 main 入口构建通过。有效行工具 31 项和 ratchet 通过，本批主要实现／测试最大 225 有效行；共享管理路由为 488 行（489→488），入口路由为 60 行（57→60）。Rust 前两轮均 15 passed / 1 failed，完整管理上传返回 400。调查发现两处问题：全局正文上限无法兑现独立文件上限，已按既有 backup 路由方式独立挂载 Files；multipart 字段读完仍持有 multer 内部锁，独立复现得到 `failed to lock multipart state`，显式 drop 后转为正常下一字段，已修复 metadata／文件两处生命周期。保留共享 LAN／授权／生命周期中间件，未增大全局配置。第三轮大文件管理链通过，旧列表测试并行争抢共享两个操作槽而收到 409；不修改生产限流，按仓库命令串行复测后 **16 passed / 0 failed / 0 ignored**。生产／关闭 feature 编译通过。最终回执见 [verification.json](../evidence/luma/gateway-luma-files-extended-20261120/verification.json)，前三轮失败原日志一并保留。

没有调用真实 Luma 账号、上传供应商文件、消耗生成积分、修改现有运行实例／数据库或生成原生包。正常网页→真实管理 TCP 的联合、供应商摄取／复用／删除和真实 TTL 行为尚未验收。

## MCP 接续调查

本批先检查 MCP 单任务创作。2026-11-20 联网核对 [官方 Connector Guide](https://lumalabs.ai/learning-center/articles/luma-claude-connector-guide) 和 [帮助中心 MCP](https://luma-ai.support.site/article/mcp)，能确认固定远程地址、工作区、模型权限及 App credits 关系；搜索 `luma_generate_image`、`luma_run_await` 未取得可实现的完整结果 schema。匿名向固定 MCP 地址请求 tools/list 返回 HTTP 401，未取得目录。两页公开原文及匿名请求状态已归档。旧 11 月 13 日目录原始证据已不存在，现存摘要不能代替完整合同。本批没有读取真实凭据或假造生成／资产结构。

因此先完成可由公开合同实现的 Files 链。MCP 单任务仍需恢复当前工具输入输出 schema（session/workspace/run/asset/usage）后继续；现有只读工具目录及模型入口保持原范围。MCP 业务取消不能用传输 session 的 DELETE 代替。

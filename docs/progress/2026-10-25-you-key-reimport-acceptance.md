# You 标准模板：真实密钥重新导入验收

日期：2026-10-25（Asia/Shanghai）。**You 官方搜索 API 的新模板创建、账号导入、
显式搜索测试与普通 Gateway 搜索均已得到真实成功证据，可以登记蓝色完成标记。**
这不表示旧凭据更换密钥后的健康恢复问题已解决，也不表示 You 网页聊天已接入。

## 首轮认证材料更正

[首轮记录](2026-10-25-you-template-acceptance.md)中的备份和导入值实际均为
`${YOU_API_KEY}`，不是可用的真实 API Key。导入前后相等只证明内容未损坏，
不能证明材料有效。独立官方诊断把占位符原样发送，得到通用 `ForbiddenException`；
不能将该结果解释为真实密钥被撤销、余额不足或缺少 scope。当前检查进程以及 Windows
用户级、系统级均未设置 `YOU_API_KEY` / `YDC_API_KEY`。

用户本轮提供真实密钥，仅经本机 UI 的 API Key 控件保存到受限私有数据目录。
不记录密钥正文、前后缀、摘要，不写入源码、公开证据、截图或构建产物。
官方认证仍为 `X-API-Key`，搜索端点为 `POST https://ydc-index.io/v1/search`。

## 实际 UI 操作及旧健康状态边界

- 复用首轮已通过 UI 创建的 `you` 空池及隔离数据目录
  `C:/Users/vmjcv/.ng-devtest-4300-you-20261025`，未重新直接写入路由或数据库。
- 在 `127.0.0.1:1435/ui/` 编辑 `you-cred-0`，保存用户密钥，得到
  `r3-418cad054633`；内存比对确认与用户提供内容逐字一致。
- 点击旧账号的“运行测试”：200，3 个有效网页结果，1877 ms。但返回
  `healthRecovery: protected`；原 403 的 blocked 健康记录受到保护，不自动解除。
- 普通 Gateway 搜索随后返回本地 503，125 ms；审计为 0 次上游尝试，
  `upstream_credential_unavailable`。这是尚未解决的“原凭据换 key 后继承旧禁用”边界，
  不是新 key 被 You 拒绝。未通过数据库清理、伪造健康成功或放宽 CAS 绕过。
- 使用“手动录入账号”重新导入独立条目 `you-cred-1`，名称 `You API 20261025`，
  支持模型 `you-search`。保留旧条目及全部审计，在 UI 中停用 `you-cred-0`。
  最终配置 revision 为 `r5-2f4a4037ebfb`；新账号 enabled=true，旧账号 enabled=false。
- 点击新账号“测试账号 → 运行测试”，再验证普通 Gateway 的 `query` 与 `q` 两种输入。
  旧账号的永久健康记录仍保留，重新导入是本轮可用路径，不冒充恢复逻辑修复。

## 真实成功证据

| 检查 | 结果 |
| --- | --- |
| 新账号 UI 搜索测试 | passed；200；3 个有效网页结果；1525 ms；healthRecovery=applied |
| Gateway `POST /v1/search`，query | 200；1 个有效网页结果；1125 ms |
| Gateway `POST /v1/search`，q 别名 | 200；1 个有效网页结果；438 ms |

两次普通搜索均返回 `Rust Documentation` / `https://doc.rust-lang.org/stable/`。
审计均精确选中 `you-cred-1`：`search/search`、`you_search`、
`search_api_compatible`、`direct_http`，各只有一次上游尝试，没有调用其他服务商。
搜索测试保持 `score=null`、`correct=null`、`capabilityLevel=unrated`，不制造聊天评分。

- 新账号 UI 审计：`probe-cc0cf9d7-c855-4f80-a16d-3c07ff4a2409`。
- query 审计：`gw-audit-3839a546-d8e2-4ecd-acba-f33bb22d043d`。
- q 审计：`gw-audit-39d8396a-4244-43d6-a667-234af2379ee0`。
- 旧账号换 key 后本地拒绝：`gw-audit-5ec76eaf-6130-46e4-aec9-cbdc82ffca00`。

调用适配器、标准 API 指纹层和普通 HTTP 执行复用首轮已验证的三适配器链。
不新增重试、提取、抓取或其他收费参数。本轮只进行两次 UI 搜索和两次成功普通搜索；
额外一次普通请求在本地被拒绝，未出站。未再次执行独立官方直连诊断。

## 隔离与验证范围

原 4200 Gateway 未操作、停止或重启，原 You provider 与首轮备份逐字段一致，
原 routes SHA256 未改变。新密钥只在独立 4300 数据中，不表示已部署到用户正在使用的实例。
复用首轮哈希已核验的 API devtest 验证程序，不生成桌面分发包、不冒充正式 Release。

代码仅新增 You 的真实验收 receipt，并更新对应完成标记测试；不修改 Rust 运行逻辑。
完成标记测试先因缺少 You receipt 实际失败，登记后与 You/Nvidia/OpenRouter 模板测试
共 4 文件、14 项通过。TypeScript typecheck、有效行检查器 31 项、ratchet 和
`git diff --check` 通过；6 个本轮源码/文档文件均为 UTF-8 无 BOM，非私有文件密钥扫描通过。
完成标记模块有效行 21→26，关联测试 91→91；无新超限、例外或 baseline 修改。
不重做付费上游全量测试，不验收站点健康轮询、其他 You 产品或聊天/流式行为。

本轮证据：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-you-reimport-20261025/`。
首轮失败证据保留在原目录。运行结束仅停止自建 helper，保留隔离数据和受限备份。
未提交、未推送。

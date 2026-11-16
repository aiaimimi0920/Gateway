# Suno MCP 独立客户端登记与账号资格阻碍

## 本轮推进与授权范围

用户明确同意为 Gateway 注册独立 Suno MCP 客户端，并自行在官方页面登录授权；范围仅获取接入所需工具目录，不生成音乐、不下载、不消费积分，不读取既有凭据。

先只读核实剩余项：FreeBuff 三协议公共实时 SSE 已有生产入口和正常网页联合合同，不重复实现；Suno 基本 Web 音乐/probe/MP3 路径已接。Udio 当前产品合同禁止据旧音频地址新增下载，未采纳音频读取建议。

## 实际联网结果

- 官方自定义连接说明：`https://help.suno.com/en/articles/14551425`，支持将 `https://api.suno.com/mcp` 添加到支持自定义 MCP 的客户端。
- 重新匿名读取资源 metadata 和授权服务器 metadata，均成功；端点、scopes、S256 与 public client 信息已保存，不把 discovery 当作账号可用证明。
- 向 metadata 声明的 `/register/` 登记独立客户端成功，名称 `Neuro Gateway Protocol Validation`。只请求 `authorization_code`、`response_type=code`、`token_endpoint_auth_method=none` 与本机 callback，不借用官方 CLI 或其他产品身份。
- 授权请求使用随机 state、PKCE S256、`resource=https://api.suno.com/mcp` 和 `scope=api:v0`，没有请求 offline_access、credits:spend 或 download。scope 名不被视为实际工具或收费授权。
- 官方授权入口转到 `suno.com/link-account`。用户自行登录后，页面明确显示：`Connecting AI apps to Suno isn't available for your account yet.`；主线程通过页面 DOM 独立核实。

**结果：客户端注册成功，但本次账号不允许连接 AI 应用。未进入授权码交换，未获得 access token，未读取 tools/list，更未调用任何生成/下载工具。** 不能由此推断所有账号不可用，也不能猜测升级套餐即可解决。

## 证据与收尾

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-suno-mcp-20261112`。

- `resource.json`、`authorization-server.json`、`discovery-receipt.json`：本次公开 metadata 与 HTTP/哈希回执。
- `registration.json`：非秘密客户端 ID、名称、redirect 与登记时间。
- `inspection-result.json`：账号阻碍、零生成/下载及监听关闭记录。
- `inspect_mcp.py`：一次性受限调查程序，仅允许 initialize/initialized/tools/list，无 tools/call；未作为生产 OAuth/MCP 实现交付。

用户报告失败后未重试、未增加权限、未换身份或尝试绕过。已停止本次唯一调查进程并核实本机 callback 端口 63783 不再监听。没有账号授权可撤销；独立客户端登记仍存在，未声称已注销客户端。临时授权链接已失效，不应复用。

本轮没有改动生产源码、账号池、持久数据库或运行实例，也未构建、出包、提交或推送。文档增量 whitespace 检查通过；没有为纯文档回执重复执行 Rust 或发布检查。

## 下一步准入

本账号的 Suno MCP 调查暂停在资格边界。只有账号获官方开放，或用户明确选择另一个有资格的账号后，才继续官方授权和 tools/list；不重复注册客户端来碰运气，不要求购买套餐。已登记客户端可在明确恢复时优先复用，需重新建立本机监听和新的 state/PKCE，不能复用旧授权链接。

工具 schema、任务/资产协议、正常模板、授权保存与刷新 owner 尚未冻结或实现；本次登记成功不改变 `suno-oauth` discovery/接入调查状态，不增加模板或整卡 offline_ready。其他渠道与总计划不因这一账号受限而被标为完成。

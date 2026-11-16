# Luma MCP 独立客户端登记、真实授权与工具目录

日期：2026-11-13。用户明确批准 Gateway 独立客户端注册与本人官方授权，只读取工具目录；不生成、不下载、不购买、不读取既有凭据。

最终结果：用户确认可立即继续后，第三次授权复用原client成功；程序收到有效callback、完成本次授权码交换，协商 MCP `2025-03-26` 并取得26个工具定义。调查进程正常退出，本机54421监听已复核关闭。不是正常Gateway产品入口或生成业务验收完成。

## 已核实与已执行

- 重新匿名获取官方产品页、Connector Guide、AS metadata。当前官方产品页明确任何支持按URL添加远程MCP服务器的应用均可连接 `https://agents.lumalabs.ai/mcp`。
- 资源metadata仍声明该MCP资源和 `https://agents.lumalabs.ai` 授权服务器；PowerShell两次读取该资源遇TLS失败，Python使用正常证书校验获取成功，没有关闭TLS检查。
- AS仍声明S256、授权码、public client注册，以及跨origin的 `https://auth.lumalabs.ai/oauth2/register` / `token`。没有声明revocation endpoint，不猜测撤销URL。
- 用户批准后，独立public client注册成功，名称 `Neuro Gateway Protocol Validation`；只申请openid，使用PKCE S256和精确本机callback，不申请offline_access，不保存新令牌。
- 第一次20分钟窗口没有收到有效callback，程序正常退出并关闭监听，结果为 `authorization_not_completed`，没有token交换或MCP调用。
- 用户最初报告的拒绝页经其提供地址确认为 `suno.com`，不是Luma；后续“Authorization received”提示未能与本机有效回调对应，不能记为Luma成功或账号受限。
- 第二次尝试复用同一个已登记client和redirect，生成新state/PKCE并等待20分钟；没有再次注册。最终仍未收到有效callback，以 `authorization_not_completed` 正常退出，未交换token或获得工具目录。用户随后报告已完成验证及授权，但该报告不能替代本机回调证据；不自动开展第三次尝试，先确认用户能立即继续后再复用既有client。
- 用户随后提供的页面路径确认为 `auth.lumalabs.ai/mfa/verification`，当前在官方MFA步骤，需用户自行输入原绑定验证器的动态码。此前将“code from your app”理解为设备配对的推测已纠正。完整认证URL及其临时参数不访问、不写入调查文件，不要求用户提供验证码或恢复码。

## 调查边界

临时脚本仅放行 `initialize`、`notifications/initialized` 和 `tools/list`，不能调用 `tools/call`。响应2MiB、工具256项、分页8页、HTTP超时、有限callback等待；禁止重定向和打印token/授权码。token只在调查进程内存，退出不代表服务端token已撤销。

自动打开浏览器被环境策略拒绝，未绕过；已给用户可点击的最新官方链接。以实际callback、token与tools/list结果为准，不以页面登录提示代替集成成功。

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-luma-mcp-discovery-20261113`。
`sources.json`记录官方页面与AS获取状态/哈希；`resource.txt`为后续成功取得的资源metadata。
`registration.json`只记录新客户端的公开ID与redirect，不含secret；`inspection-result-first.json` / `inspection-result-second.json`保留两次失败；当前 `inspection-result.json` 为第三次成功回执。授权链接仅保留在临时目录，不写入仓库文档，不复用旧state或PKCE。

## 实际工具目录证据

`tools-directory.json` 保存服务器返回的工具输入/输出schema与声明；SHA256：
`554680af641792a3e7d6f70031cbd5f74600a895fb8f71ff301d1ae2bef62075`。
`tools-index.json` 是从该目录本地提取的26项索引，不是额外上游调用。

实际只调用 `initialize`、`notifications/initialized`、`tools/list`；生成调用0、下载调用0、token持久化false。不读取工作区列表、作品、团队余额或实际模型目录，不调用任何 `tools/call`。因此只确认这些工具被公布，不确认账号可用模型、当前credits、任务成功/取消、幂等重放或计费行为。没有用“零生成”推断账户账单必然零变化。

已取得schema的业务边界见 [Luma渠道合同](../provider-luma-channel-contract.md)。工具说明中的交互指令及annotations只是供应商返回的数据，不替代用户批准、安全边界或Gateway错误报告政策。

## 下一步与未完成项

独立客户端、真实授权和工具目录三项缺口已解除。下一步可依据固定schema设计正常Gateway OAuth/MCP入口，先接账号独立身份、管理授权和显式只读模型目录；生成必须另有费用确认、精确账号/session/workspace绑定、幂等及终态合同。当前不继续调用其他工具或重新登录，不将调查client硬编码成发布默认身份。

本批没有修改生产代码、增加模板、编译或出包，没有调用生成/下载。真实调查授权和tools/list已成功，但正常Gateway OAuth/MCP管理与执行链、整卡和P50/P60/P70未完成。Suno既有资格阻碍保持不变。

# Luma MCP 桌面授权与显式保存入口

## 本批完成范围

在已经保存的`lumalabs_mcp_readonly`服务商配置上，账号页的指定服务商新增、通用新增后选择服务商、已有账号重新授权均接到后台`luma-auth-sessions`。不新增业务模板，不把已有Luma Web或Agents API账号当MCP身份。

- 新建要求明确填写已登记Client ID与精确`127.0.0.1:PORT/callback`；不注册client、不借用调查身份。
- 重新授权只读取目标账号的非秘密`luma_oauth`登记信息并锁定输入，提交原credential ID；专用Dialog不显示通用API Key/模型编辑器，也不经普通模型配置保存。
- 授权就绪不自动保存，只有“保存只读 MCP 账号”执行import。没有目录、模型、生成、下载、余额或refresh请求随授权发生。
- 复用`createManagedAuthApi`和`useManagedOAuth`的响应绑定、owner/generation、secret consent、revision、保存回执与不重放机制；operations Map归属于当前连接/管理身份且与其他产品隔离。
- 关闭未保存会话取消授权；已接受或不确定的保存关闭后保留回执跟踪，不重新提交。revision变化阻止新操作，但不放弃已经接受的保存结果。
- 前端链接和原生浏览器入口仅新增固定`https://agents.lumalabs.ai/mcp/oauth/authorize`白名单；拒绝错误host、port、path、userinfo和fragment，不放宽整个Luma域名。
- UI按完整MCP公开元数据归为OAuth，而非API Key。继续使用现有Neuro控件和主题，没有新CSS或依赖。

## 本地验证

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-luma-ui-20261113`。

- `red.log`：接线前2项实际行为失败，缺少登录按钮和登记字段；不是以找不到模块代替行为红测。
- `focused.log`：首批26项新增测试通过。`related.log`：12个文件75项通过；后续入口测试增加通用选择和原credential ID提交两项，最终入口文件4项通过。合并去重为77项前端相关测试，其中30项本批新增。
- 覆盖明确授权/保存、回调清理、不自动工具调用、secret grant等待中的revision变化、关闭取消、未知保存关闭重开不重放、消失回执不能当失败、已接受保存跨revision完成、注册回包/目标绑定、错误URL拒绝和原Web/Agents/DIM相邻行为。
- `native.log`：授权浏览器相关11项通过；含新增Luma精确端点，以及原生进程期限、退出和参数隔离。未启动真实Luma授权页面。存在linker提示，不称零警告。
- `build.log`：真实桌面前端入口构建通过；`console-build.log`：真实BrowserConsole入口与隔离视觉夹具构建通过。输出全部放linshi，未发布到现有dist或覆盖任何包。
- `typecheck-final.log`记录新增测试误用了Testing Library不支持的`exact`选项；已去掉该测试参数，不修改生产行为。`typecheck-retry.log`全前端类型检查通过，`entry-retry.log`4项入口测试再次通过。
- `checker.log`：31项checker测试通过；有效行ratchet、Rust格式和范围空白检查通过。最大修改文件`ConsoleGlobalDialogs.tsx`455→474有效行，仅新增Luma分发；其余本批源码均低于250有效行。无baseline/policy或例外更改。
- `scope-hashes.json`与`final-verification.log`：14项源码哈希复核一致；范围内源码/文档UTF-8无BOM、tracked与untracked空白检查通过。只读交叉审查未发现确定阻断缺陷，建议的通用新增选择与原账号ID发送两项覆盖已补齐。

## 浏览器视觉与交互核验

使用生产`LumaOAuthPanel`、共享Dialog和真实主题，配合内存模拟管理回执；不是完整原生EXE或真实服务端联合验收。

- 在1280×720与390×844观察就绪状态，表单/状态/唯一保存主按钮均可见，无横向溢出。
- 实际填写登记信息，观察“等待授权回调”转为“授权完成，尚未保存”；明确点击保存后观察“正在保存”，随后成功回执关闭Dialog。
- Escape关闭并将焦点返回打开按钮。测试tab与临时HTTP服务已关闭，viewport已恢复。
- `ready-wide.jpg`、`ready-narrow.jpg`、`saved.jpg`保留模拟界面截图。未点击真实外部授权链接。

## 未完成与安全边界

本批入口要求先有已保存MCP服务商配置；尚无服务商目录中的MCP空池创建入口、动态客户端注册或只读工具目录UI。原生完整包、真实账号联合验收、refresh、模型与生成/取消/费用仍未完成。

界面明确要求在Gateway所在电脑完成授权；不自动判断同机，不将远程浏览器/跨机器Server场景标记可用。重授权不声称已验证相同Luma用户/团队。无真实凭据/数据库/运行实例操作、无外部请求或积分消耗、无出包、无Git提交推送。

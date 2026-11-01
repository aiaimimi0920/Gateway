# Antigravity 公共客户端参数恢复内置

操作者在明确 Google 桌面应用的 public-client 参数与个人账号 token 的区别后，
要求恢复内置版本，并在 GitHub 对相关公开参数处理误报。

## 默认行为

- 从本地保留的原实现恢复同一套公开 installed-client ID / 配套参数，不修改账号 token 或 Key。
- 默认无需环境变量、JSON 或既有数据目录文件即可开始 OAuth 授权和 token 刷新。
- 保留完整环境变量对、显式绝对 JSON 路径的可选覆盖；不再自动读取此前准备的本地客户端文件。
- 显式覆盖错误仍明确失败，不静默混用不同 client；会话快照、PKCE、state、固定 Google
  endpoint、禁止重定向及只在相同 client ID 下添加客户端参数的安全边界不变。
- 用户 access/refresh token 继续由既有凭据 owner 管理，不内置、不新增导出或浏览器依赖。

## 误报处理范围

Google 官方将桌面 installed applications 视为 public clients；该场景下客户端参数不被当作
可保密的用户凭据。参考 [Google OAuth 文档](https://developers.google.com/identity/protocols/oauth2#installed)。

Gitleaks 仅在两个已知公开常量上使用行级例外；不添加整文件、整类 Google 凭据或个人 Key 的排除。
GitHub 只对这两个具体检测处理误报，不全局关闭用户级 push protection，不扩大仓库访问权限。
本轮只读预检发现仓库级 secret scanning / push protection 原本已为 disabled；没有由本轮关闭。
GitHub 官方说明：公开仓库即使未启用仓库级 scanning，也可能受默认用户级 push protection 保护。
实际放行结果、最终远端 SHA 和包验收以工作区外本轮回执为准，不以源码说明代替执行成功。

## 验证与交付

默认内置静态回归先红后绿；配置优先级、无文件默认、显式覆盖失败、刷新作用域和既有
账号持久化走定向测试。重新执行 formatter、checker 自测、ratchet 和 diff check。
使用官方 desktop devtest、锁定依赖、Jobs=2；新建不可变版本目录，不覆盖旧包或重启在线实例。
包内真实 EXE 验证无客户端文件即可创建授权会话，并检查公共 API 的 pricing 错误转路由。
不将 Google 真实重新授权或付费生成当作本轮默认测试。

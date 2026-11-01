# Antigravity OAuth 客户端配置外置

> 后续决策：操作者明确要求恢复内置公共客户端参数。本文件保留外置方案的历史边界，
> 不再代表当前默认行为；当前为内置默认、仅显式外部覆盖，见
> [内置恢复记录](2026-11-02-antigravity-oauth-builtin-default.md)。

## 原因与范围

GitHub Push Protection 拒绝了尚未发布提交中的 Google OAuth Client ID / Client Secret。
按操作者选择改为运行时私有配置，不绕过保护，不编码隐藏常量，也不改变已发布远端历史。
原被拒绝提交只保留本地回退引用，不随 `main` 推送。账号 access/refresh token、API Key、
路由、数据库版本及已有凭据格式不因客户端配置外置而迁移或重写。

## 实现

- `protocol::antigravity::OAuthClient`：完整环境变量对、显式绝对路径文件、共享数据 owner
  默认文件三层来源；半配置拒绝回退，无编译时注入、默认凭据或永久失败缓存。
- JSON 严格字段验证，字符串非空且有界，文件上限 8 KiB、读取 deadline 三秒；错误只含
  固定安全 code，不返回配置内容、路径或 secret。对象不派生 Debug / Serialize。
- 登录绑定回调端口之前加载并固定配置快照；授权 URL、code exchange、持久化的
  `refresh_client_id` 来自同一份快照。状态视图不包含 Client Secret。
- 仅专用 Antigravity adapter 的 token 刷新加载配置。Client Secret 只发往精确的
  `https://oauth2.googleapis.com/token`，且请求 client ID 必须与配置一致。
  缺失、无效、不匹配均在网络发送之前以非 terminal 错误停止，允许后续恢复。
- 普通 OAuth 刷新不读取或注入上述客户端配置。模型生成协议、指纹和 HTTP-only 边界不变。
- 私有配置文件加入 Git ignore，真实值不进入源码或手测载荷；示例环境文件仅列空变量。

## 验证与交付证据

安全回归测试先对旧实现红测，再对外置实现转绿。覆盖运行时来源、半配置拒绝、路径约束、
严格 JSON、超限、修正后无需重启、会话快照、精确刷新作用域、错误脱敏与管理接口边界。
另运行既有 Antigravity 和 token refresh 定向合同、formatter、行数 checker 自测及 ratchet。

本轮修改文件全部不超过 500 有效行。新增配置 owner 为 114 行，配置测试为 144 行；
原协议常量文件从 30 行减少至 7 行，不新增超限或基线豁免。

完整执行回执放在工作区外的
`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-antigravity-external-oauth-20261102/`。
新包按官方 desktop devtest 流程重新构建，锁定依赖、Jobs=2，保持源码指纹与双 EXE 哈希校验。
最终包完整性、包内真实 EXE 的隔离配置恢复/会话/API 验收及远端提交核对以该目录中的
最终回执为准；构建本身不等于真实 Google OAuth 或付费生成重验。

本轮不自动停止、替换或重启当前在线 Gateway，不覆盖任何旧版本目录，不运行付费上游测试。

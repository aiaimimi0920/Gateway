# ChatGPT 网页账号模型目录与手动模型

## 本次修复

- 已保存的 `chatgpt-web-reverse` 账号接入“拉取模型”及账号目录刷新。
  请求仅传 `credentialId`，复用管理鉴权、敏感权限、取消及 revision fence。
- Web 与 Codex 使用各自的固定官网目录端点。Web 按 `models[].slug` 读取请求标识，
  不把显示名称或 Codex 的 `supported_in_api` 当作网页模型权限。
- 原生网页登录取得会话后，best-effort 读取同源网页目录并回填 fetched 栏。
  可选目录读取受独立预算约束，不能把已经成功读取的账号改判为登录超时。
- 目录被拒绝、非 JSON、空目录或网络错误不清空现有模型、不丢账号材料；
  成功导入仍显示“已读取账号”，目录错误独立显示。
- 手动模型正常保存并登记精确路由，保留已有 manual、其他账号及禁用路由。
  目录结果和手动声明都不伪造 `verified_models` 或生成成功记录。
- 目录请求禁止重定向，限制响应大小、模型数量和模型 ID；不回显上游正文或凭据。
  未增加验证码求解、指纹修改或网络切换行为。

## 已通过的定向检查

- 前端 6 个相关文件：45 tests passed；TypeScript typecheck 通过。
- Node 导入、登录、页面会话和目录相关用例：27 passed。
- Rust Web 目录解析/HTTP 合同 2 passed；通用 slug 解析回归 1 passed；
  Web/Codex 管理目录权限合同 2 passed。
- Python 浏览器运行时打包合同：2 passed。
- 真实 Chrome 的本地合成会话回归：1 passed，覆盖 Cookie 持久化、读取重试、
  模型回填、Browser.close 和专用 profile 回收；所有页面请求均由 fixture 拦截。
  本轮首次执行在初始合成浏览器退出阶段报 `browser_timeout`，未进入账号读取；
  未修改生产超时，随后同一用例重跑通过。该次启动波动原因未定位。
- 有效行检查器：31 passed；ratchet 通过，governed source >700 为 0。
  本次最大受影响源码为 `CredentialDialog.tsx`，400 effective lines；新增模块均低于 500。
- Scoped rustfmt、UTF-8 无 BOM 检查及 `git diff --check` 通过。

## 真实业务验收与交付边界

不关闭、重启或替换现有 4200 Gateway，不改其配置、凭据、数据库或网络出口。
新包使用 desktop devtest 流程，由用户手动切换；构建和打包必须核对同一源码指纹。

真实调用使用新包、独立 4300 数据目录，限定 `chatgpt-web / vmjcv111`，
并在本地核对保存的账号及 token 邮箱 claim 均为目标账号。禁止切换其他账号凑成功。
手工指定模型仅是诊断声明，只有该账号返回有效回答和终止标记，且审计归因匹配，才算成功。
调用阶段禁用浏览器回退，不自动求解上游挑战。

本文件不宣称目标账号已完成真实生成。新包清单、哈希及隔离调用的脱敏结果另保存在
`linshi/gateway-chatgpt-web-call-20261028/`；其中私有隔离数据不进入包或版本管理。

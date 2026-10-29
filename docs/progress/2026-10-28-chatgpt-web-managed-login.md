# ChatGPT Web：由程序管理的网页登录入口

## 用户要求与范围

用户否决“输入调试端口、选择现有页面”作为普通账号导入入口。默认改为：
点击网页登录 → Gateway 打开独立可见浏览器 → 用户完成登录或人工验证 → 自动读取会话材料 → 用户保存账号。
这只是交互体验类似 OAuth，不调用 Codex OAuth，不将 OAuth token 当作网页 Cookie。

## 实现

- `ChatgptWebBrowserImport.tsx` 不再显示端口和页面选择；展示网页登录、等待、取消和账号结果。
- `browser-login.mjs` 复用已锁定的 playwright-core，发现本机 Chrome/Edge，创建新的独立浏览器上下文，不连接、复制或重启日常浏览器。
- 浏览器打开 ChatGPT 首页，用户自行完成登录及验证码。仅在精确 ChatGPT origin 读取会话接口与该域 Cookie，不调用模型，不自动处理挑战。
- 成功、窗口关闭、超时都关闭本次拥有的浏览器。当前上下文是临时的；不承诺下次免登录。会话材料仍由已有凭据事务保存，不在普通配置视图暴露。
- Rust 接口保留管理令牌、secret grant、真实 loopback、服务商类型和 revision 校验，返回材料前再次校验。
- 登录最多等待约 3 分钟；worker 总硬上限 220 秒，Rust 监督上限 225 秒。敏感信息授权过期时拒绝返回材料。
- 取消操作使用每次登录的随机 requestId，独立于 HTTP 断开；有界取消记录处理取消请求先到的竞态。worker supervisor 负责进程树终止和回收。
- 原 list/prepare 后端能力保留兼容，已从默认 UI 移除。会话 JSON 保留为备用方式。

## 验证边界

本轮定向结果：Node 7 项通过；前端 3 个测试文件共 24 项通过；TypeScript typecheck 通过。
最终源码的 Rust 定向测试 2 项通过，包含取消先到、取消标识隔离及登录输入不接受手动端口。
有效行检查器 31 项通过，ratchet 和 git diff --check 通过。新增登录模块 29 有效行，取消注册模块 53 行，入口组件 55 行；既有 handler 190→231 行，进程监督模块保持 422 行。
4200 原有两个监听进程 PID 82428、4280 未变；本轮没有启动 4300 实例或生成新 EXE。

Node 测试使用注入的模拟浏览器，不等于真实账号登录成功；前端组件测试不等于新 EXE 的原生 UI 验收。
本次不更换 4200 实例，不修改其账号和凭据，不自动构建或发布桌面整包。
真实浏览器启动、人工登录、采集保存及取消清理，仍需在包含新代码的隔离版本中验收。

定向入口：

```powershell
node --test scripts/tests/chatgpt-browser-login.test.mjs scripts/tests/chatgpt-browser-import.test.mjs scripts/tests/chatgpt-browser-import-wire.test.mjs
cargo test --locked --lib http::routes::internal_console::chatgpt_web -j 4
npm run typecheck --prefix apps/desktop
# 在 apps/desktop 工作目录运行：
npm exec -- vitest run src/features/console/ChatgptWebBrowserImport.test.tsx src/features/console/CredentialDialog.web-session.test.tsx src/features/console/CredentialDialog.test.tsx
```

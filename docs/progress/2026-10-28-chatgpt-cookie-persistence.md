# ChatGPT 网页登录关窗后无法读取：Cookie 持久化修复

## 原因与证据

用户在专用 Chrome 完成网页登录并正常关窗后，Gateway 未显示“已读取账号”。
运行包 `gateway-devtest-20261028-200825-session-read` 的管理请求实际返回 503；
延长读取时间与细分错误码没有修复该交接问题。

Rust `ConsoleBrowserWorkspace` 使用 `canonicalize(temp_dir())` 保留安全清理所需的
目录身份，在 Windows 产生 `\\?\C:\...`。Node 根据继承的 TEMP 创建专用 profile，
原实现直接将该 spelling 传入 Chrome 的 `--user-data-dir`。

真实 Chrome、仅本机 HTTP 与合成持久 Cookie 的四格对照：

| profile 参数 | TEMP 环境变量 | 关窗重开携带 Cookie | Cookie 数据库 |
| --- | --- | --- | --- |
| 普通路径 | 普通路径 | 是 | 存在 |
| verbatim 路径 | 普通路径 | 否 | 不存在 |
| 普通路径 | verbatim 路径 | 是 | 存在 |
| verbatim 路径 | verbatim 路径 | 否 | 不存在 |

所有进程正常退出，页面都成功访问本机 fixture。故障在 Chromium profile 参数，
不是用户未登录，也不是 TEMP 环境本身。证据位于
`linshi/gateway-chatgpt-native-package-20261028/cookie-matrix-20261028-205640/result.json`。

## 修改边界

- 仅在 Windows Chrome argv 边界，用 Node 标准 file URL 往返去掉 profile 的
  verbatim spelling；同一 profile、原始文件 I/O、父进程 canonical 身份和清理规则不变。
- 不提前开启 CDP、不注入登录页、不放宽浏览器安全参数、不读取用户日常 profile。
- 同时修复前端两个经失败测试确认的状态问题：后台刷新选项或 reveal callback
  不再清空已读取材料；修改本地账号 ID 不再卸载并取消正在进行的网页登录。
- 关闭弹窗、切换编辑目标或服务商、撤销 Secret Grant 仍清理/隔离材料。

## 聚焦验证

- 修复前：Windows profile 参数回归测试失败；前端两个材料丢失/捕获取消测试失败。
- 修复后：Node 导入/登录/页面状态 30 项通过；前端针对性测试与 TypeScript 通过。
- 新增 opt-in 真实 Chrome 合成 Cookie 测试，直接使用产品
  `createNativeBrowserSession`，继承 verbatim TEMP、中文及空格路径。
  第一阶段无 CDP、正常退出；第二阶段真实 CDP 重开，服务端确实收到原 HttpOnly Cookie，
  随后仅回收本测试拥有的进程及 profile。该测试已通过，不访问账号或付费模型。
- 后续包验证与最终检查回执保存在
  `linshi/gateway-chatgpt-native-package-20261028/cookie-fix-tests/` 和
  `linshi/gateway-chatgpt-native-package-20261028/cookie-persistence/`。

本记录不是新包真实账号人工验收。当前 4200 实例、凭据和数据库不做修改或重启。

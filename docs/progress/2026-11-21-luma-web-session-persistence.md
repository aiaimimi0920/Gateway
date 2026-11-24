# Luma Web 会话维护与轮换保存

本批补上已观察到的 `Set-Cookie: wos-session` 轮换消费路径。上批 User-Agent 贯通继续保留。Web 会话实现及本地链路验证完成；真实账号和新手测包状态见下方。

## 实现

- 生产 worker 关闭页面请求后读取最终、唯一且作用域正确的 `wos-session`。业务 402 等正常失败也返回私密回执；回执带本次调用绑定，原 task／媒体结果不携带 Cookie。
- 正常 Gateway 初始化独立 Web 会话 owner。Local 复用现有安全目录、writer lock 和原子替换；Server 用单 key Redis CAS。没有改 OAuth 材料格式、数据库版本或既有账号配置。
- 账号、原导入材料、站点、realm、UA 和认证设置组成稳定身份。新请求读取已保存的最新 Cookie。请求前持久领取原状态；并发调用、迟到回执和未知发送不能覆盖或重放。
- 写入失败时先在有界内存中保留已收到的材料，后续维护／新请求／正常关闭只补写存储。原子保存完成后重启可直接复用。若进程在收到回执之前终止，则保持未确认，要求新的浏览器会话；不猜测原请求是否执行。
- 已启用的 Web 账号每 5 分钟经原 board 和认证 signature GET 检查，不发送创作 POST。明确 401 或 Cookie 删除进入重新登录状态；不绕过验证，也不把 Cookie 的 Max-Age 当作内部 token 到期。
- 远端 executor 使用新版本路径，返回的内部回执在 caller 解析业务错误之前保存。新客户端不会回退到旧路径；新版远端保留旧路径的任务结果和 402 分类，只有通过绑定校验的版本化入口可以返回私密回执。分开更新时先更新远端 executor。

## 验证记录

初始真实 Edge 回归复现“402 后返回中没有轮换 Cookie”。修复后的 Node／真实 Edge 回环组合 **39 passed**，包含 User-Agent、单次生成、307 不重放、SSE 清理、成功检查和失败响应轮换，以及 Cookie 作用域拒绝。首次新增重复 Cookie 负对照暴露了将重复项当缺失处理的问题，已改为明确拒绝后通过。

两次 Rust 回归先发现旧路径夹具及 worker 成功信封不匹配。修复夹具并保留原有的 307／308 不重放、128 KiB 边界和完整任务校验断言；另补旧入口保持业务 402、新入口拒绝未绑定请求的回归。

- 定向 Rust：**92 passed / 0 failed / 3 ignored**。
- Rust → Node → 真实 Edge 回环：**1 passed**。认证 GET 获得轮换 Cookie；重新打开存储后继续使用新值；合成业务 402 返回的再次轮换也落盘，单次提交且不重放。
- 独立 tmpfs Redis：**1 passed**。跨 owner 验证 CAS、并发互斥、轮换后读回及迟到回执拒绝。
- 规模检查器 **31 passed**，ratchet、格式和 diff 检查通过。

见[脱敏验证回执](../evidence/luma/gateway-luma-web-session-20261121/verification.json)。

## 新桌面手测包

启动：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/release/Gateway/gateway-devtest-20261121-luma-web-session/gateway-ui.exe`。保留整个版本目录；这是 desktop devtest，不是正式 Release。

包校验 **6157 项**，构建及打包源码指纹 `269b36429b3e4a088b820b5ee36ce550a6ef87fbfffd71991add0a1134d693d6`。包内 worker、会话回执脚本及六份 SQL 与源码一致；未携带实际 `.ng` 或 `.env`。

- `gateway-ui.exe` SHA-256：`5889891e7f3cc8fa9ccb2abbaafbb0e1a07a71f48cedf4d95dac8957878716e6`
- `gateway.exe` SHA-256：`899d4cd4b5abb5bd2f4a95a50a3e55250bf647cd82dadc3338e9ae2313708630`
- 隔离启动：UI 拉起同包后端，临时端口 61402，SQLite 就绪，`/ui/` 200，结束后同包进程 0。未操作原有 Gateway 实例或真实账号。

详见[包核查](../evidence/luma/gateway-luma-web-session-20261121/package-inspection.json)与[隔离启动](../evidence/luma/gateway-luma-web-session-20261121/desktop-smoke-result.json)。

## 真实账号验收边界

本批没有执行真实付费生成。正常管理只读检查显示当前运行实例的 Web provider／credential 数均为 0。原本机 Edge DevTools 仍存在，可读文字；切换应用程序面板先返回 `element 34 has no cached bounds`，重选并激活后返回 `coordinate input geometry is unavailable`，按工具恢复规则停止操作。因此本批不宣称真实账号的自动轮换或失效恢复已通过。

MCP 分层仍按此前 5 credits 余额、官方最低 75 credits 的证据暂缓，未实现；没有重试原未确认分层调用或改写其错误原因。

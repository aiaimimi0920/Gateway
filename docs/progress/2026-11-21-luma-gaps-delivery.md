# Luma 明确缺口的新包交付与剩余对账

## 本轮结果

前批完成的 5 个功能点已经进入新的 desktop devtest 包：MCP `approve`、`confirm`、`text_input`、`select` 显式答复，以及 Web 凭据 `Keep` 的目的地／realm／UA／到期与认证上下文绑定。新增功能的源码哈希与原定向验证回执逐一核对，12 个文件全部一致。本轮没有修改产品源码，没有再次执行收费生成。

桌面入口：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/release/Gateway/gateway-devtest-20261121-luma-complete-gaps/gateway-ui.exe`。必须保留整个同目录载荷；这是日常手测包，不是正式 Release。旧包及原 Gateway UI／后端进程均保留，没有替换运行实例、凭据或数据库。

## 实际验证

| 项目 | 结果 |
| --- | --- |
| 标准构建和打包 | `package-gateway-devtest.ps1 -Edition desktop -Jobs 4` 退出 0；后端 11m16s、桌面壳 25.65s，完整命令 1194.94s |
| 来源一致性 | 构建前后、打包及交付前源码读回一致：`79cb0043e2cd4ba728e3cd47e37c73c6221a3966f84b42d291c293826387689b`；整个 dirty 工作树参与构建 |
| 包完整性 | 6154 项 checksum 记录；暂存与最终目录的 EXE 大小／SHA-256 检查通过；六份 PostgreSQL 初始化 SQL、Luma worker 与模型清单均和源码字节一致 |
| 隔离原生启动 | UI 拉起同包后端；随机端口 52919，独立 SQLite ready、`/ui/` 200；供应商调用 0，本轮进程剩余 0 |
| 功能检查复用 | 58 个 Rust 测试通过，4 ignored 不计 passed；24 个前端测试通过；此前类型、格式、31 项规模检查器及 ratchet 证据按不变源码复用 |

| 文件 | SHA-256 |
| --- | --- |
| `gateway.exe` | `72d300f799452147d49cb499686310df194abba46f784e72b1d57f6e8354a719` |
| `gateway-ui.exe` | `51de34cd6399e86c44884614f852909ea43a98ffe9bac9103b3d7ef7da6b7898` |

额外的源码读回首次单独调用内部 helper 时缺少 `Get-RelativeUnixPath`，报错后取消；补齐构建入口已有依赖后读回通过。正式构建、打包与隔离启动均一次通过，未因此重新编译。这里只对子进程清除继承的 `PSModulePath`，没有修改系统配置。

长期证据见[交付总账](../evidence/luma/gateway-luma-gaps-delivery-20261121/verification.json)、[包核查](../evidence/luma/gateway-luma-gaps-delivery-20261121/package-inspection.json)与[隔离启动](../evidence/luma/gateway-luma-gaps-delivery-20261121/desktop-smoke-result.json)。功能回执仍为[明确缺口验证](2026-11-21-luma-explicit-gaps.md)。交付后只补本文、状态页及证据索引，不改 EXE 或产品源码。

## 仍剩两项

1. **MCP 分层结果合同和实现。** 已归档的工具说明承诺每层 `index/label/bounds`，但该目录的 `RunAssetOutput` 无 `layer` 字段。本轮联网检索进一步找到官方 [Get Session Run](https://docs.agents.lumalabs.ai/api/agents/operations/get-session-run/) 与 [Start Run](https://docs.agents.lumalabs.ai/api/agents/operations/start-run/) 的 REST `RunOutput.layer` 定义；它仍不能证明 MCP `luma_run_await` 的返回结构。取得 MCP 实际结果或对应 schema 后才能完成映射；既有 Agents REST 分层不受影响。不能把 Web 的额度不足推定为 MCP 额度不足。
2. **Web 自动续期与轮换持久化。** 已从本机原有、已登录的普通 Edge 取得真实轮换证据，见下文。当前成功 probe 丢弃响应 Cookie，没有连接到正常刷新 owner；还需补原账号绑定、并发轮换和收到新材料后的持久保存／恢复，并核实失效会话的边界。现有导入、有效期和失效探测仍可使用。Cookie 的 400 天保存期不能直接当作内部 access／refresh token 生命周期，也不能把新 Cookie 出现当成过期会话恢复已通过。

这两项仍可推进，不能把它们记成已完成或额度豁免。第二项的协议调查已有实质进展，但当前只读证据尚不覆盖失效恢复与并发轮换，本轮没有接入自动写回。原生 Luma 业务点击及其他渠道此前未执行的真实验收仍分别保留，隔离启动不代替这些结果。

## 原 Edge 的会话轮换证据

只操作原 DevTools 的 Network 筛选与既有请求详情；未刷新页面、重放请求、提交生成、重登、导出 HAR 或读取完整浏览器 profile。窗口开始多次处于最小化状态；用户手动展开后继续读取。后续目标被其他应用遮挡时停止该输入，没有改动其他应用。

录制约 7.4 小时、8121 条请求。`domain:app.lumalabs.ai /refresh|session|auth|token/` 在当时 8108 条中匹配 0，不能据此断言不存在续期。转查 `Set-Cookie` 后有 28 条响应，再按 `set-cookie-name:wos-session` 匹配 24 条，包含 Board POST、健康 POST、connectors／directives GET。

选中的 `POST /api/canvas-health` 为 204。请求和响应各存在一条 `wos-session`，在内存中比较两者值不同；响应域 `app.lumalabs.ai`、路径 `/`、`HttpOnly`、`Secure`、`SameSite=Lax`、Max-Age 400 天。仅归档属性、匹配数量和比较布尔值，不保存 Cookie、真实 realm／team ID 或签名。GET 的 Cookie 值没有逐条比较，不声称其具体轮换过程已验证。

脱敏观察见[会话轮换回执](../evidence/luma/gateway-luma-gaps-delivery-20261121/web-session-observation.json)。本轮没有改变已保存材料，也未验证 Gateway 跨重启自动续期。

## 额度与使用边界

原 Web 单次生成的 `402 / Not enough credits` 继续按用户决定跳过；不计入实际 passed，也不再试生成、充值或换号。成功受理、完成事件、媒体和依赖成功受理的远端取消没有新增证据，不虚构任务或账单。

手动使用新包时，先正常退出正在使用的旧 Gateway，再启动新目录中的 `gateway-ui.exe`，避免默认 4200 端口冲突。隔离启动结束后原 UI PID 24600、后端 PID 6620 仍在运行。没有提交、推送或部署。

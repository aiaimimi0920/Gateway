# 2026-10-02：管理密钥轮换与丢失响应恢复

## 功能与责任边界

设置页原先只有外观配置，现已接入 **设置 → 管理安全**。轮换调用现有的
`POST /v1/internal/gateway/console/session/rotate`，不新增后端接口或依赖，
不修改数据库、凭证格式或 native IPC 权限。运维步骤见
[管理密钥轮换](../operations-manual.md#管理密钥轮换)。

- 新密钥须重复输入并显式确认已保存；禁止未改变的密钥、输入不匹配和重复提交。
- 写入前限制为 1–4096 个不含空白的可打印 ASCII 字符，避免提交无法再作为
  HTTP header 使用的密钥。环境变量托管时只显示部署配置提示。
- 成功后清空输入、切换当前会话并撤销已有 Secret Grant；失败提示不回显错误中
  可能包含的秘密。卸载或切服后不展示迟到的 UI 结果。

后端先原子提交新 hash 再返回响应，原前端在响应丢失时仍持有旧密钥。
`managementTokenRotation.ts` 现在负责有界恢复：轮换只写一次；网络、响应解析
或 5xx 错误后仅验证一次新密钥，最多等待 5 秒并清理取消计时器。明确 4xx
不读回、不重试；验证失败保留原始失败，不把候选密钥的 401 当作当前会话无效。

`ManagementSessionProvider` 以 host epoch 和 credential intent 检查结果归属，
退出、切服或新的凭证操作后不再写入迟到的轮换结果；已切换新密钥后，旧请求的
认证失败不能撤销新会话。API client 的候选校验选项只抑制全局失效通知，
不吞掉 401/403 请求错误，也不会把该选项发送给服务器。

## 验证结果

- 新增的响应丢失恢复回归先失败，错误为
  `expected TypeError: Failed to fetch to be undefined`，实现修复后通过。
- 最终前端聚焦 12 文件 **56/56**，包括实际 API client + MSW 的服务端已提交/
  未提交分支、失败分类、5 秒取消、退出/切服竞态以及设置页输入和只读行为。
- `npm run typecheck` 通过；最终实际产品入口的隔离 Web 生产构建通过。
  该构建沿用官方 Rsbuild 配置，输出到 linshi，不发布或清理仓库正式 Web 产物。
- checker 测试 **31/31**、effective-line ratchet 通过：2647 个文件，受治理源码
  超过 700 行为 0；14 个既有 immutable runtime assets 单列，没有修改策略或基线。
- 收尾重新校验 13 个源码 SHA-256，均与最终测试后的 `changed-files.json` 一致。
  Gateway 与 Neuro 根的 `git diff --check` 均通过；根仓库仅有既有 LF/CRLF 提示。
- 桌面前端没有配置独立 formatter 脚本或 Prettier/Biome 配置，本轮不声称通过
  一个不存在的格式化门禁；没有修改 Rust，不复跑无关的 Rust formatter/build。

### 实际浏览器闭环

Chromium 加载最终产品 Web bundle，经真实登录边界、会话 Provider 和设置页
执行合成验证。fixture 只监听 `127.0.0.1:18748`，只接受合成密钥，所有状态在
内存中；不连接真实 Gateway、账号或供应商。

服务端收到轮换后先更新内存，再故意断开响应。最终回执按本次操作的计数增量
确认 **1 次轮换、1 次新密钥校验**，当前会话已切换、输入已清空、
`localStorageHasToken=false`、`resourcesLocalOnly=true`。另验证了环境变量
托管的只读分支、中英文切换和 Unicode 输入在请求发出前被拒绝。

1280 与 390 像素宽布局人工检查；390 宽下 `documentWidth=390`，确认框与
文案保持同一行。无关 API 由 fixture 明确返回 503，断开响应也产生预期的网络
错误，因此不声称浏览器 console 零错误。

自建 Playwright session 已关闭，fixture 在所属工具 session 中用 Ctrl-C
停止，退出码 1 为该主动停止的结果；进程退出及 18748 监听关闭均已复核。
独立审查代理因服务端 503 未能执行，本轮只有主代理的本地逐文件复核，不计为
独立审查通过。

## 大小与源码证据

本轮 13 个新增/修改源码均为 UTF-8 无 BOM，全部低于 500 有效行，无大小例外。
新增恢复 owner、设置组件、样式和测试均单独承担明确职责，没有扩展 Rust/Gemini
保留工作范围。

| 文件（相对 `apps/desktop/src`） | 修改前 → 修改后 |
| --- | ---: |
| `session/ManagementSessionProvider.tsx` | 402 → 416 |
| `session/managementTokenRotation.ts` / 测试 | 新增 32 / 60 |
| `session/ManagementSessionProvider.rotation-recovery.test.tsx` | 新增 70 |
| `session/ManagementSessionProvider.rotation-http.test.tsx` | 新增 54 |
| `features/settings/ManagementSecuritySettings.tsx` / 测试 / CSS | 新增 80 / 77 / 11 |
| `features/console/ConsoleIndependentWorkspace.tsx` | 106 → 108 |
| `api/client.ts` / 测试 | 125 → 127 / 108 → 119 |
| `api/console/api.ts` / `core.ts` | 288 → 289 / 65 → 66 |

证据目录：
`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-management-rotation-20261002`。
`changed-files.json` 记录修改前后行数、BOM 与源码 SHA-256；
`source-fingerprint-verification.json` 记录收尾时逐文件匹配结果，
`browser-result.json` 保存最终计数增量，`cleanup-result.json` 保存清理结果。
布局证据包括 `management-security-wide.png`、`management-security-narrow.png`
和 `settings-readonly-en.png`；测试、typecheck 和构建结果记录于本轮工具输出。

## 交付边界

这是已实现并完成聚焦源码及隔离 Web 验证的功能，不是正式 release 验收。
没有轮换真实服务的密钥，没有构建或替换正式 EXE，没有把上一轮 OAuth 原生
验证当作本轮密钥轮换的 native 证据。真实服务、远程部署和新包原生验收仍未做。

GTK 两条安全公告的限定例外尚未获批准，正式 release 仍受
[安全门禁](../security-release-gates.md)约束；没有绕过或修改例外。没有提交、
推送或重置 Git，保留 Gateway 已有 OAuth、Linux 测试修复和缓存 dirty 内容，
Neuro 根与其他独立子仓库不在本轮修改范围。

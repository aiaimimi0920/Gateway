# Luma 正常账号显式刷新与恢复

日期：2026-11-14。此批完成正常管理入口、桌面控件、兼容客户端注册、结果接纳及原账号轮换发布的本地完整流程；已追加正常管理 API 的真实账号刷新及原生进程正常重启验收，不代表整个 Luma 创作模块完成。

## 用户流程与实现

- 新增账号可明确选择“允许刷新账号授权”，独立注册同时支持 `authorization_code`／`refresh_token` 的 public client，然后完成离线授权与显式保存。默认普通注册／授权不变；旧注册回执缺省为 code-only，不静默提升权限。两种能力分别登记，确认前不发远端请求，未知注册结果不重放。
- 已保存的 offline MCP 账号可“刷新账号授权”→“确认刷新授权”。准备操作不发送上游请求；确认后由原 Registry 持有任务，不依赖 HTTP 请求存活。重复确认复用任务和永久 grant 登记。
- 执行顺序为 reserve→单次 receive→持久 receipt→稳定解释→持久 Result→有限账号投影→事务提交→Published。保留原账号 ID、disabled 状态和其他策略，只更新 access、refresh 和 Luma context；不写通用 expiry、不启用推理或自动定时刷新。
- access 到期只影响即时使用，不影响历史 receipt／Result 的读取或恢复。expiry 锚定原发送时间；缺省 refresh 保留旧值，畸形和重复字段仍拒绝。Result 必须与已保存回执重新解释的结果完全一致，无回执和伪造 successor 均拒绝。
- “恢复保存”仅恢复已收到／已保存的证据，不再次发送刷新请求。未知 Sending 保持关闭；账号／revision 被替换后不覆盖新授权。收到响应而磁盘保存失败时在有界 session 中保留原响应，成功落盘前不按普通终态期限丢弃；容量仍限制为 16。
- 复用原 refresh transaction journal、publication locator、正常 SecretPatch 和恢复投影校验，没有新增平行持久层。修复本地恢复重复获取 writer lock 的问题；本地与 Redis 使用分别适配的持锁接口。Luma 只读账号无推理 credential cache，不执行无意义的 cache repair。

## 验证记录

- 新增正常管理入口测试先失败：原入口返回 404，未实现刷新。
- 首轮实现后：Luma 过滤回归 253 passed、2 failed、2 ignored。两项失败均为重启 publication repair 重入本地 writer lock，错误为 `console_locked`；不是忽略或放宽测试。
- 修复并补充保存失败／恢复和取消后重建测试后：**257 passed、0 failed、2 ignored**。包括真实本地文件存储、正常管理鉴权、重复确认一次发送、原账号保存、收到结果／Result 的重建恢复、过期 access 的历史恢复、提交后未 Published 的恢复、替换账号隔离、有限投影约束、磁盘保存失败后的 disk-only resume。
- 前端 Luma 相关 16 个测试文件：**156 passed**；TypeScript typecheck 通过。覆盖显式确认、丢失响应后重新打开、不重复 import、原 revision 恢复、再次显式刷新、注册能力区分和敏感确认失败。
- 复用本轮实际编译出的测试程序，公共 `token_refresh` 回归 **182 passed、4 ignored**，`console::refresh_grants` 回归 **12 passed**；过滤范围有重叠，不相加冒称独立测试数量。未再次排队编译同一测试程序。
- 有效行检查器 31 项通过，最新 ratchet 通过。无新增超过 700 有效行的源码；本批不刷新基线、不撤销其他改动。
- 证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-luma-closure-20261114`。`red.log`、`green.log` 保留实际失败；`final-luma.log`、`frontend-final.log`、`typecheck-final.log`、`ratchet-final.log` 记录修复后结果。`source-hashes.json` 记录本批源码指纹。

## 真实验收与交付边界

本批本地测试不访问官方 token endpoint。2026-11-14 已再次匿名联网核实官方 OAuth metadata 声明 refresh_token、offline_access 与 public-client none，见证据目录 `public-metadata.json`；服务端声明不代表具体账号授权已经成功。

用户已批准：隔离环境注册新的支持刷新客户端、配合官方登录、一次真实刷新及只读工具目录；保存／轮换此次新测试凭据，不读取或修改既有账号，不生成、下载、购买作品。

### 真实账号与 Web 手测包追加回执

- 包：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/release/Gateway/gateway-devtest-luma-refresh-20261114`，Web devtest，非正式 Release，无独立 desktop launcher。`gateway.exe` SHA256：`260d8ef6faa54148b1f9e586e91b1f9a2f617ccad6b717de3ae3187b24ae916a`。构建源码指纹一致、staging／最终包完整性检查通过；本节是构建后追加文档，没有因此重编译。
- 隔离 SQLite 实例经正常管理 API 初始化空 Luma provider，正常注册新 offline client。初次临时会话后续已不可用；复用同一客户端新建授权会话，没有重复注册。用户完成新链接后，实际状态为 `ready`。
- 显式保存返回 `succeeded`，revision 为 `r2-461b81e8478b`；真实 tools/list 返回 26 项。
- 仅执行一次真实刷新，返回 `succeeded`，revision 为 `r3-c3fb43e47368`；刷新后 tools/list 返回 26 项。
- 正常 drain 原进程后，使用同一个 EXE、同一隔离数据目录启动新进程（PID 74148 → 42896）；tools/list 仍返回 26 项，revision 保持 `r3-c3fb43e47368`。随后正常 drain，未修改既有实例。
- 脱敏目录回执保留于证据目录 `live/catalog-*.json`。隔离授权数据不提交、不展示；未调用创作工具、未生成或下载作品。授权网页由用户完成，管理链通过 API 验收，不冒称已做完整网页点击或窄屏验收。
- 已关闭本地正常账号注册／授权／保存／单次显式刷新／只读目录／正常原生重启的真实联合验收。强制崩溃、远程 Server／跨机器及创作业务仍不在本次通过范围。

尚未关闭：OAuth/MCP 创作业务（模型、生成、结果、取消、费用）、Web 当前协议与权益、Agents REST 扩展操作及适用的真实／远程 Server／跨机器验收。该批不减少供应商剩余总数，也不把显式刷新称为自动刷新。

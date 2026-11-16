# Luma MCP 注册回执的持久恢复

本轮补注册已接受后的同数据目录恢复；不调用真实 Luma、不创建真实客户端、不生成或下载作品。

## 改动

- 复用运行实例已有 `RouteConfigPersistence`，保留共享 read-only／degraded 状态；不另建数据库、不更改 storage version、不读取既有凭据。
- 同一 `writer.lock` 临界区完成读取、owner／provider 去重、容量检查和原子写前回执。只有成功取得新 claim 才发一次远端注册请求；未知、冲突或存储故障都不重发。
- 持久文件 `console/luma-client-registrations.json` 只含注册 id、owner 指纹绑定、provider、revision、数值 loopback redirect 与可选公开 client ID。client 为空表示结果未确认，不能当作尚未提交。无 token、授权码、client secret 或 registration access token。
- 成功响应先释放 callback listener，成功 client 写盘后再发布内存 Registered；结果写盘失败继续保持 Unconfirmed。运行实例仍持有任务并负责 shutdown／join。
- GET 内存缺失时读取原回执；准备流程先找回持久结果，不再新建注册候选。恢复为 Unconfirmed 后仍可读取另一实例后来保存的成功结果；不替换本进程活跃的 Registering。
- 固定版本、最多 16 条、严格字段／身份／callback 校验。无旧文件可正常使用；损坏或新版本不能降级为空存储；不自动清理已接受记录。只新增文件，未迁移或覆盖旧数据库。

## 验证记录

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-luma-registration-recovery-20261114`。

- `red.log`：先运行重建 Registry 后恢复未知回执的测试，因返回新 id 而失败，确认原缺口后才实现持久链路。
- 新增测试覆盖未知／成功回执恢复、owner 隔离、网络前持久标记、shutdown、存储失败禁止发送、结果写盘失败不得显示成功、两实例候选去重、容量／不可变结果、损坏文件及晚到成功的缓存更新。
- 只读审查发现“已缓存未知状态遮挡另一实例的后续成功”问题，已增加同 id 绑定校验及单向升级，并补确定性测试。
- `tests.log`：最终 Luma 相关 **143 passed、0 failed、2 ignored**，包含本批 9 项新测试；执行 28.10 秒，构建报告 19m20s（包含共享锁等待），不作为纯编译速度数据。两项 ignored 是既有独立浏览器专项。
- checker 31 项、ratchet、scoped rustfmt、范围空白与 UTF-8 无 BOM 检查通过。`scope-hashes.json` 固定 12 项源码并复核一致；新增持久层 141 行、异步存储桥接 60 行，所有本批文件均在 500 有效行以内；不改 baseline、policy 或例外。
- `production-check.log`：`cargo check --locked --offline --lib --no-default-features --features runtime-defaults -j 4` exit 0；构建含共享目录锁等待，仍有 warnings，不声称零警告。编译完成后再次核对 12 项源码哈希一致。
- 未提交、推送、出包或操作真实运行实例、凭据和数据库；新增状态文件只在合成测试数据目录中验证。

## 验收边界

恢复要求同一 state root、原管理 token fingerprint、Host（含端口）和 client IP；不承诺换身份／地址或跨机器、多节点恢复。本功能保护新版本写入的回执，不迁移调查 client，也无法恢复旧版本从未落盘且已经丢失的回执。不同本地候选竞争时只有持久胜者发送；输者保持自己的未知状态，不替换为胜者身份，重新进入准备流程才能取得权威回执。

测试使用重建 Registry、真实文件锁／原子 JSON 与合成 HTTP peer，不等同操作系统强制断电、完整进程崩溃、真实平台注册或原生包验收。refresh、模型发现、业务生成／取消／结果／费用、远端清理实测和完整账号联合继续保留。不减少整产品未完成数量。

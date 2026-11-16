# FreeBuff 浏览器批准：管理会话与服务端秘密保存

日期：2026-11-06。状态：本子批实现及下述定向验证通过；不是正常前端登录、R06/W04、P50 或总计划完成。
仓库：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`，`main`，HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`；保留既有未提交修改。
证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-management-20261106`。

## 1. 本批实际完成

延续[已验独立批准协议](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-06-provider-channel-expansion-freebuff-login-wire.md)，新增正常管理 HTTP、runtime-owned 会话与服务端保存。沿既有 FreeBuff session 产品，不新建 OAuth/API Key 池；累计新增正常模板仍为 16。

- 管理 API 为 create/status/poll/import/cancel；所有 mutation 复用正常管理身份和 Secret Grant/Origin，status 绑定同管理 token fingerprint、Host 和 IP。三组路由均 no-store，包含鉴权/JSON/body limit 提取错误。
- 创建、poll、保存准备均实查可写 coordinator、真实 provider/preset、原 revision 和最终产品目标。新 typed receipt 只接受固定 FreeBuff origin/原生 adapter，不接受携密自定义业务 path/header、其他 refresh owner 或冲突执行设置；旧 generic adapter 不因此迁移或改投产品。
- registry 最多 16 会话、600 秒登录 TTL；一次 poll 一个 paced GET，不重复 code POST，不新增无限账号轮询。初始化、poll、已接纳保存均为受跟踪 task，取消/过期释放未保存材料，正常 runtime shutdown 接 join。
- 保存使用普通 redaction→SecretPatch→正常 commit/CAS，不把 HTTP waiter 当保存 owner。20 秒包含 publication wait；成功才 succeeded，CAS 竞争/timeout/drop 为 save_unconfirmed，重复 import 不重投，已接纳保存不能通过 cancel 假装回滚。
- authToken 留在 credential 的 `api_key` 兼容槽，语义仍是 session Bearer；设备 receipt 在 credential-only `freebuff_session` 私有字段，独立脱敏/Replace/Keep/Clear，不继承 provider 或递归拼接，不放进业务 header/body。
- 重授权保留稳定账号 ID、原设备、模型/agent metadata、同池其他账号和无关秘密；显式换设备或错账号在发 code 前拒绝。不推导 agentId、token expiry 或 refresh。folder 原始 CLI receipt 暂不支持，明确拒绝，旧无 typed receipt 导入兼容保留。

完整接口和状态见 [FreeBuff 管理登录合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/provider-freebuff-login.md)。批准链接/服务端 receipt 不授予管理权限，不证明真实账号、模型资格或可用额度。

## 2. 失败现场与恢复

1. 首次 `red-secret-owner` 失败是已有 BoringSSL 临时构建输入不存在，`os error 3`，没有执行测试。随后 `red-secret-owner-v2` 到达真实行为红测：新增 fingerprint secret descriptor 不存在，1 failed；不是把环境错误冒充行为红测。
2. 接线后 `green-freebuff-v1` 编译报三处 `E0063 missing field freebuff_session`，已给既有账号组测试的三处初始化补 `None`。没有删除旧测试或放宽断言。
3. `green-freebuff-v2` 的工具句柄失效，复核记录 PID 已不在，且没有最终 receipt；原因未知，保留 `green-freebuff-v2-unconfirmed.json`，不计通过。`v3` 在当前源码重新验证，日志逐行刷新并保留最终 receipt。
4. 一次性 scope verifier 初次使用多一级相对目录，`ERR_MODULE_NOT_FOUND`；只修临时脚本路径，保留 `audit-bootstrap-error.json`，没有因此修改生产源码。

## 3. 最终定向验证

所有 Cargo 命令均 `--locked --no-default-features --jobs 4`。主配置为 `--features runtime-defaults`；代表性关闭为 `--features line-gemini-web-reverse`，不是全部 feature 矩阵。测试另加 `-- --test-threads=1`，TEMP/TMP 指向本批 `tmp`，没有设置 `RUST_MIN_STACK`。

| 阶段 / 选择 | 结果 | 实测编译 / 测试 |
| --- | --- | --- |
| `green-freebuff-v3`：`test --lib freebuff` | 156 passed / 0 failed / 0 ignored | 22m14s / 79.78s |
| `adjacent-secrets`：`test --lib console::secrets::` | 11 passed / 0 failed / 0 ignored | 4.11s / 0.05s |
| `adjacent-payload`：`test --lib db::provider_credentials::payload::` | 11 passed / 0 failed / 0 ignored | 1.29s / 0.01s |
| `adjacent-account-groups`：`test --lib account_groups` | 17 passed / 0 failed / 0 ignored | 1.47s / 1.23s |
| `production-check`：`check --lib` | exit 0 | 6m24s |
| `feature-off-management`：`test --lib disabled_freebuff_management` | 1 passed / 0 failed / 0 ignored | 9m48s / 0.01s |
| `feature-off-protocol`：`test --lib protocol::freebuff_login` | 6 passed / 0 failed / 0 ignored | 3.43s / 0.01s |
| `feature-off-production-check`：`check --lib` | exit 0 | 1m45s |

按完整测试名去重 **190 个不同 Rust 测试通过，其中本批新增 25 项**；共同测试不重复累计。八阶段各冻结 3060 项选定 Rust/配置输入，均 `changedInputs=[]`，收尾再次逐项与当前字节核对；不是全仓全 feature/集成测试通过声明。

真实管理 router oneshot + 随机 loopback 合成上游 + 正常配置 revision archive，覆盖丢失 HTTP waiter、重复 import/cancel、publication lock 超时、CAS 竞争、stable ID/device 重授权、秘密不公开、缺 Grant/错误 Origin/错误目标/JSON 限额拒绝、取消/TTL/capacity/join 和关闭 feature。这里 Redis backend 为既有合成 fixture，未用真实 Redis/PG，也不是管理 TCP/SQLite 重建。

Rustfmt、检查器 31 项、ratchet 4562 项与 `git diff --check` 通过；14 项 immutable runtime artifacts 单列，governed source 无 >700。静态结果复用本批已完成且源码未变化的检查，不无故重跑；既有共享行数报告原字节恢复。前序协议子批的扩大 Python scanner 失败没有在本批重跑或宣称清除。

41 项受影响源码最大 495 有效行，21 项新增源码最大 196；原 schema 328→330、secrets 393→397、账号组既有测试 335→337 / 494→495，其余逐项见 `scope-lines.json`。未改 policy/baseline/exception、依赖、storage version 或数据库迁移。已核输入/鉴权/秘密、目标注入、body/TTL/会话预算、锁顺序、取消和 task 生命周期；最终来源/编码/本地链接及差异核对见 [closeout.json](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-management-20261106/closeout.json)。

## 4. 下一接力点与未完成边界

1. 接正常 CredentialDialog 的 FreeBuff 浏览器批准 UI、同管理身份的会话重开/未知保存提示和稳定账号重新授权；复用最新有界管理会话框架，不能只把底层接口称作用户可用登录。
2. 正常 UI→真实管理 TCP/SQLite 保存与同目录 runtime 重建；独立进程退出/重启、FreeBuff-specific Server/Worker Redis、PG/Hosted 和跨进程 receipt reconciliation 单独验收，不借其他产品的证据替代。
3. 完整 agent map、session/run lease/取消、精确账号 probe、公共推理及失败/资源退出；没有 agentId 的批准 receipt 不自动成为完整业务账号。
4. 其他原始渠道、OAuth 兼容/恢复、Qwen 受限决策及 API/媒体未完项继续；P50 全范围无凭据整包、P60 集中录制、P70 逐渠道真实模型/额度/费用验收不删除。

本批未读取或试用 `.ng`/历史凭据，未调用真实平台、录制、付费生成、创建或操作容器、替换运行实例、出包、提交或推送。P34 的同一容器授权此前已消费并清理；历史问卷回放不是新许可，本批没有复用。总目标保持 active。

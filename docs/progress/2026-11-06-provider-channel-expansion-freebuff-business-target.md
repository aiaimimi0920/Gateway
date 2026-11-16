# FreeBuff：稳定目录选择、业务目标保存与旧调用隔离

日期：2026-11-06。工作区 `C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`，`main` / HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`。承接[设备状态与正常 UI](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-06-provider-channel-expansion-freebuff-device-ui.md)，保留 inherited dirty edits；HEAD 不是本批实现的交付凭据。不代表 R06/W04、整卡 offline_ready、P50/P60/P70 或总目标完成。

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-business-target-20261106`。最终以 `closeout.json`、各阶段 receipt、冻结输入表和失败日志为准。没有读取真实凭据、访问实际平台或购买会话；P34 Redis授权已消费，不因历史问卷回放再创建容器。

## 1. 正常编辑到原配置 owner

正常 `ConsoleGlobalDialogs → CredentialDialog → FreebuffCatalogPanel` 现在允许勾选当前目录开放且符合普通模型 ID 规则的稳定 key，经风险确认暂存到原账号表单，再按原保存流程发布。不是新账号或其他产品的通用导入，不把 handle/fetchId 当持久模型，不自动选择 locked/未知模型。

保留手动模型，替换 fetched 选择；模型、provider 汇总和路由集合沿原 `applyModelSelection` 对账，保留其他账号、其他 provider 的路由 ownership、disabled route 与 aliases。业务目标写入 credential-only `extra_body.freebuffBusiness = {origin, client}`；原登录 `base_url`、token/receipt、custom metadata 均保留。重开编辑恢复已保存目标与手动/拉取模型分类，不自动读取或注册。

dirty、pending、管理身份/Grant、revision、目标和取消隔离沿原 scope owner；只读目录仍不提交配置。应用需要同一目录/selection 对象和当前有效界面 scope，并经共享 ConfirmDialog 明示公共目录模式尚未接通，不购买会话。保存后敏感权限仍以服务端判定为准。

## 2. 配置与秘密的责任边界

严格目标仅接受两个已证 HTTPS 业务 origin 与 CLI/Desktop，拒绝 provider 继承、别名、未知字段和任意 URL。登录目标不被改投业务 host。正常 validate/commit 对比稳定 provider/credential ID；新增、修改或移除此账号目标都要求有效 Secret Grant，即便全部秘密补丁都是 Keep。commit 在原 publication lock 下读取 active 目标，再走原 revision CAS。普通目标不变的模型/metadata Keep、数组重排不被额外授权门禁阻断；整个账号删除不表示授权转移。

真实 TCP 红测证明旧路径无 Grant 仍返回200，现已修复。没有要求用户重新导出原 token，也没有放宽原 Keep 对产品、路径、receipt 或其他认证 owner 的绑定。

## 3. 不让稳定 key 误入旧自动会话

保存业务目标只是配置，不是 admission/费用授权。带此标记的 request plan 和旧执行器在任何网络前返回 `409/freebuff_business_admission_required`，不把 key 当 agentId，不退回旧 session POST 或 unsigned completion。caller body 中同名 runtime 字段会被过滤，不能借请求修改账号业务目标；旧未 opt-in 配置的既有行为保留。

这是有意的未完边界，不是公共业务实现。最终签名、显式 admission/reuse/消费确认、typed agent/lease、probe/取消及公共 JSON/SSE 仍须接力。

## 4. 已执行验证

- 最终 `runtime-defaults` FreeBuff 定向：220 passed / 1 ignored；跳过的是需单独启动的旧浏览器 host，不隐藏为通过。包含6项新增业务目标/隔离/授权/持久子链测试。
- 真实管理入站 TCP、production bootstrap/router、Local runtime/SQLite与持久配置：无/无效 Grant 的 validate/commit 拒绝、有效 Grant 保存、旧 revision 冲突、target 保留的 Keep、host/client 更改与目标移除、重建后的旧 Grant 拒绝及 fresh Grant 更新、两次同目录/同端口 runtime 重建；合成登录 peer 始终0请求，正常结束 server/owner/executor/SQLite/peer join。
- 生产 lib 与官方全 workspace Rustfmt 检查通过；共享管理相邻回归14/14、配置文档/秘密合同70 passed / 1 ignored、lib秘密变换12/12通过。后者与FreeBuff定向有重叠，不把阶段测试数相加冒充不同测试；额外ignored需显式导出合成前端夹具，本批未执行。五个最终Rust阶段各3246项选定输入无漂移。
- 前端9文件101/101、最终类型检查、隔离正常 main 与实际账号编辑宿主构建通过；三个最终阶段各951项选定输入无漂移。
- 浏览器使用实际产品组件与**合成 catalog API**：读取→选择→风险确认→暂存→正常账号保存到草稿→重开；reads=1、draftWrites=1、secretEdits=0，目标、手动模型与 fetched 分类恢复。390 CSS像素无页面横向溢出；截图与结果记录在证据根。不是浏览器→真实管理 TCP 的联合验收。
- checker31与最终ratchet4617通过，14 immutable runtime、governed source >700=0；本批23源码及1参考文档，源码最大398有效行，4新增源码均低于250。未改 baseline/exception；root有效行报告直接备份并原字节恢复。

最终来源/UTF-8无BOM/差异检查及资源退出状态，以 `audit.json` 与 `closeout.json` 单列，不能由上述定向通过推定全仓绿。静态helper经stop sentinel/绝对生命周期退出，最终精确命令行无对应Python进程；没有对在线实例执行停止。

最终测试后仅补全 `src/protocol/freebuff/catalog/REFERENCE.md` 的完成描述与回执链接；这是非执行文档，audit单列该差异，所有执行源码仍匹配最终验证输入，不为文案更新重跑已通过测试。

## 5. 失败记录与接力边界

- 保留配置/目录应用/确认的真实红测。前端参数表曾把数组展开为位置参数：Vitest 运行通过不足以证明预期 cases，最终 tsc 的 TS2345 暴露该问题；改成 `{keys}` 用例对象后101/101与类型检查重验通过。没有修改生产校验来迁就测试。
- 第一次后端授权红测实际200而非403；修复生产门禁。随后4 passed / 2 failed是路由夹具错误：配置账号 ID 应从 `candidate.payload.credential_id` 读取，`provider_credential_id` 是独立数据库凭据维度；并为夹具账号补其真实 model allowlist。按现有候选 owner 修正后完整定向重验通过。
- 临时 Node 行数测量第一次因 Windows ESM 未使用 `file:` URL 失败，改正脚本 import 后测量成功；不是源码或产品失败。此前失败日志、早期绿测和非最终输入快照均保留。

当前没有新模板或整卡 offline_ready。接下来优先实现 session/completion 最终签名与显式 admission/reuse/费用确认，再推进 typed agent/lease/probe/公共链。Server、独立进程/跨页面恢复、设备 UI/目录应用到真实管理 TCP 的联合验收、原生 EXE/整包与真实平台权益均另验；不把同进程 runtime 重建称为进程或 OS 重启。

未提交、推送、部署、出包或操作在线实例；未创建或清理容器。P50/P60/P70及其他原定渠道继续，不重复已验的登录/设备持久 owner/目录配置子链。

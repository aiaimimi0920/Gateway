# FreeBuff：正常管理原 claim、分页历史与显式结束

日期：2026-11-07。工作区：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`；`main` / HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`。实现包含未提交改动，HEAD不是本批交付凭据；未提交、推送、出包或修改运行实例。

## 1. 实现范围

本批只接已保存账号的离线原claim/分页历史和显式精确attempt end，不开放idle管理购买或公共guard。真实管理token/SecretGrant/expected revision在发送和归档边界重验；历史重复end零上游，旧attempt不作用于新active claim。目录模型被本地移除仍能收尾；缺金额/pending/grace/未知保留原义务，取消只停止等待，不自动重放。

- [管理reader](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/http/routes/internal_console/freebuff_claim.rs)与[原claim end](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/http/routes/internal_console/freebuff_claim/end.rs)复用既有`authorize_snapshot`：同一snapshot解析已保存账号的`freebuffBusiness`和原token，不接收调用者origin、client、模型、instance或秘密。两个POST均有16KiB body限制、strict DTO和成功/错误`Cache-Control: no-store`。
- [离线journal](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/local_runtime/freebuff_sessions/journal_view.rs)在单个SQLite读事务中读取active与历史；不占OS执行guard，不请求上游、不注册/修复设备、不生成key、不清记录。每页最多20项，额外一项探more，cursor只能是本账号已归档的canonical UUID4，不暴露全局rowid；同scope AEAD/attempt/receipt验证失败明确报错，不装作空记录。
- [sealed claim scope](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/freebuff/catalog/claim_receipt.rs)只给crate内本地可信reader使用，不是device/network/active capability；沿用原receipt解析和schema，无数据库迁移或凭据格式变化。token/origin/client改变仍阻断原记录，不能靠换scope看到空slot再买。
- 精确end先读取本地原claim。若已归档，仅返回原历史事实；若仍active，要求已保存registration、fresh signed catalog/fetchId及原recovery OS guard，再调用已有`SessionRecovery::end`。用户UUID只作exact target比较，不能直接构造远端claim。最终回执与原sealed preimage的加密归档/CAS释放仍由同一原事务负责。
- [正常UI](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/apps/desktop/src/features/console/FreebuffCatalogPanel.tsx)挂载[claim panel](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/apps/desktop/src/features/console/FreebuffClaimPanel.tsx)，即使当前保存模型列表为空也可显式读取。确认绑定账号/受众/revision/原attempt/model；scope、授权或dirty变化失效，确认消费一次，缺Grant恢复不自动重放，失败/取消要求重新读记录；late/unmount结果不覆盖当前账号。目录/设备操作在claim操作期间禁用，未保存目标不能借用。

## 2. 正常管理 HTTP 合同

`POST /v1/internal/gateway/console/freebuff-session-claim`

请求：`providerId / credentialId / expectedRevision / cursor?`。响应身份为`revision / providerId / credentialId / businessOrigin / client`，并返回`cursor / claim / history / nextCursor`；claim仅`attemptId / modelKey`，每条历史仅原`attemptId / modelKey`和typed最终receipt。返回的ID/事实不是消费许可，也不能将inspection中foreign active会话认领为本地claim。

`POST /v1/internal/gateway/console/freebuff-session-claim/end`

请求：`providerId / credentialId / expectedRevision / attemptId`。返回同账号身份与原`attemptId / modelKey / receipt / localReleased / fromHistory`。`localReleased`只指该原attempt，不能解释为账号没有更新的active claim；旧attempt重读历史不会结束新会话。最终退款0有效，未知金额不造0，pending/grace不释放；退款是上游Freebucks事实，不映射为Gateway用户余额。

LocalRuntime为显式必需owner；Server当前409，不回退另一目录。正常管理授权或revision在DELETE后失效时，原claim保留且不归档；这不是撤回已经发生的远端DELETE。正常新购买和public端点没有因此开放。

## 3. 验证、失败分类与证据

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-managed-closeout-20261107`。实际行为红测`ui-red-v1`因缺少“检查本地会话”按钮失败，不是import/编译红测。`typecheck-v1`暴露request schema union的泛型推断错误，显式声明返回union后`typecheck-v2`通过，没有降级schema或使用any。

`management-v1`完成编译但0 passed/10 failed：八项在最后清理触发既有`gateway-local-`路径安全断言，两项错误地在已被router使用的Arc上调用`get_mut`。修正新fixture的安全前缀，并在无owner/重开测试中创建独立AppState，重新申请管理Grant；未削弱公共清理guard或修改生产语义。失败日志和合成临时root保留，不计通过。随后`management-v2`定向10/10通过。

最终FreeBuff 316 passed/1 ignored、LocalRuntime 50/50、代表性仅关闭FreeBuff的管理行为测试1/1；前端4文件101项与类型检查通过。正常/代表性关闭生产lib、正常main隔离Web构建、15个Rust文件官方格式、checker31/31及ratchet4857文件通过。Rust按harness与完整名去重358 passed/0 failed/1 ignored（11新增）；失败分类、所选源码一致性、UTF-8/范围diff与公共guard/根报告原bytes保护见新回执audit；不叠加中间失败或历史通过数。

新增Rust行为保护10项，另有仅关闭FreeBuff的1项；前端新增27项包含在101项中。后端通过生产router、正常管理Grant、真实SQLite及回环合成上游验证：离线零网络/零新设备、strict请求/foreign拒绝、账号authority改变保留记录、执行guard持有时只读且end不能偷占、最终refund0/4.25和真实Ed25519/headers/空DELETE、非final/500/307保留、DELETE期间真实Grant/revision撤销、21条历史20+1分页/旧end不碰新active、上游关闭后的本地重开和sealed篡改拒绝。

前端验证是React单测；管理HTTP是production router测试和真实合成上游TCP，不是本批浏览器→后端TCP联合。正常main隔离Web构建只证明bundle能生成，不是浏览器运行、安装包或EXE验收。源码一致性按实际consumer核对：前端通过后仅改了三份后端测试fixture，前端所选输入保持相同；后续后端/生产/静态阶段按完整选定输入与终态核对，不把旧Rust快照算终态同hash。

最终10个后端/构建/静态阶段各5171项选定输入与终态逐项同hash；前端类型/UI两个阶段核对其未变化的`apps/desktop`所选consumer子集。初始来源只允许20个范围源码新增或修改，无旧输入删除。18份直接preimage、26个范围文本UTF-8无BOM、原公共guard/baseline/exceptions与根报告逐字节保护、15份隔离main bundle文件hash、范围diff及新增文件独立空白检查均见[audit.json](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-managed-closeout-20261107/audit.json)与[范围diff](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-managed-closeout-20261107/scoped-diff.patch)。所选输入不等于所有动态输入、安装依赖或环境均冻结。

本批20份Rust/TypeScript源码最大423有效行，新增生产Rust最大162、所有新增源码最大188；未改baseline或exceptions。八份原源码写前→写后：catalog 200→201、claim receipt 57→74、session journal owner 226→233、closeout 110→136、internal console接线421→423、management router 402→418、catalog UI 63→67、合成upstream fixture 142→145。两个已有管理接线文件保持单一职责且低于500，其余均低于250。生产lib仍有111/132项warnings，没有suppression或放宽准入。本批未重跑未改动的共享body suite，因此去重分母不同于前一批，不能以总数增减衡量完成度。

## 4. 接力边界

正常UI单测与真实管理router/SQLite分别已验，未执行浏览器→真实管理TCP的本批联合验收；不冒充原生包、Server或真实平台。逐次购买/价格余额确认与public caller/receipt/candidate、完整harness/工具/子agent与多步费用/取消FINISH、实时公共三协议、持久run/远端lease/probe/完整取消仍缺。模板仍17，无新增整卡offline_ready，P50/P60/P70和总目标继续；无真实凭据/供应商/Redis/Docker/现有实例/出包/提交操作，不复用已消费P34授权。

本批未新增公开wire调查，复用既有固定session end/receipt合同；不据旧参考声称当前线上或真实资格/费用有效。下一步优先补正常UI→真实管理TCP联合及公共调用触发的server-side pending/消费确认绑定，再接完整执行与结算；不新增闲置购买按钮来绕开既有quote owner的运行时边界。

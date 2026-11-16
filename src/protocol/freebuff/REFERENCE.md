# FreeBuff 业务协议参考与当前实现边界

2026-11-06 通过 GitHub connector 联网核实公开源代码，固定到
`CodebuffAI/freebuff` commit `6f567ec21542afc07dbe447bce0dcaf7a5a73a8d`。
仅阅读协议与类型，没有下载执行 SDK、代理代码、远程 agent 或调用平台认证/生成。
本目录旧 generic executor 不是当前产品业务合同已经实现的证明。

## 已核实的 wire

- [CLI session API](https://github.com/CodebuffAI/freebuff/blob/6f567ec21542afc07dbe447bce0dcaf7a5a73a8d/cli/src/utils/freebuff-session-api.ts)：POST 使用 `/api/v1/freebuff/session/admission`，携带模型与 wallet 标记；GET/DELETE 使用 session 路径。未知 POST 结果不能当作可安全重投。wallet 标记不是消费上限，确认在客户端完成。
- [共享常量](https://github.com/CodebuffAI/freebuff/blob/6f567ec21542afc07dbe447bce0dcaf7a5a73a8d/common/src/constants/freebuff-models.ts#L3874-L3895)：独立 admission/reuse 路径、精确 instance/model header；reuse 只复用指定 live instance，不购买或抢占。acting-user/privileged-user 是服务端专用，不是普通账号权限。
- [模型目录与签名类型](https://github.com/CodebuffAI/freebuff/blob/6f567ec21542afc07dbe447bce0dcaf7a5a73a8d/common/src/types/freebuff-model-catalog.ts)：目录 GET `/api/v1/freebuff/models`；持久 key、账号短效 handle 与 fetchId 不可互换。catalog 模式的设备 Ed25519 签名覆盖实际 method/path/timestamp/body hash/fetchId。fallback 模式存在，但不能据此假定任意旧静态模型现在可用。
- [客户端 session metadata](https://github.com/CodebuffAI/freebuff/blob/6f567ec21542afc07dbe447bce0dcaf7a5a73a8d/cli/src/utils/freebuff-session-identity.ts)：`freebuff_instance_id`、`freebuff_multi_session` 与 `surface` 由实际 host/session 产生，不来自公共推理请求的任意扩展字段。
- [catalog root agent](https://github.com/CodebuffAI/freebuff/blob/6f567ec21542afc07dbe447bce0dcaf7a5a73a8d/cli/src/utils/freebuff-catalog-agent.ts)：每次发送按目录 row.handle 构建 `base3-free-catalog` root，不把 key/handle 当 agentId。最终 [completion hook](https://github.com/CodebuffAI/freebuff/blob/6f567ec21542afc07dbe447bce0dcaf7a5a73a8d/sdk/src/impl/__tests__/request-headers-hook.test.ts) 验证最终序列化 body 与实际 POST `/api/v1/chat/completions` 一致；签名必须覆盖这些最终 bytes。
- [SDK START/FINISH](https://github.com/CodebuffAI/freebuff/blob/6f567ec21542afc07dbe447bce0dcaf7a5a73a8d/sdk/src/impl/database.ts#L419-L554)：run action/agentId/runId、状态/steps/credits。该 SDK 还可获取可执行 agent，Gateway 不为补 map 引入远程执行或默认 agent。
- [业务地址解析](https://github.com/CodebuffAI/freebuff/blob/6f567ec21542afc07dbe447bce0dcaf7a5a73a8d/sdk/src/constants.ts)与 CLI session API 均受应用部署 URL 控制；公开 CLI fallback 为 `https://codebuff.com`。[官方 smoke 构建](https://github.com/CodebuffAI/freebuff/blob/6f567ec21542afc07dbe447bce0dcaf7a5a73a8d/freebuff/e2e/README.md#L88-L92) 明确将 `www.codebuff.com` 业务与 `freebuff.com` 登录分开。不能从登录 origin 推断业务 host，也不能把 smoke 例子当正式 binary receipt。新独立 [catalog/device 子链](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/freebuff/catalog/REFERENCE.md) 只允许显式选择已证业务 host，尚未自动改变 typed receipt 或管理目标。

## 本轮实际修复

请求方的 runtime 配置字段不进入 wire body；自定义普通 metadata 保留，run/cost/client 及 FreeBuff session/surface 声明只能由执行器补入。非 object metadata 归一化为空 object，不能绕过必需 envelope。无实际 instance 时不保留请求方伪造的 instance。

session/run 的进程内 namespace 绑定 credential ID、当前 token 和规范化目标，即使账号 ID 稳定，换 token/目标也不会借用旧 run/session。各字段长度定界并 SHA-256 摘要，key 不含原材料；没有修改账号 ID、持久化 schema 或凭据内容。旧 namespace 的在途 owner 和 shutdown 清理由已有有界 registry/lifetime 持有，不声称重新授权立即撤回在途请求。

## 仍未完成

旧未 opt-in 配置的 executor 仍走 legacy session 自动 POST/轮询，不得据此执行当前产品验收。正常管理业务 origin/client、目录选择/保存、安装 key 与精确 registration 持久 owner 已接；带目录业务目标的公共请求仍在网络前拒绝，不回退 legacy executor。

独立 [signed business wire](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/provider-freebuff-business-wire.md) 已实现同 snapshot handle/fetchId、session inspect/CLI显式单次admission/heartbeat/精确attempt end，以及冻结最终body的completion签名发送；这是传输层，不是正常管理或公共调用已放行。消费确认/价格余额重验、持久pending/unknown claim与恢复、active业务准入、agent/run/lease/probe及公共完整JSON/SSE/取消仍须接线。dedicated reuse仅核到常量语义，没有完整method/body/header/handler，不猜协议、不回退自动POST。`cost_mode=free`、wallet=`session`或FINISH零credits都不是钱包零消费/退款保证。

登录协议仍沿 `../freebuff_login/REFERENCE.md` 的独立旧固定来源；本轮没有用新业务参考重写已验登录 receipt 合同。公开源内引用的 `docs/freebuff-model-catalog.md` 本次 fetch 返回 404，该不可读文档不作为已证依据。

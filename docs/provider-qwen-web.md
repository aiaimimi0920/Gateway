# Qwen Web 当前消费端调用合同

适用工作区：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`。本批为 R07 的限定单轮文本子链，不代表 Qwen Web 整卡完成，也不替代 Qwen API / OAuth。

## 公开来源与实现边界

2026-11-06 联网核实官方 [Qwen Chat](https://chat.qwen.ai/) 及首页引用的 [0.3.36 前端资源](https://assets.alicdn.com/g/qwenweb/qwen-chat-fe/0.3.36/js/main.js)。本地固定来源及窄摘录见 [sources-manifest.json](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-qwen-web-20261106-contract/sources-manifest.json) 和 [official-contract-snippets.json](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-qwen-web-20261106-contract/official-contract-snippets.json)。下载时的精确 UTC 未记录，manifest 的 `recordedAtUtc` 不是下载时间。

官方前端不是已取得开源授权的实现：这里只提取请求路径、字段和终态等协议事实，由 Gateway 自有框架实现，不复制压缩代码。MIT 的 `youssefvdel/qwengate@5220f7d3c50c2d9538ec3d6d393aed4f13369950` 作为交叉参考；GPL 的 `xtekky/gpt4free@5db80cc64b69377f65f11b64835b050e8d62b277` 只核对事实，不复制实现。错误命名的下载副本 `omni-qwen-web.ts` 已排除；旧 OmniRoute executor 不是当前平台证据。

## 正常入口与身份

保留正常 `qwen-web` 模板和 `qwen-web-chat` / `qwen-webui-replay-live` preset；typed `qwen_web_compatible` + `special_adapter=qwen_web` 使用独立的 [web_session owner](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/qwen/web_session/mod.rs)。旧非 typed 执行器保留，不把它的 signin/history flatten 能力冒称本批能力。

模板原 `https://chat.qwen.ai/api/v1` 仍是可读配置身份，不要求用户改数据库；实际 wire 固定为同一 HTTPS origin 的 `/api/v2`，不再重复拼接 `/api/v1/api/v2`。不允许 userinfo、查询、片段或其他 host，也不自动改投 `auth.qwen.ai`、DashScope、API Key 或 OAuth 池。

仅使用精确账号的 Bearer、完整 Cookie、可选原始 UA 和 Accept-Language；调用方 Authorization / Cookie / UA 不进入上游。Cookie 与 Bearer 标记为 sensitive，固定 Origin / Referer / Version / source，并生成 request ID 和 UTC Timezone。现有会话的到期时间不能当作 refresh；typed 渠道不进入旧 password/profile/browser 恢复。关闭该 Cargo line 后仍保留 typed ownership，在网络前明确拒绝，不能退回旧传输。

正常材料导入仍沿现有 SecretPatch / Grant / revision 保存链；可选原始 UA 在暂存到提交时重新核对，不能改名大小写或后来增加、删除、替换，也不能让 provider 默认值接管。后端 Keep 将 Qwen Web 纳入已有受保护产品集合，比较目的地、认证、协议/flow、有效 UA 和绝对 expiry；普通模型/标签修改不要求重录凭据。显式 probe 使用正常 Grant 和精确账号选择，只允许当前配置的模型身份及正常别名解析；默认检查、不启用账号、无 Grant 或未知模型不发送生成请求。probe 的当前 owner、脱敏及 recording seam 复用现有机制，不另建测试代理。

## 已实现的单轮调用

沿现有三适配器、公共 Chat / Responses、精确凭据路由、审计和取消链执行：

1. 任何 create 前校验：只支持一个 user 的一个非空文本 part，原空格保留，最大 64 KiB；不默默 flatten system、多轮、工具、附件、reasoning、previous response/session key、生成参数或未知 raw 参数。
2. 一次 `POST /api/v2/chats/new`，发送官方可见 create 字段，必须收到合法非空 `data.id`。
3. 一次 `POST /api/v2/chat/completions?chat_id=...`，使用同一会话 ID、当前用户消息和明确禁用的搜索/工具开关，不凭空导入 OpenAI 全历史。
4. 严格增量 SSE 解码；answer 为正文，不把 think 文本混进正文；阶段 `status=finished` 不代表响应成功。仅明确 `[DONE]` / `done:true` 且正文非空才能完成。error 优先于同帧 done；`response.stopped`、坏 UTF-8 / JSON、未知或未支持的输出、无终态 EOF 均失败。
5. 已报告的 input/output 或 prompt/completion usage 才计入；缺字段不补零，不凭文本估算或拿公开模型列表当账号权益。

一次调用总预算不超过 120 秒，create 含 body 不超过 15 秒，completion headers 不超过 30 秒；create body 64 KiB，SSE 累计 8 MiB、单行/事件 1 MiB、16384 事件、正文 2 MiB。无自动重放、redirect、换号或跨渠道 fallback；typed 错误使用固定 `qwen_web_current_*`，不泄露远端原文。流为 pull-driven，无本批后台 relay；本地 Drop 释放 HTTP body，不造成功终态。

旧 buffered accumulator 另外补明确终态与晚错误校验；历史支持的普通 JSON、无空行 data 和明确 `finish_reason=stop/length` 仍保留。这不是宣称旧流式 translator 或历史 flatten 已完成整体治理。

## 仍未完成

后续 [普通余额Key真实TCP联合](progress/2026-11-12-provider-channel-expansion-qwen-prepaid-tcp.md) 已把正常管理创建Key、message/token余额、真实下游HTTP请求与合成上游接成同一次流程；成功按规则结算、截断和客户端增量后断连释放预扣并关闭上游。此处不再仅oneshot，但仍非真实供应商/PG/现金/成品验收。

后续 [混合成功/取消结算](progress/2026-11-12-provider-channel-expansion-qwen-token-mixed.md) 已验证同Key重叠流在两种终结顺序下只退取消者预扣，保留成功实际消费及欠额，后续请求仍拒绝。仅本地合成合同，不是供应商退款、PG或跨进程验收。

后续 [两个已获准请求超额累计](progress/2026-11-12-provider-channel-expansion-qwen-token-concurrent.md) 已验证本地同Key两个流式响应先后/同时消费时，两笔usage完整累计且不覆盖另一预扣；合并欠额后拒绝新请求。仅current-thread单实例合同，不是PG/跨进程/压力或现金验收。

后续 [欠额后追加额度恢复](progress/2026-11-12-provider-channel-expansion-qwen-token-recovery.md) 已验证同Key正常tokenDelta追加先抵欠额，余额0仍拒绝，足额后恢复调用、按实结算并再次耗尽拒绝，总量不重置。此为token管理接口合同，不是支付/现金充值验收。

后续 [超预扣及欠额结算](progress/2026-11-12-provider-channel-expansion-qwen-token-overage.md) 已验证既有token规则：buffered/SSE有usage时按实扣减，超过余额后保留负值，下一请求拒绝且不增加上游调用。不是现金透支授权，不代表并发超额、充值恢复或供应商真实计费验收。

后续 [精确阈值与重叠准入](progress/2026-11-12-provider-channel-expansion-web-token-competing.md) 已验证：未知usage成功在精确预扣余额下归零；两个重叠请求只有一个获准，被拒绝者不误退，取消获准者后只恢复一次。限单实例本地合成合同，非有usage欠额结算或跨进程/PG压力验收。

后续 [非流式结算与不足准入](progress/2026-11-12-provider-channel-expansion-web-token-admission.md) 已补齐 buffered 成功 token 结算，并验证0/1 token不足余额在普通/流式入口均429且零上游调用。仍不代表现金、并发或供应商收费验收。

同日补充 [token 预付费合同](progress/2026-11-12-provider-channel-expansion-web-token-prepaid.md)：成功流有 usage 按实结算，无 usage 保留既有预扣且审计仍未知；失败和增量后取消释放预扣。仅本地合成合同，非现金/供应商收费证据，buffered 成功 token 结算未由本批覆盖。

2026-11-12 已补[普通 Gateway Key 按次数预付费合同](progress/2026-11-12-provider-channel-expansion-qwen-prepaid.md)：生产本地状态下，成功请求在已报告/未报告 token usage 时均扣一次，失败或增量后取消释放本地次数预扣。下游为 router/body drop、上游为合成 peer；不是现金计费、真实供应商扣额或远端退款证据。

- 当前官方 signin 在独立 `https://auth.qwen.ai/api/v2/auths/signin`，构造使用一次 SHA256 password；返回 envelope、MFA/OTP/challenge、Cookie 与真实登录未验。本批不开放旧 seed 导入，不进行密码猜测或多种哈希重试。
- 多轮所需的远端会话、parent/response ownership 与恢复；system、工具/local MCP、搜索引用、附件、多模态及其他已声明能力。
- 账号可用模型/权益与独立额度；公开目录只是公开元数据，不是当前账号授权。没有写死“目录里有即账号可调用”。
- 真实 token/Cookie 轮换、Set-Cookie 持久 owner、SSO/challenge 恢复、平台 TLS/WAF 接受性及当前余额/收费。
- stop 路径虽有官方证据，但完整请求体未追完；不能猜请求。参考中的 `DELETE /api/v2/chats/` 是删全部会话，禁止用于请求清理。当前取消不承诺远端停止生成、退款或删除会话。
- 独立进程/Server/真实 PG、devtest 整包、原生 EXE、集中录制与真实平台验收。

离线定向检查、真实红测和后续结果应以本批进度文档及 `linshi/gateway-qwen-web-20261106` 的最终回执为准；尚未完成的检查不得由本合同文字替代。

2026-11-12 后续已补 [公共真实 TCP](progress/2026-11-12-provider-channel-expansion-qwen-public-tcp.md) 及 [正常网页材料保存/精确账号 probe](progress/2026-11-12-provider-channel-expansion-qwen-probe-ui.md) 的合成联合验收。后者覆盖普通 Grant/CAS、新会话材料实际出站、唯一审计和集中结果页，尚不证明重启恢复、普通 Key 余额或真实平台可用性。

同日已补 [本地 authority 恢复合同](progress/2026-11-12-provider-channel-expansion-web-session-restart.md)及[管理 TCP 保存与恢复联合](progress/2026-11-12-provider-channel-expansion-web-session-management.md)：正常 Grant/secretPatches/CAS 写入后，新管理状态从真实磁盘恢复材料，旧 revision 不得覆盖；发布前后中断恢复另有阶段注入测试。这不是实际应用进程重启、真实登录轮换或上游权益验收。

同日进一步完成[正常网页跨测试进程联合](progress/2026-11-12-provider-channel-expansion-qwen-process-ui.md)：网页保存到真实local后首宿主退出，不同PID恢复同revision/材料，无重新导入，网页精确probe实际使用恢复bearer/Cookie/UA完成一次create/completion，结果与唯一脱敏审计一致。此处是测试宿主进程重启，不是成品EXE/全启动/真实平台验收。

# 服务商分渠道扩展：第 27 批，Grok 授权上下文与专用刷新

> 2026-11-03（Asia/Shanghai）；R05 / W03 / P20。本批定向验证已收尾，Server 交接缺口转 P28；不是完整渠道或总计划完成。
> `main`，HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`；未提交、推送、打包或接管运行实例。

## 1. 范围与兼容路径

[P26](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-03-provider-channel-expansion-p26.md) 已接固定 Build 业务受众，但因缺少授权上下文暂时拒绝全部 legacy refresh 配置。本批继续接真实的专用刷新，不把该临时拒绝当作最终支持。

新增上下文只属于 credential，不逐字段继承 provider 默认。内容限固定发行方、显式 client、请求/返回 scopes、consent 预选及实际 principal hint；它不含 token/client secret，也不是可授予管理权限的 receipt 或账号权益证明。

旧读写格式保持可用：新字段缺省为 None，序列化省略，不修改旧 refresh 字段的缺省，不 bump storage version，不迁移任何现存数据。新上下文由新 reader 保存，旧二进制不应打开并重写新增渠道配置；回退使用原先保留的旧实例/数据副本，不删除新字段强行降级。

复用既有 single-flight、generation、退避、rotation writer 和 revision/CAS；不新增刷新 scheduler。正常管理登录/设备监听、Secret Grant、从实际 receipt 的正常保存和抗 caller drop 的提交任务仍由后续工作接通，本批不通过手工 Binding 冒称完整 OAuth。

## 2. 证据与验证

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-provider-expansion-20261103-p27`。
源码修改前保存 `source-before.json`，扩 scope 前保存 `source-extra-before*.json`。`red-context-v1.log` 实际编译10m24s，5项中2 pass / 3 fail / 0 ignore：旧 schema 丢弃 context、旧配置暂拒所有 refresh、Keep 不识别 context 变化。两项已通过的负测不计作修复。此阶段同时采集旧格式 JSON/YAML canonical digest，后续测试固定两项原值，不通过更新期望值掩盖漂移。

## 3. 当前实现与审查边界

- credential-only typed `grok_oauth` 保存固定 issuer、显式 client、requested/granted scopes、consent 预选及实际 principal hint；拒绝未知字段，token/client secret 仍由既有秘密字段拥有。正常 receipt 可导出上下文，不把可反序列化元数据变成授权凭证。
- 有 context 时派发专用固定 token endpoint，校验显式 legacy endpoint/client 的一致性；没有 context 的旧 refresh 配置仍拒绝。专用 owner 不继承共享客户端的 Cookie/Authorization，不跟随重定向；缺少明确 expiry 的回包不冒用通用六小时默认值。
- 复用 single-flight/generation/backoff、PendingRefresh 和正常 automation CAS。上下文同时参与 Keep、publication、迟到结果授权比较和 Redis 原 raw-entry CAS；Hosted merge 不继承或拼接 account-level context。
- 明确返回的 scope 缩小与 token rotation 同步写入 runtime、local CAS 和 Redis cache，下次以已观察范围为上限；不是只检查一次子集然后丢弃。opaque access token 可保持原 principal hint，不伪造刷新后的验签保证；返回的可读 principal 冲突则拒绝。
- 只读复审指出 **Server 模式 policy 替换仍有运行态交接缺口**：enabled/pool policy 改变只共享 attempt lock，交换中旧 owner retire 后 Server 分支会丢弃迟到 rotation；成功 runtime 缩权后再替换 policy 也可能从旧 document 重建上下文。该路径部分继承旧逻辑，但与新可变 context 交互必须补专门保护，不能以本地 CAS 或正常 Server cache 纯函数通过冒称已闭合。

最终 `source-frozen-v1.json` 冻结23项源码。`green-refresh-v1.log` 33项通过，其余10个相邻过滤器合计129次通过；`audit-results.mjs` 按测试全名去重后为 **155 pass / 0 fail / 0 ignore**，162次执行包含7次交叉重复，不把162报成独立测试数。测试进程沿用16MiB `RUST_MIN_STACK`，不是生产线程设置。

`feature-disabled-v1.log` 的代表性 `line-gemini-web-reverse`、无默认 feature 的 lib check 通过（1m46s），不代表全 feature 矩阵。Rustfmt、checker31/31、ratchet4070文件/14 immutable/governed source >700=0及 tracked diff 通过。`audit-final.mjs v1` 核对23项源码哈希、28份 UTF-8无BOM文本、84个本地链接、围栏与含未跟踪文件的 whitespace；P28改源码前再次通过。后续批次可以修改这些源码，不能拿当前源码重跑旧冻结审计后期待继续匹配。

新生产 context150、专用refresh71有效行；新测试最大234，受影响现存源码最大491，没有新增500行以上文件，没有改 baseline/豁免。红绿日志、去重清单、前后有效行与冻结哈希均留在上述证据目录。本批范围不包含已确认的 Server policy/enable 替换缺陷，后续见 [P28](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-03-provider-channel-expansion-p28.md)。

没有读取真实 `.ng`、历史凭据或在线数据库，没有向真实上游发送授权、刷新、推理或生成请求。

## 4. 未完成的全局范围

R01–R16、Vertex Express/Luma Agents 等必要分支、Qwen/Bedrock 剩余保真与 PG 审计/取消、媒体生命周期、P50 包和隔离启动、P60 集中录制与 P70 真实逐渠道验收全部保留。API Key/OAuth/Web 依产品、受众和额度隔离，unknown 不合池；Web 不作为前两种不存在时才使用的兜底。

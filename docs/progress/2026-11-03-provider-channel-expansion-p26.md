# 服务商分渠道扩展：第 26 批，Grok Build 产品身份与 Responses 业务边界

> 2026-11-03（Asia/Shanghai）；R05 / W03 / P20。最终163项定向离线测试通过；不是完整 OAuth 渠道或总计划完成。
> `main`，HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`；未提交、推送、打包或接管运行实例。

## 1. 真实前置与范围

[P25](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-03-provider-channel-expansion-p25.md) 已实现协议 acceptance 和实例会话，但没有独立可保存的 Grok Build preset。不能让新的正常管理入口只接受一个实际无法编译保存的 provider，也不能借 `xai-api` 或 `grok` Web 身份代替。

本批先补真正的产品和业务发送前置，再继续管理/保存；不把这一前置缩成最终交付范围。参考仍为 [授权合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/plan/provider-auth-contracts.md) 中已固定的官方 Grok Build 源码，不安装或执行参考项目，不内置其 OAuth client ID。

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-provider-expansion-20261103-p26`。

## 2. 本批实现

- 独立 `grok-oauth` backend preset、`grok_build_oauth` auth marker/profile 和 Responses family；空池无候选、无默认模型。它不是第十个前端模板，正常登录 UI 尚未开放。
- `protocol/grok_oauth/business.rs` 在无 flow shortcut 前保持身份；固定 `https://cli-chat-proxy.grok.com/v1`、POST `/responses`、Responses response kind、直接 HTTP，禁止 API/Web 目的地、外族指纹、session/browser、发现结果和其他端点覆盖。
- 原始 preset 锚定的配置编译校验覆盖最终 parent 和每个 credential（含 disabled）；同时识别无 preset 的 marker、固定业务域名及 credential override，不因省略 preset 或晚期改 base/auth 就丢掉产品身份。
- 复用共享 Responses codec，Chat/Messages/Completions 可以桥接，但出站仍固定 Responses；最终 defaults/profile merge 后重新检查 plan，不以入站端点当出站解码类型。
- Build 不进入 native Codex/compact/WebSocket 或通用 Codex/Claude 指纹。最终 sender 校验 bearer 与 payload 一致，拒绝混入 Cookie、Host 和其他认证材料，移除调用者外族软件指纹，使用 Gateway 自身 User-Agent 与已观察到的 Build wire selector。该 selector 不是第一方 OAuth client ID，也不证明厂商客户端使用授权或额度。
- buffered/stream 共用发送边界，禁重定向；SecretPatch keep 复用受保护产品的目的地/消费者检查，不允许保留 token 后改投 xAI/Web。
- 模型 tiering 的 generic lookup 在 ChatGPT 分支和任意 `modelsPath` 解析前拒绝 Build 材料；普通 connectivity probe 同样在网络前拒绝，UI 报 unsupported 而非连通成功。配置的模型清单仍可作静态目录，不冒称已调用或已获得权益。

**刷新限制是未完成项，不是功能支持：**当前专用 refresh context/scheduler 尚未接通，因此暂拒 legacy refresh 配置，避免误走通用 form refresh。后续必须实现专用上下文、generation/rotation/CAS 和重启恢复，不能以此限制关闭需求。

## 3. 测试与来源状态

- `red-product-v1.log` 实际编译失败，既有缺失 `business` 实现，也有测试作者写错 `normalize_openai` 函数名；不能把全部错误冒称纯生产缺陷。
- 已修正为真实 `normalize_chat_completions`；`red-product-v2.log` 实际 exit101，仅剩三处缺失 `business`，属于新功能缺实现红测，不是线上行为复现。
- `source-before.json` 在 P26 新测试前保存；`source-extra-before-v2.json` 在额外源码修改前记录 hash/有效行；完整 `source-all-frozen-v3.json` 在测试前冻结27个现存源码文件，包括未改但纳入边界审计的管理接缝。
- v3 实际 exit101：新测试误把 snapshot 当作直接含 providers 的对象，已按真实 `get_providers()` 接缝修正；没有将此测试作者错误当作产品修复。v3 静态检查通过，但不作为最终功能来源。
- 独立只读复审发现并确认三项真实接线缺口：无 preset 时可绕过暂禁 generic refresh；generic 模型 lookup/连通性 probe 可携 Build bearer 绕过固定受众；foreign header 名单漏掉 ChatGPT account。v4 已修补并补负测，复审静态确认闭合；未运行测试的审查不能算动态通过。
- `source-all-frozen-v4.json` 冻结31个现存源码；v4 编译12m44s后，实际运行64项时发生 Windows `STATUS_STACK_OVERFLOW`，没有正常测试汇总，不算通过。在同一源码上，仅为测试进程设置 `RUST_MIN_STACK=16777216` 后重跑；脚本结束恢复原值，不改生产线程设置。最终 `source-all-frozen-v5.json` 与v4源码一致，`green-product-v5.log` 实际64 pass / 0 fail / 0 ignore，缓存检查4.36s、测试15.14s；不是EXE构建时间或生产栈验收。
- 相邻回归：refresh17、call-flow40、registry15、request-profile6、native-codex21，全为exit0且无fail/ignore。`audit-results.mjs v5` 逐个解析测试名，六个过滤器无重复，合计163个实际执行通过的测试，不累计v3失败或v4异常终止的部分结果。
- 测试使用合成材料、显式随机 loopback；测试 remap 仅存在于 `cfg(test)`，且位于生产 plan/header 校验之后，不增加 release 配置或环境变量逃生口。实际 sender 合同只证明请求和原始流读取，不替代全部公共 router/完整 SSE 保真、管理HTTP保存或真实上游验收。
- 扩展 lookup 范围时先保存 hash，事后由精确匹配原 hash 的原文补测有效行。`profile_inference` 的拟 before 命令异步返回，实际 hash 已是修改后，不能冒称事前快照；改前247行取自 HEAD，并验证当前文件恰好只增加一条分支。细节保存在 `source-extra-before-measured-v4.json`，不覆盖或伪造旧记录。
- 最终 `feature-disabled-v5.log` 的代表性产品关闭编译 exit0，耗时2m07s；不替代全 feature-off 矩阵。`run-static.ps1 -Version v5` exit0：Rustfmt、checker 31/31、无报告写入 ratchet（4064文件、14项 immutable、governed source >700=0）和 tracked whitespace 检查通过。
- `audit-final.mjs v5` 核对31项冻结源码 hash，35项源码/文档为 UTF-8 无 BOM，83个本地链接存在，围栏及含未跟踪文件的独立 whitespace 检查通过。本批现存范围最大454有效行，新增模块均低于250，无 baseline 或豁免变更。

没有读取真实 `.ng`、历史凭据、在线数据库；没有向实际上游发送授权、刷新、推理或生成请求，没有费用或在线实例变更。

## 4. 保留的下一步

1. 在正常管理鉴权和 Secret Grant 后读取真实 provider/preset/expected revision，不能相信外部传入的 Binding。
2. 正常 code/device 会话、loopback listener、状态查询与取消；会话终态可观察与清理必须有界。
3. 普通 SecretPatch/CAS 保存及独立拥有的提交任务，覆盖 caller drop、冲突、退出和结果查询；保存本次真实 client/scopes/principal 上下文快照。
4. 专用 refresh dispatch 复用现有 single-flight/generation/rotation writer，授权等价比较纳入完整上下文；补重启恢复。
5. 正常模板/UI、完整响应保真/模型策略、客户端授权边界核实、P50 包与隔离启动、集中录制及真实验收。

R01–R16 及必要同平台分支全部保留。其他 OAuth/API/Web/媒体、Qwen/Bedrock 剩余项、旧 feature/前端/expiry 未通过项不因本批消失；总 goal 保持 active。

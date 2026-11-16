# ChatAIBot：精确图片测试、逐次费用确认与正常账号入口

2026-11-07。本批完成 R13 的无凭据显式媒体 probe/UI 子链，不代表完整 ChatAIBot 渠道、P50 或总目标完成。工作区 `C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`；保留继承的 dirty edits，未提交、推送、部署或出包。

证据目录：[gateway-chataibot-media-probe-20261107](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-chataibot-media-probe-20261107)。`before/` 是本批直接 preimage；`run.py`、各阶段 JSON/log、`passed-rust-test-names.json`、`audit.json`、`scoped.diff` 和 `closeout.json` 分别记录来源、失败及最终检查。不读取真实 `.ng`/历史凭据，不发真实认证或生成，不使用 Redis/容器，不重放已消费的 P34 授权。

## 1. 改了哪些内容

- 正常账号“测试”按 ChatAIBot 产品进入图片表单，不再进入文本计划。只允许手动、显式模型、一个精确账号、`image` 或 `image_edit`；单张源图走编辑，2–4 张走既有合成。生成不允许附图，video、mask、显式比例与未实现参数仍在网络前拒绝。
- 使用同一个管理 provider probe 接口、Secret Grant、Origin、启用状态、账号归属、模型 allowlist/alias 解析及 revision/轮次 owner，不新增未认证入口、传输层、凭据 store、普通 Key 或调度器。只支持 typed `chataibot_web`；legacy 不借测试偷偷写旧账号设置。
- 既有 Luma 显式媒体 owner 迁入 [media_model_probe.rs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/provider_runtime/media_model_probe.rs)，共用精确账号准入、一次普通 sender、audit/health、取消和结果结构，按真实产品分流 canonical、family 和成功语义。Luma native generation/credits 保留；不把 Luma source/参数套给 ChatAIBot。
- ChatAIBot 参数预检调用原 sender 的 `Prepared`，没有复制第二套规则。真正执行仍是一次提交→同 job 轮询→受控下载及完整有界 PNG/JPEG 验证；只有 sender 成功后才恢复精确 resolved model 健康。结果 `answer=None`，不作文本/IQ 评分。
- 审计仅增加封闭的 `chataibotImage={verified:true,count:1,supplierCharge:null}`。不复制 URL/base64/提示词/任意上游 metadata；不编造 native job ID、Token、零扣额或旧价格估算。管理媒体 probe 不扣普通接入 Key 余额。
- 默认、普通文本、保存/临时/命名/自动文本计划不生成图片。HTTP、坏 job、无效图片、timeout 或 caller Drop 不重提、不换号、不 fallback；本地取消不保证撤销远端任务或退回供应商额度。

## 2. 前端与费用授权

[正常表单](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/apps/desktop/src/features/console/ChataibotMediaProbeDialog.tsx)只展示实际控制项、必要错误及结果；费用风险只在每次运行的即时确认中出现。不保存或继承费用授权，不提供自动媒体测试。

模型只来自显式配置和映射，不自动填协议默认型号。按原生已实现能力过滤生成/编辑；保留请求 alias。后端会随机选择可用 multi-target，所以 UI 要求全部账号可用目标都支持当前操作，不能只挑有利的一项。该静态能力表不是当前官方目录或账号权益证明。

源图使用本地文件选择，不提供 URL/generation ID 语义，不自动预览或联网取图。最多四张 PNG/JPEG/WebP/GIF；原文件合计 135000 bytes，data URL 合计 180000 字符，整个请求 JSON UTF-8 仍不得超过管理 parser 的 262144 bytes。此处为小样本 probe 的独立预算，不缩小普通公共图片入口的既有预算。替换、目标变化或卸载会中止自有 FileReader，generation 屏蔽迟到回写；操作切换清空隐藏图片。

pending 同时绑定目标配置和完整请求；目标、输入、dirty/revision 门禁改变即撤销确认，确认时再次比较。关闭先取消本地轮次，再交原 session owner Abort HTTP；本批表单测试直接证明本地迟到结果隔离，既有 controller 测试证明其卸载 AbortSignal，不将二者冒充完整真实浏览器→管理 TCP 的联合验收。

只读复核发现 `callFlow` 与 credential `auto` 的 UI 误拒绝：两项实际红测后修正，前端现按 Rust compiler 使用 provider flow，接受其 `callFlow` 序列化别名，不自行发明 credential flow override。

## 3. 验证及来源边界

- 后端首轮红测 **1 passed / 1 failed**：原媒体 dispatch 将 ChatAIBot 拒为 Unsupported；不是编译失败替代红测。其 4712 项选定输入期间只新增/变动前端文件，回执如实 `source_unchanged=false`，没有 Rust 输入漂移。
- 前端入口红测 **0/2**；补充 flow 兼容红测 **0/2**（名称筛选其余 14 skipped），最终全量选定用例均转绿；不把筛选 skip 当通过。
- 最终正常 `runtime-defaults`：ChatAIBot **47/47**（8 新增），涵盖三类图片操作、alias/精确身份/验证图 audit、普通 Key 不扣、Grant/Origin/账号/参数拒绝、文本计划不生成、HTTP/坏结果/总 timeout 与三阶段 Drop/source 关闭。
- 相邻 `luma` 名称筛选 **120 passed / 1 ignored**，包括既有 Luma Web/helpers 和 Agents API，不称纯 Agents120；共享 probe/audit/identity/Local recovery **26/26**。上述 Rust 按完整测试名称去重 **193 passed**。ignored 是已有显式 browser host，未运行且不计通过。
- 最终前端八文件 **105/105**（21 新增），含 ChatAIBot 表单/alias、文件选择/编码预算/reader 替换及卸载、目标及 revision 变更、每次费用确认、关闭迟到/跨账号结果隔离、Luma 表单及既有 session/controller/run owner。完整 TypeScript typecheck 通过。
- 非测试正常生产 lib 和仅关闭 ChatAIBot 线的生产 lib 均编译通过；Rustfmt、checker **31/31**、effective-line ratchet 和 diff/编码/引用检查见最终回执。本批没有增加依赖、修改 baseline/exception，也没有以编译代替真实平台验收。
- Rust green 构建期间只继续修改前端，完整选定集合如实有漂移，Rust 子集不变；后续相邻/生产/关闭/最终 UI 和类型阶段各 4720 项选定输入前后无漂移。最终当前来源按各自适用子集核对，不称全仓冻结；runner 不覆盖所有动态夹具、PowerShell、Python、外部工具或全 feature 矩阵。
- 16 项当前受审源码最大 447 有效行；console model probe 436→447，其余当前模块不超过 250。六处既有源码调整、一处媒体 owner 迁移和九项新源码符合门禁；没有 >700 的增量例外。根有效行报告用本批直接 preimage 逐字节恢复，不恢复旧批快照。

## 4. 明确未完及接力

没有真实 Cookie/challenge/refresh/登录恢复、当前完整型号/比例/output preset、其他认证和独立额度证据；没有供应商实际扣额终态。正常材料/Keep 联合、独立管理 TCP/跨进程、Server/PG、原生 EXE/整包、真实平台生成及 UI→真实后端端到端仍需继续。共享 CAS 已有相邻测试，本批新媒体测试未单独预置每一种 blocked/并发新状态，不能外推跨 store 原子性。

累计新增模板仍 **16**，没有新增整卡 `offline_ready`。FreeBuff 公共业务、Luma Web、Suno/Udio 当前媒体及精确测试、Kiro OAuth 新模板/兼容、Qwen OAuth 待决等继续；P50 全范围无凭据集成/devtest、P60 集中录制、P70 真实逐渠道验收均未完成。

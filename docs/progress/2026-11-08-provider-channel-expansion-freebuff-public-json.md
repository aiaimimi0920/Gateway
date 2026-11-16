# FreeBuff 公共 Responses / Messages 非流式 JSON 子批

日期：2026-11-08。工作区 `C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`；分支 main，HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`。保留继承 dirty/untracked 内容，不提交或推送。

## 完成的生产接线

目录 CLI / Local 公共 `/v1/responses`、`/v1/messages` 的限定非流式文本／function-tool JSON 子集已接原有 public owner；Chat 保持原生输出。正常公共鉴权、原请求绑定、逐次购买批准、Grant/revision/候选围栏、持久 claim、签名 START/model、已报告费用、显式 FINISH、取消／期限和 Local 结算没有新建平行执行器。

输入先检查原 `raw_body` 的闭合字段与模态，再映射到原 `RelayStep` 二次校验。system/developer 和混合文本／工具结果按原顺序保留，不借通用 pack 丢字段或注入 CLI system。Messages 展开有累计 4096 条的前置限制；序列化字节按实际 JSON escaping 计数。canonical policy 已改变的语义不被 raw 恢复覆盖。

输出复用框架 envelope／finish 规则，但不使用可能解析文本工具或补默认工具参数的宽松 unpack。标准文本、函数调用之外，reasoning/details、原 finish、费用／部分 usage、工具原生扩展显式保存到 `gateway_freebuff`；不冒充 Anthropic thinking/signature，不克隆整个原生 completion。Messages 原 arguments 字符串另存，以免解析 JSON 后丢失 lexical 数据。

无法表达的多 choice、缺失／重复工具 ID，以及 Messages 非 object 工具参数等，在 FINISH 前成为失败；仍由原 ledger 显式发送 failed FINISH 和实际 reported credits，不补零、重发、重买、end 或认领旧 claim。

精确字段、扩展格式和限制见[公共 JSON 合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/provider-freebuff-public-json.md)。扩展只保存观测，不是隐藏 continuation 或新的执行权限。

## 验证与证据

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-public-protocol-json-20261108`。

- `json-red`：新增 Responses 公共请求在旧实现下确实提前结束、无法产生 pending，1 项行为红测失败；不是以编译失败冒充行为红测。
- `json-initial`：首次接线有 Rust re-export 可见性错误 `E0364/E0603`，已在责任层修正，保留原日志。
- `json-focused`：中间 39 passed / 0 failed / 1 ignored，但执行期间 4 个文件有审阅修正，source_unchanged=false，**不作为最终源码验收证据**。
- `freebuff-final`：**473 passed / 0 failed / 4 ignored**，包含本批 10 项新增（7 项公共 TCP、3 项纯映射／边界测试），不与中间通过数相加。按完整测试名对照上批，0 项旧身份丢失；4 个 ignored 为 3 个旧 opt-in 浏览器夹具与进程 child entry，原 parent 内六阶段真实 kill 场景仍在已通过集合中。
- `production`、`production-freebuff-disabled`：正常 runtime-defaults 与仅关闭 FreeBuff 的代表性生产 lib 均实际检查通过；不等于完整 feature matrix 或关线单测。
- `rustfmt-final`：27 个直接／相邻 Rust 文件官方 formatter 检查通过，其中本批实际改动 17 个。
- `ratchet`：4965 个文件、14 个已分类 immutable runtime assets，governed source >700=0，通过。报告已另存证据目录，工作区原报告按直接 preimage 原字节恢复；未变更 policy/baseline/exceptions。
- checker 测试已实际通过 31/31；本批未改变其消费脚本。

新增测试覆盖两个真实公共 HTTP 入口和真实管理 HTTP 批准／取消、原签名 wire、原生 reasoning/工具/费用保真、部分 token 不造零、无效投影 failed FINISH、无 replay/rebuy、旧 claim 保留、Local 失败退款和 permit 释放，以及 policy divergence、序列化 escaping 与累计展开上限。使用合成 peer，不读真实凭据或调用真实平台。

最终来源／范围／编码和资源检查见证据目录 `final-audit.json`：最终 FreeBuff、两生产 lib、formatter、ratchet、diff 六阶段各 4430 项选定输入前后及与终态一致。17 个 Rust（10 新增）及 4 个文档构成本批范围；最大 249 有效行，新文件最大 150。根生成报告及治理文件原字节保护，UTF-8 无 BOM、scoped diff、完整名对账与精确 harness 无活进程均须在该回执中通过，不能只凭测试计数代替来源。

## 改动责任与边界

- [prepare](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/pipeline/stage_send/freebuff_catalog/prepare.rs)：报价前准入并冻结映射后的原生 intent。
- [protocol](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/pipeline/stage_send/freebuff_catalog/protocol.rs)及其按请求／输出拆分的模块：有限公共协议映射和安全观测。
- [paid](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/pipeline/stage_send/freebuff_catalog/paid.rs)、[decoded](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/freebuff/catalog/execution/decoded.rs)：原 run 的 fallible projection → 原 FINISH，不改变原费用 owner。
- [公共 TCP 测试](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/pipeline/stage_send/freebuff_catalog/tests/protocol_json.rs)：复用已有 host/peer，不另起测试服务器框架。

本批无前端改动，不重跑未变前端／浏览器或构建原生包；新协议的正常浏览器联合不宣称已验。旧全目标 `E0560` 不在本批 scope，不报全仓绿。不操作 Redis/Docker、现有实例、真实账号、凭据或数据库；不复用历史 P34 授权。

**未完成：**实时三协议 SSE、未支持模态／参数、跨协议 native reasoning 历史、完整工具／child 与费用、未知 START/FINISH 的显式对账／安全续跑、远端 lease、Server/PG、独立进程／原生包和真实平台验收。下一开发接缝是实际增量流与原费用／取消／FINISH owner 的联合，不把 buffered JSON 拼为 SSE 后称实时。

17 个新增模板、无新增整卡 `offline_ready`；P50 全范围无凭据集成／devtest、P60 集中录制、P70 真实逐渠道验收及总目标仍未完成。本批不缩小总计划，也不要求现在逐项录制凭据。

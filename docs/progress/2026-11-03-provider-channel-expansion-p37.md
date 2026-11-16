# 服务商分渠道扩展：P37 Perplexity Web 单轮底层链

> 2026-11-03；单轮底层实现与45项定向 Rust 测试已通过，完全关闭 feature 的扩大编译另有失败，见第3节。R12/W08、P50 与总目标仍执行中，不按单轮切片标记服务商完成。
> 工作区 `C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`，`main`，HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`。保护全部前序及并行脏改，未提交或推送。

## 1. 本批范围

新增 `src/protocol/perplexity_web/` 的 pinned auto request、完整快照/legacy FINAL、严格有界 SSE owner，以及 `src/upstream/perplexity_web/` 的显式 session 值与单次 HTTP exchange。仅在现有 protocol/upstream 模块表增加最小声明，不修改既有 Sonar/Search preset、Key、URL、协议或调度。

精确支持、预算、错误及未完成接线见[实现合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/provider-perplexity-web.md)。固定消费端 HTTPS origin/path，不携密重定向，禁默认 API headers；仅未分片 session 值，可选 challenge，不接受整个浏览器 profile。Cookie sensitive、不实现 Session Debug；正文、网络地址、会话 token/read_write_token/上下文 ID 不进入错误或返回 metadata。

当前只聚合完整答案，不能宣称实时 token streaming、多轮、工具、附件、Research、任意模型或 diff/JSON Patch 支持。未新增 preset/模板、材料导入/SecretPatch、账号 probe 或公共 dispatcher；不是 P50 UI→提交→运行集成闭环。

## 2. 来源与审查取舍

继续使用[固定参考合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/plan/provider-perplexity-web-contract.md)，本批经公开联网再次读取 helallao pinned `client.py/utils.py/LICENSE`，没有登录、业务请求、执行参考项目或安装依赖。MIT 版权/许可随源码保留；edusu 仅提供 wire 行为线索，不复制其许可文本未完整取得的实现。

两个请求 profile 没有混拼。只实现 helallao 的默认 auto/concise body 和 ask_text/legacy FINAL；参考对 diff 没有读取 op/path/index，因此本批明确拒绝 patch 与 ask_text_0_markdown，不虚构 JSON Patch 请求/成功 fixture。可选来源列表只处理公开 direct web_result_block 形状，不声称两 profile 的实际互通或 citation marker 一一绑定。

只读交叉审查指出并收紧：终态 data 不能绕过内容检查再用旧 partial 成功；来源不能无条件跨答案快照继承。现在 terminal 仅允许空 data/空对象，其他 payload 明确拒绝；来源仅来自最终答案同一事件。新增相应负向/归属测试，`final_sse_message` 单独出现与最后事件无答案继续失败。取消夹具增加首个 server body poll 信号，不只等待请求已抵达；它不冒充客户端已消费特定 token 的观测。

## 3. 验证记录

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-provider-expansion-20261103-p37`。仅合成材料和随机 loopback 端口，不读 `.ng` 或真实账号。

| 检查 | 当前记录 |
| --- | --- |
| 初始红测 v1 | stub 误用不存在的 `GatewayError::internal` 导致编译失败，不能当行为红测；保留日志 |
| 初始红测 v2 | 实际执行 3 项 request/modern snapshot/legacy FINAL 测试，0 passed / 3 failed，确认缺实现；`red-protocol-v2.log` |
| 实现测试 v1 | 测试 helper 缺少 `CanonicalRelayRequest` import，编译失败；已补 import，未修改行为断言 |
| 实现测试 v2 | 22 passed / 0 failed；`perplexity-web-v2.log` |
| 最终 Perplexity 扩大回归 | 32 passed / 0 failed，含上述22项与10项原 API/Search/注册/路由合同；`perplexity-final.log` |
| 共享 SSE decoder | 13 passed / 0 failed；`shared-sse-v1.log` |
| 完全关闭 feature 的扩大编译 | exit 101，15项 Gemini Web 缺失符号，未改该 lane；`feature-disabled-v1.log`，见下 |
| 前序代表性 feature 组合 | `--features line-gemini-web-reverse --lib` exit 0；`feature-representative-v1.log`。不是全矩阵 |
| checker tests / 最终 ratchet | 31/31 / exit 0；`checker-tests-v1.log` / `ratchet-final.log` |
| Rustfmt / diff | 首次只发现新模块声明排序，已调整；最终均 exit 0；`rustfmt-final.log` / `diff-check-final.log` |
| 源码/文本冻结审计 | 13源码+5文档+1许可共19项，UTF-8无BOM/尾空白及hash通过；`audit-final.json` |

本批 **45个不同 Rust 测试通过**（22新增+10原Perplexity+13共享decoder），不累计重复运行；没有 ignored。完全关闭 feature 的编译仍失败，不能声称全矩阵绿色。错误为 `surface::speech`、`WebUpload`、`prepare_media` 等未改动 Gemini Web 符号缺失，位于 `src/upstream/gemini/web_reverse/`。`src/protocol/gemini/mod.rs:35-45` 在两个 Gemini feature 均关闭时选择 disabled surface；本批没有接管 S06 或改其 stub/调用者。没有在移除 P37 源码的旧快照上独立复跑，因此只按实际报错归属记录，不伪称已完整证明历史基线归因。

本批所有新增/改动源码最高282有效行；两个既有模块文件分别91→92、186→187，去掉唯一的 Perplexity 声明后 SHA-256 与修改前完全一致，确认没有撤销其前序/并行接线。源码13项与 `source-frozen-v1.json` 全部一致；未改 baseline/exception，没有新增超限文件。全仓 ratchet 接受既有基线债务，不等于 strict 债务清零。

后端命令固定 `--locked --jobs 4 --no-default-features --features runtime-defaults`，TEMP/TMP/TMPDIR 指向本批 linshi/tmp；`RUST_MIN_STACK=16777216` 仅属于测试进程。不启用 management-ui 发布/删除 hook，不升级依赖或清理缓存。复用既有 Cargo target 缓存，证据与临时目录在 linshi。

## 4. 接力剩余

- R12 的 preset/profile/typed flow/implementation line、正常空池模板、专用材料导入、SecretPatch/CAS、精确账号 probe、公共 dispatcher 和端到端离线测试。
- Cookie/storage state 的范围/安全/过期/分片与错误渠道材料拒绝、适用 session 生命周期；当前仅 token-value constructor，不能冒充完整导入器或 refresh。
- 实际 terminal data、来源分布、模型权限、session/challenge、额度与授权边界仍待集中录制；不立即要求用户逐项登录。
- diff/多轮/Research/附件/模型选择等能力需有独立证据后再开放。当前保守拒绝不能算需求已完成。
- 其他原计划渠道和 Grok 既存剩余项、P50/P60/P70 不变。本批没有新增完整 offline_ready 卡，新增前端模板仍为 P36 的 10 个。

本批没有使用历史凭据、调用真实平台、消费付费额度、创建 Redis/Docker 资源、出包、替换实例、提交或推送。P34 单容器授权已于前序消费并清理，本批未复用。

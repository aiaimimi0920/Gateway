# 服务商分渠道扩展：P39 Perplexity 来源保真前置

> 2026-11-03 开始，2026-11-04 完成本批来源前置验证：225个不同Rust测试通过。R12/W08、P50/P60/P70及总目标仍执行中，不把来源前置算完整渠道。
> 工作区 `C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`；`main`，HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`。保护前序/并行脏改，未提交或推送。

## 1. 本批范围

P37 已能得到最终答案与同 snapshot 来源，P38 已保护材料及目标，但原 canonical response 没有来源字段，Responses 转换器会丢弃未知字段。先完成这个公共保真接缝，不提前开放不能正常执行的模板，也不把来源塞进文本、工具调用、模型或请求 metadata。

- 新增 typed `ResponseSource`、canonical `sources` 和精确 `gateway_sources` 公共扩展。旧 canonical 缺省空、空时省略；Perplexity 复用同一类型并整体移交最终来源。实际模型声明与 usage 仍未知，不将 `turbo` 请求策略冒称后端模型身份。
- 公共 `stage_send::pack_response` 与聚合 Chat SSE 终帧投影来源；Chat→Responses 将其放在最终 `response` 顶层；Responses→Chat、JSON 解包及既有 Responses 聚合器回读后仍保留三字段、顺序和重复项。没有标准 citation offset 的证据，明确不制造 `annotations/url_citation`。
- 解包只读精确扩展名并限制64项及 URL/name/snippet 字节预算，URL 仅 HTTP(S)/host/无userinfo；只复制三字段，不复制 read/write token 等其他字段，不访问 URL。非法已声明扩展明确失败且不重试，不静默清空。
- Chat 来源只接受终帧；终态发出前重复来源、非终帧来源以及来源冻结后的新增正文/工具增量会中止。允许同一终帧的最后 delta 与后续 usage-only 帧。公共终态后保留原 drain 行为，不承诺再解析/验证尾部材料；原生 passthrough 不重打包。
- 修正共享 usage 读取将 `usage:null` / 非 object 误造为零值的问题；合法 object 的旧字段、别名及空 object 默认语义保留。
- 为共享 Rust 类型补齐旧 literal 的空来源字段；Gemini 必要接口接线不改其执行逻辑，不接管 S06，也不修旧全 feature-off 问题。

具体 wire 合同见[来源扩展](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/provider-response-sources.md)。这里的流是**完整答案聚合后输出**，不是 Perplexity 实时 token streaming。

## 2. 验证记录

证据：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-provider-expansion-20261103-p39`，只使用合成材料和既有 loopback 测试，不需要 Redis 或真实凭据。

| 检查 | 当前结果 |
| --- | --- |
| 第一轮红测编译 | `red-sources-v1.log` 因新测试调用错 accumulator 入口而编译失败；修正测试调用，不修改生产语义，不计作行为红测 |
| 实际行为红测 | `red-sources-v2.log`：8项，1 passed / 7 failed；实测来源丢失及非法来源被忽略 |
| 第一版绿测 | `green-sources-v1.log`：16 passed；只证明第一版，不冒充之后冻结答案边界的最终源码 |
| 最终来源定向 | `green-sources-v2.log`：18 passed，覆盖最终源码及来源冻结、合法usage尾帧、JSON/SSE双向/聚合、字节预算、空值与旧格式等合同 |
| Responses / OpenAI / stage_send | 57 / 63 / 46 passed；`regression-responses.log`、`regression-openai.log`、`regression-stage-send.log` |
| canonical SSE / Perplexity协议 / transport / canonical类型 | 4 / 12 / 10 / 19 passed；对应 `regression-*.log`，transport使用合成loopback，不是真实账号 |
| 代表性feature编译 | `cargo check --locked --jobs 4 --no-default-features --features line-gemini-web-reverse --lib` exit0；`feature-representative.log`，不是全feature矩阵 |
| checker | 31/31，`checker-tests.log`；checker 本身未改，不重复累计 |
| ratchet | 最终代码 `ratchet-v2.log` exit0，未写原有脏 JSON报告、未改baseline或exception |
| 源码/文本审计 | 最终34源码+4文档的UTF-8无BOM、尾空白、scoped Rustfmt与hash检查见 `audit-final.json`；`source-frozen-v2.json` 保留测试源码冻结，`source-final.json` 包含最终文档；独立diff检查见 `scoped-diff-checks.json` |

`test-summary.json`按lib target与完整测试名称去重，最终为**225个不同Rust测试通过**。18+57+63+46+4+12+10+19为229次通过，其中4项重复，不能直接相加，也不累计第一版16项或红测中的通过项。前端没有改动，未无故重复P38前端测试；没有运行全仓、真实上游或发布门禁。

34项源码最终均不超过476有效行：canonical474→476、前向translator431→463、反向351→357、pack66→68、canonical SSE112→113、Perplexity入口39→48；新source owner61、新JSON合同测试106、新stream合同测试296。其余精确前后值见冻结与审计，不改baseline/exception。19个文件的24处空字段接线之外逐字节不变；另有原SSE测试文件的2处空字段和新增测试。Gemini五文件七处均在上述纯机械集合内，未改S06执行逻辑。JSON两个unpack入口另接严格来源读取，不混称为空值接线。

只读复核发现反向 SSE 原本吞掉 unpack 错误，以及来源终帧后等待 usage 的窗口仍能追加答案，两处都已补生产保护和专门回归。静态审查不是运行证明。批量机械替换曾超时并发生部分写入，随后逐文件/冻结hash确认最终状态，没有假定全成功、盲目重放或回滚其他改动。

首次独立diff审计将既有CRLF文件的Git转换warning误记为空白错误，`audit-v1.json` / `scoped-diff-checks-v1.json`仍保留。后续只对该检查命令设置`-c core.safecrlf=false`关闭转换warning，继续保留`--check`及独立尾空白检查；没有改Git配置、attributes、原文件换行或跳过真实whitespace错误。Gemini文件的机械字节证明见 `mechanical-source-fields.json`。

后端统一 `--locked --jobs 4 --no-default-features --features runtime-defaults`，TEMP/TMP/TMPDIR 指向本批 linshi，测试进程 `RUST_MIN_STACK=16777216`。不启用 management-ui 的发布/删除 hook，不升级依赖或清缓存。

## 3. 下一步与未完成边界

1. 接 Perplexity 专用 dispatcher，正确区分 caller alias 与选路后 `turbo`；在网络前拒绝未支持参数/endpoint，不能删除 P38 gate 后落入 generic raw-body fallback。
2. 专用 dispatcher 可执行后再注册 preset/profile/line/feature/前端 selector，并开放正常空池模板。
3. 将 P38 专用 parser 接入统一材料入口、普通 apply/暂存后 UA 与 expiry 绑定、SecretPatch/grant/原 revision/CAS 的真实正常保存；不再另造保存协议。
4. 接精确账号 probe 和完整 auth/filter/route/send/finalize 的 Chat/Responses buffered/适用聚合SSE、取消与审计；当前 source 单元链不代表普通 HTTP 已接通。
5. 继续其余原定渠道与 P50/P60/P70，最后再集中录制和真实验收。当前新增模板仍10个，没有新增完整 offline_ready 平台卡，R12/W08 与总目标未完成。

没有读取或试用历史凭据、真实上游调用、付费生成、创建Redis/Docker资源、出包、替换在线实例、提交或推送。P34单容器许可已在前序消费并清理，本批没有复用。

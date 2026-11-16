# FreeBuff：原 claim、catalog run 与 completion 响应生命周期绑定

日期：2026-11-07。工作区 `C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`，`main` / HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`；包括继承的未提交改动，HEAD 不作为本批实现交付凭据。

## 1. 本批边界

在[持久 claim 子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-07-provider-channel-expansion-freebuff-claim-recovery.md)之后，新增内部 `ActiveAdmission → CatalogRun → 冻结 completion → 响应 body` 子链。它借用原持久 owner，绑定当前目录的 stable key/handle/fetchId、真实 START 返回的 run ID、host client ID 与原 instance；不接旧 model→agent map，不重新购买。

**这仍不是正常公共 FreeBuff 推理或完整 agent harness。** 没有开放消费接口或公共入口，目录模式仍在网络前返回 `409/freebuff_business_admission_required`；没有通过注入产品身份/system prompt marker 来冒充完整官方 root。调用者仍须提供真正 harness 的最终 Chat body，当前代码不构造 row compaction、工具运行与完整 prompt。费用确认、Grant/revision 重验、完整 decoder/计费和 FINISH 等继续保留。

## 2. 实现和已知限制

- [Local 生命周期桥接](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/local_runtime/freebuff_sessions/lifecycle.rs)对 `ActiveAdmission` 作独占借用；run、冻结请求、HTTP response 和流式 body 均携带该借用。原 kernel guard 不能在 body 尚被持有时释放，也不能同时从同一 owner 再开始一条借用执行。
- [run/context owner](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/freebuff/catalog/execution.rs)在 START 前、START 回来后、冻结前和实际 completion send 前核对 scope、原 stable model、CLI 和单调 TTL。观察过期不会获得新授权；START 成功但观察已失效时也不放行 completion。它不通过后台 heartbeat 延长观察，不保证取消已经在服务器执行的操作。
- [独立 START wire](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/freebuff/catalog/run_wire.rs)固定 `POST /api/v1/agent-runs`，body 为 `START / base3-free-catalog / ancestorRunIds=[]`，使用同一账号和 origin 的 Bearer。不猜调用者指定的 path/agent，不发送 server-only acting-user header，不给 run API 伪加 session/device 签名。
- START 无 retry/redirect，header+body 共用最多20秒预算，解码正文累计准入64 KiB（不等于底层读取/heap/wire的严格上限）；非2xx、坏 JSON、缺失/畸形 runId 不生成假 ID、不视作成功。错误和 Debug 不输出上游 body、token、claim、run ID 或 prompt。
- SDK 的 START 使用 `JSON.stringify` 字符串 body。本机 Node `v22.22.2` 无网络构造 `Request` 实测其自动 Content-Type 为 `text/plain;charset=UTF-8`，见证据根 `fetch-request.json`；Rust 显式保持该字段，未把“源码没有显式设置 Content-Type”误读为实际 Fetch wire 没有该字段。这不是线上服务要求的验证。
- completion 消费本地 run capability；最终 body 的 model 必须是当前所选 handle。host 覆盖保留的 runtime metadata，原 instance、`freebuff_multi_session="1"`、`surface="cli"`、真实 run ID、host client ID 与 `cost_mode="free"` 一起冻结，再走现有有界序列化/精确字节签名。保留其他普通 metadata，不接受字符串等畸形 metadata。
- [共用 metadata 组装](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/freebuff/request.rs)同时补齐旧 helper 在 CLI claim 下遗漏的 multi-session/surface；legacy 非 CLI instance 不被升级为 multi-session。
- 响应仍是 transport 结果，200 不等于完整推理成功。body wrapper 不把原始 response 单独移交，不额外无界收集；真实 decoder 仍须自行执行累计预算、JSON/SSE 解析、费用和错误安全投影。completion 原有最多300秒 request timeout 继续覆盖 body；本批没有把 START 与 completion 合并成一个跨阶段总 deadline。
- Drop/取消只丢弃当前 future/response，不自动 START 重试、补 completion、FINISH、end/refund 或删除 journal。原 claim 继续 outstanding。**本批没有持久 run ledger、FINISH、远端 lease 收尾或跨请求 exactly-once 保证**；不能将“没有自动重投”理解为未知 run 已成功恢复或结算。

## 3. 新补齐的公开合同

仅匿名 GET `CodebuffAI/freebuff@6f567ec21542afc07dbe447bce0dcaf7a5a73a8d` 的公开原文；不是当前线上服务的实测。

- [database.ts](https://github.com/CodebuffAI/freebuff/blob/6f567ec21542afc07dbe447bce0dcaf7a5a73a8d/sdk/src/impl/database.ts#L408-L519)：START/FINISH 共用 agent-runs，但没有显式 device 签名；START 返回 `runId`，FINISH 的 credits 来自调用参数，不是 transport 写死0。Gateway 没有照搬该 SDK 的 POST 重试或吞错日志行为。
- [catalog root](https://github.com/CodebuffAI/freebuff/blob/6f567ec21542afc07dbe447bce0dcaf7a5a73a8d/cli/src/utils/freebuff-catalog-agent.ts)：root 每次按当前 row 重建，model 是 handle；仅发送 root ID 不等价于完整实现 root definition。
- [session identity](https://github.com/CodebuffAI/freebuff/blob/6f567ec21542afc07dbe447bce0dcaf7a5a73a8d/cli/src/utils/freebuff-session-identity.ts#L22-L36)、[cost mode](https://github.com/CodebuffAI/freebuff/blob/6f567ec21542afc07dbe447bce0dcaf7a5a73a8d/cli/src/utils/constants.ts#L188-L202)与[llm metadata/费用转换](https://github.com/CodebuffAI/freebuff/blob/6f567ec21542afc07dbe447bce0dcaf7a5a73a8d/sdk/src/impl/llm.ts)：CLI 原 instance/multi-session/surface、FreeBuff free cost mode、reserved identifiers 的覆盖次序；usage cost 是 dollar 输入，不能不经累计/转换就当 FINISH credits。

七份原文包括 LICENSE，实际 URL/UTC/HTTP/SHA-256 保存于证据根 `C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-catalog-run-20261107/sources.json`；`database.ts` SHA-256 为 `1e53339043cd41e0acb6f8ffc31d696493e8702084ca4608fcf7495c4338281a`。仅参考 wire/type 独立实现，没有执行或复制第三方 harness。公开客户端不能证明 server 对 run/claim 的强绑定、价格 ceiling、当前服务 schema 或实际计费。

## 4. 验证回执

证据根为 `C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-catalog-run-20261107`，全部临时 runtime 数据置于该目录。后端使用 `--locked --offline --no-default-features -j 4`；正常测试/生产检查开启 `runtime-defaults`，不触发 `management-ui` 发布/清理 hook。测试局部使用16 MiB最小线程栈，没有修改生产栈或依赖。

| 实际检查 | 结果 | 日志 |
| --- | --- | --- |
| 改生产代码前的 CLI metadata 红测 | 编译后实际断言失败，`left: Null / right: "1"` | `red.log` |
| 首轮 `freebuff` 名称筛选 | 267 passed / 1 ignored | `green.log` |
| 补齐 Fetch Content-Type 与全前缀 header 断言后的终态 `freebuff` 筛选 | 267 passed / 1 ignored | `green-final.log` |
| 终态 `local_runtime::` 名称筛选 | 49 passed | `local-runtime.log` |
| 正常生产 `cargo check --lib` | exit 0 | `production.log` |
| 从正常 feature 集只关闭 FreeBuff 的代表性生产 `cargo check --lib` | exit 0，不是全 feature 矩阵或关闭线运行测试 | `production-off.log` |
| `cargo fmt --all -- --check` | exit 0 | `format.log` |
| 官方 checker tests | 31/31 passed，checker 的源码/测试/配置后续未变 | `checker.log` |
| 官方 effective-line ratchet | 4768 files，14 immutable 单列，governed source >700=0 | `ratchet.log` |

终态两个 Rust 筛选按完整测试名去重为 **307 passed / 1 ignored**，不重复累加首轮与终态。9个新增命名测试为8项 run/context 合同与1项 metadata 红绿合同。既有 ignored 是需要专用目录/浏览器的 UI 夹具，没有算成通过。实际测试覆盖原 claim 与新 handle/fetchId、真实 START 返回 ID、签名覆盖最终 metadata/body、另一 LocalRuntime 在 body 持有期仍 busy、scope/model/过期拒绝、坏 START/redirect/超大响应不重试、START header/body 共享预算、冻结后观察到期零 completion、body 总 timeout 和取消 START 后 journal 保留。均为合成回环 HTTP/真实 SQLite，不是正常公共调用、完整 agent、有效推理或远端费用/取消的证明。

9个受审源码中3个新增，最大416有效行；主要前后值 `request.rs` 198→230、`catalog.rs` 187→190、`business.rs` 261→265、Local生命周期53→60。未改 baseline/exception。独立只读审查未发现这条受限子链的具体身份、资源或副作用缺陷，不把静态审查当运行验收。

终态6个适用阶段各4750项所选输入均无阶段内漂移，checker 的实际 owner 输入另外核对未变；不是全仓所有动态输入、工具链或包的冻结。完整来源、严格 UTF-8/无 BOM/尾随空白、本地链接、范围 diff、公共 guard 原 SHA-256 未变与根 effective-line 报告直接 preimage 恢复见 `audit.json`，本轮改动范围见 `scoped.diff`。编译 warnings 保留，包括尚未进入正常调用的内部 owner；没有宣称全仓、正式发布门禁或真实服务端验收全绿。

## 5. 接力入口

先闭合逐次消费确认、即时价格/余额/Grant/revision 与 exact account execution context，再接完整 root/harness、持久 run/lease、JSON/SSE decoder、真实费用累计/FINISH、精确终态/closeout/probe，最后开放正常入口；不能先移除目录 guard。旧 FINISH 的写死0和 legacy retry 不进入新 owner。

模板仍累计新增16，无新增整卡 `offline_ready`；R06/W04、其余渠道和 P50/P60/P70 不因本子批完成。没有读取真实凭据、真实设备注册/会话购买/模型调用、现有实例变更、数据库迁移、容器创建、出包、提交或推送。历史 P34 已通过且容器已清理；本轮不重复创建，也不复用其单容器授权。

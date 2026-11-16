# P49 后续：Kiro Key 公共调用链与输入/终态完整性

日期：2026-11-05。工作区：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`。
状态：主运行配置定向回归、代表性关闭编译和最终来源/静态核对通过；不是总计划 P50 或 R02/W02 整卡完成。
分支：`main`；HEAD：`7969f12c85efceacf2a5e384e143b5c39a550981`。保留既有 dirty edits，不提交或推送。

## 1. 改了哪些内容

- 用真实 `build_router`、认证、normalizer、preset/route、精确 Key、model map、普通 HTTP sender、协议编码和 SQLite finalize
  覆盖 Chat/Messages/Responses 的 JSON/SSE。不是直接调用 packer 代替公共入口。
- Key sender 对传输错误、非成功 HTTP 和非 EventStream 的 200 使用不可重试的 Abort；不读取/返回/持久化 HTTP 错误正文，
  保留类型/状态及显式 Retry-After，禁止换账号/换渠道和重定向。上游任意错误正文/code 不进入公共响应或审计。
- Key 的帧/CRC/provider 错误以固定诊断的 domain error 传播，避免 Responses 桥忽略 Chat error object 后在 EOF 合成 completed。
  部分输出后的错误仍是失败，不发布成功 usage；caller drop 沿原 tracked-stream owner 取消与释放连接，不增加 detached reader。
- Key buffered/SSE 共用原有有界 EventStream parser 的严格模式。已知事件的坏 JSON/字段类型、空输出、无 identity 工具、
  同 ID 改名/关闭后追加、未 stop 的工具及 stop 时非 object JSON 参数被拒绝；未知元数据事件沿原语义忽略，不算输出。
- Key 工具完整性状态最多 4096 项、64 MiB retained 字节估算；参数按片追加，仅在 stop 解析一次并释放参数存储，EOF 校验在 reset 前。
  这不是整个请求的总堆内存上限；原 frame、accumulator、wire encoder 各有自己的留存边界。
- Key packing 前明确拒绝尾部 Assistant、不可解析的历史参数、缺失/重复/不配对工具 ID、未解决的历史调用、remote/大写 `DATA:`
  或其他不支持的内容、服务端 `previous_response_id` 续接。历史工具需在当前 tools 显式声明，不再默默补 placeholder 或丢弃内容。
  这是首批产品限制；旧 OAuth 的 packing 修复和宽松 EOF 合同保留。
- 公共工具/图片用例暴露 Messages normalizer 只读 `source.url` 而丢弃合法 base64 图片；在该边界最窄修复，构造标准 data URI，
  缺少 media_type/data 明确 400，原 URL 分支保留。Key 下游仍只接受受支持的 data image，不新增远程下载器。
- 后续用例再暴露 Responses `input_image` 始终为 Raw，导致严格 Key 拒绝标准图片；只将标准 type/image_url/string detail 转成 ImageUrl。
  file_id、扩展或错误类型 detail 保留 Raw，不把信息先丢掉再放过 Key。Key 图片仅接受缺省/auto detail，high/low 等未证 CLI 控制明确拒绝。

主要 owner：
[Key sender](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/kiro_api_key.rs)、
[帧 payload](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/kiro/event_payload.rs)、
[Key response](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/kiro/key_response.rs)、
[Key request](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/kiro/key_request.rs)。

## 2. 验证与过程保留

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-kiro-key-public-pipeline-20261105`。
Cargo：`--locked --jobs 4 --no-default-features --features runtime-defaults`；测试栈 16 MiB，TEMP/TMP 在本批目录。
未启默认 management-ui 发布/清理 hook，不升级或重装依赖。

| 检查 | 实际结果 |
| --- | --- |
| 首次行为红测 | `red.log`：3 passed / 4 failed；复现空流成功、HTTP 错误换候选、Responses 帧错误后假完成；健康断言另有异步顺序竞争 |
| 第一版公共组 | `green-v1.log`：7 passed / 1 failed；上述错误路径修复通过，新增工具/图片用例暴露 Messages base64 图片丢失 |
| 完整性红测 | `red-integrity.log`：7 passed / 3 failed，来源未变；复现合法 CRC 的坏 JSON 假成功、丢尾 Assistant，保留图片失败 |
| v2 公共组 | `green-v2.log`：9 passed / 1 failed；严格负例通过，工具/图片用例的 400 暴露 Responses 图片缺少 typed normalization |
| v3 编译 | `green-v3.log`：测试对未实现 PartialEq 的 ContentPart 使用 assert_eq 触发 E0369；仅改 matches，不给生产类型增加比较职责 |
| v4 公共组 | `green-v4.log`：10 passed / 0 failed / 0 ignored，冻结来源未变；三入口与 JSON/SSE 在测试函数内逐格覆盖，不将循环格数当新增测试数 |
| Kiro 协议及 sender | `kiro-protocol.log`：61 passed；`kiro-senders.log`：20 passed，其中包含上述 10 项公共组，不重复累计 |
| Anthropic 首轮回归 | `anthropic-protocol.log`：57 passed / 1 failed；唯一失败是旧测试明确要求 base64 图片归一化为空 URL，锁定了已复现的丢图行为 |
| Anthropic 修正后回归 | 仅将旧断言改为正确 data URI 并改名；`anthropic-protocol-fixed.log`：58 passed / 0 failed / 0 ignored，24 项冻结来源未变 |
| Responses 与流错误 | `responses-protocol.log`：57 passed；`stream-errors.log`：4 passed，均无失败/忽略、来源未变 |
| 代表性 Kiro feature-off | `kiro-feature-off-exit.json`：exit 0、来源未变；展开 runtime-defaults 后仅去除 `line-kiro-official-vendor-api`，执行 locked `cargo check --lib --jobs 4`，不是关闭配置的运行测试 |
| 静态门禁 | `static-final-exits.json`：Rustfmt、ratchet、diff 均 exit 0；checker 实现/配置/输入未变，复用 v4 已通过的 31 项，不无故重跑 |

以上六组按完整 Rust 测试名去重为 **200 passed / 0 failed / 0 ignored**，包含本批 **16 个新增测试函数**；
旧图片断言的修正不是第 17 个新增测试。旧轮次、不同配置、重复运行和循环内场景均不叠加。
测试命令为 `cargo test --locked --jobs 4 --no-default-features --features runtime-defaults --lib <filter> -- --nocapture`；
filters 分别为 `kiro_api_key_pipeline_`、`protocol::kiro::`、`upstream::client::kiro_`、`protocol::anthropic::`、
`protocol::responses::` 与 `protocol::stream_error::`。本组合未启用 management-ui，不是完整默认配置/全仓回归。

健康断言的修正是在 audit 完成后有界等待模型健康写入，不改变生产 finalize 顺序。合成 SQLite 留存显示正确 exact/model/active 行，
与源码中 stream audit 先于健康更新的顺序一致。原测试失败保留，不算业务缺陷被隐藏。
独立审查确认第一版仍有语义坏帧和历史丢失缺口，草稿审查又发现大小写 data scheme 穿透；均进入 v2，未放宽 OAuth 旧合同来测绿。
源码审计辅助脚本首次使用 Windows 绝对 ESM import 触发 `ERR_UNSUPPORTED_ESM_URL_SCHEME`；修正为 file URL 后重新核对，原失败日志保留。
末轮未跟踪文件 whitespace 辅助检查初版误将 `git diff --no-index --check` 的 exit 1（有差异、无诊断）当失败；
仅修正辅助检查对原始退出码的解释，保留首轮 JSON，不修改源码或 Git 设置来消除差异。
文档链接复核另发现原计划 §10.3 的两份历史日志当前缺失；保留原路径并明确标注，不创建替代日志或声称旧证据仍可复验。
失败的 `closeout.log` 与缺失清单保留，最终结果使用 `closeout-final.log` / `closeout.json`。

## 3. 来源、有效行与安全/资源复核

原 v4 的 23 项 Rust 输入 SHA 未变；仅追加旧图片测试文件的写前快照与修正后来源，最终冻结 24 项。
源码均 UTF-8 无 BOM，最大 471 effective lines，未新增 501 行以上文件或例外。部分写前/写后值如下；完整记录为证据目录的
`final-before-after-lines.json`，没有写前快照的新增 owner 不伪造历史计数。
最终 `closeout.json` 汇总逐 phase 输入 SHA、测试去重、feature-off、静态门禁及四份文档的 UTF-8/本地引用核对。

| Owner | 写前 → 写后 effective lines |
| --- | --- |
| `src/upstream/client.rs` | 469 → 471 |
| `src/upstream/kiro_api_key.rs` | 144 → 181 |
| `src/protocol/kiro/event_stream.rs` | 472 → 417 |
| `src/protocol/kiro/event_stream_translate.rs` | 99 → 186 |
| `src/protocol/anthropic/normalize.rs` | 391 → 406 |
| `src/protocol/responses/normalize.rs` | 248 → 267 |
| `src/protocol/anthropic/normalize_tests.rs` | 171 → 172 |

`stage_send.rs`、`stage_send/feedback.rs` 和 `kiro/stream_state.rs` 与本批写前快照逐字节相同，不改变共享 finalize/健康/取消 owner。
最终 ratchet 扫描 4358 文件，14 项 immutable runtime assets 单独计，`governed source >700=0`。
本批没有修改 checker、baseline、exceptions 或 immutable policy 来测绿。

安全/资源复核保留在证据目录的 `review.md`：固定受众与 Key owner、不读取任意 HTTP 错误正文、不泄漏 provider error；
Key 完整性状态有界、参数只在 stop 解析一次并释放、声明名称 HashSet 查询，不新增 detached reader、后台刷新或隐藏重试。
复用原连接/stream/SQLite 生命周期；这是源码与合成运行证据，不是总堆内存硬上限或真实网络中断验收。

## 4. 产品语义与未验证边界

1. 公共响应保留 caller-visible `public-alias`；wire/resolved audit 使用显式 `claude-sonnet-contract`，凭据固定 `exact`。
   不因模型名映射为旧 IDE 默认模型。已有备用账号/渠道用于验证没有隐藏 fallback。
2. `max_tokens/max_completion_tokens` 沿既有输入兼容，但不映射成未证的 CLI 控制，不保证上游输出预算。
   Responses 的 max_output_tokens/temperature、reasoning、强制工具选择等当前明确拒绝，不删参数测通。
3. usage 仍是旧 Kiro decoder 的 token 估算，不是 subscription credits、真实 metering 或供应商扣费。
   工具/图片测试不证明真实模型拥有这些权益；主图片夹具是本地生成的 1×1 PNG，不做真实上游视觉验收。
4. continuation 的字段省略、identity、stop 与参数约束是已实现的合成本地合同，不是本轮联网或抓包核实的真实 CLI 完整 wire。
   首次缺 name、同 ID 改名、关闭后重用被拒绝；不外推全部 Region、模型、扩展或错误语义。
5. 本批不是正常 UI→真实管理服务→保存→公共调用/OS 重启，也未执行真实 PG→object→upsert/cache 同步、旧 route/DB 升级、
   profile/subscription 发现、跨应用 receipt 恢复或真实资格/模型/额度验收。下一步继续正常管理服务与重启闭环。

累计新增模板仍 13，不增加整卡 offline_ready；Vertex/Luma/Grok、其他 Web/媒体、Qwen OAuth 受限待决与 P50/P60/P70 继续。
本批不读取 `.ng`/历史凭据、不登录或真实推理、不创建 Redis 容器、不出包、不改在线实例、不提交/推送，不删除 linshi 外内容。

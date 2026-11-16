# 服务商分渠道扩展：第 16 批，普通 Qwen 精确账号测试的正式发送边界

> 2026-11-02；R09 / W05 / P01 / P10 的无凭据切片。精确发送接线及定向离线验证完成；不是 W05 或总目标完成，精简 feature 失败与取消 audit 缺口仍明列。
> `main`，HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`；保护 P01–P15 脏改，未提交、推送、打包或重启实例。

## 1. 产品与本批范围

仍是普通 `qwen-api` / `qwen-official-api` workspace API Key 产品，不是 Coding Plan、Qwen Web 或 OAuth。
API Key / OAuth / Web 各自评估，Web 不是兜底；额度共享没有证据时继续隔离、标记 `unknown`。
本批没有新增产品事实、外部源码复制或依赖。公开产品合同沿已联网冻结的 [API 合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/plan/provider-api-contracts.md)。没有新联网调查、历史凭据读取或真实上游调用。

只处理 P15 留下的普通产品精确 probe 接线与无请求策略，不把它扩大解释为完整推理 pipeline、OAuth 生命周期或实际额度验收。

## 2. 实现边界

- `console_probe.rs` 对普通 workspace 产品在通用 HTTP probe 之前返回 `unsupported`，明确要求显式模型/prompt 或已保存测试计划。没有隐式收费生成，也不再冒发 compatible `/models`。probePoint 同时改为准确的显式测试提示。
- `console_model_probe.rs` 保留网络封锁、disabled、精确 credential、模型声明与映射检查，在通用模型发送之前分派到新 `qwen_model_probe.rs`。不重新选号、选池或借用同品牌其他渠道。
- 新 owner 构造正常 Chat canonical 输入，映射后的模型、`stream=false`、`max_tokens=256`、`n=1`；正式 `call_flow::prepare` 后，同一 prepared payload 贯穿 `UpstreamClient::build_request_plan`、`build_inference_headers` 和 `send_profiled_plan.send()`。
- workspace/feature/Key/路径/认证保护、指纹 plan 后处理及禁止重定向由正式 owner 保留；没有复制另一套 Qwen planner、签名器或 sender。planner/prepare 拒绝时不发送。
- probe 专用响应验收有完整读取 64 KiB 上限和 60 秒总 timeout；只接受 `finish_reason=stop`、assistant role、非空文本且无工具调用/refusal/error envelope 的完成回复。坏 JSON/UTF-8、半截响应、空/未完成/截断内容不能 Passed。
- 错误只使用固定分类及 HTTP status，不返回原始上游 body/transport error。成功文本先按 prepared Key/auth/header 值脱敏，再 sanitize，答案上限 8192 字符，消息预览 320 字符。
- 原 `model_probe_recording` 继续负责精确 provider/credential/model 的 audit、usage 和模型健康 CAS；single prompt、temporary/saved/named plan 共用同一分派，没有新增平行计划执行器。

这里复用了正式产品 planner/header/sender，但响应是 probe 自己的有界 Chat JSON 验收；没有声称复用了完整 `execute_*` 解码或外部五阶段推理管线。

## 3. 测试 transport 与安全

仅 `cfg(test)` 的 task-local seam 在正式 prepare/planner/Key 验证成功后改写 plan 的 transport URL。payload 保留合法 workspace；生产没有可通过 HTTP、配置或环境变量启用的改写入口，也未为测试放宽产品 validator。
随机 loopback 上运行实际 HTTP server，显式代理同样只指向本地；非 POST（含 HTTPS CONNECT）直接拒绝，绝不转发。测试固定 managed transport=false，不绕到独立 wait client。
因此旧实现或 seam 回归也只能接触本地非转发代理，不会用合成 Key 访问真实 origin。测试证明的是合法产品配置、正式发送 owner 与本地实际 HTTP，不是官方 DNS/TLS 或真实 Qwen 服务。

临时目录和 TEMP/TMP/TMPDIR 均为 [P16 证据目录](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-provider-expansion-20261102-p16) 下的新路径。合成 local runtime 使用独立 SQLite，关闭后保留在该目录；未读取真实 `.ng`、数据库或凭据。
共享 Gateway Cargo 串行、`--locked --jobs 4 --no-default-features --features runtime-defaults`，不启用 `management-ui`，不触发工作区 live Web 发布/清理。编译等待继续原 handle，没有重复启动同一 Cargo。

## 4. 实际验证

### 4.1 红测

`red-v1.log`：2 fail。无请求用例实际得到 `failed` 而非预期 `unsupported`，复现旧通用 connectivity 发送行为；本地 trap 不转发真实上游。
空 Key 用例首轮把 provider response 的 `result.results` 错读为顶层 `results`，在 `Null` 比较处失败；这是测试 selector 缺陷，已修正，不计为有效的空 Key 行为红测。
首轮实际编译约 10 分钟，旧空 Key 通用请求还等待了其 60 秒 builder deadline；不把 test client 的 200 ms 默认当成生产 deadline 已生效证据。

### 4.2 绿测与门禁

`green-v1.log` 为 **5 pass / 1 fail**。失败来自测试把通用字符串 `synthetic` 作为秘密特征，而正常模型名正是 `qwen-synthetic`；已改为检查具体合成 Key/Cookie/非法 Key，不修改生产成功或拒绝条件来凑绿。
同次运行已通过真实管理的 single/temporary/saved/named plan、精确 wire/指纹/usage/audit/健康归属、空 Key 不触网、无 request 不触网，以及 19 类 HTTP/回复结果。随后新增独立的管理 future drop / TCP 关闭 / 不标成功用例，最终数量以重跑为准。

`green-v2.log` 为 **7 pass**，包含新增的真实管理 future drop：本地 TCP 确认未收完响应的连接关闭，audit 和模型健康没有误标成功；这不证明取消 audit 被 finalize。
最终相邻回归 `runtime-regression.log` 为 **26 pass**（包含上述 7 项，不重复累加），`sender-regression-v2.log` 为 **3 pass**，`integration-regression.log` 为 **18 pass**（Bedrock probe 7、NVIDIA probe/清理 5、Qwen workspace 运行合同 6）；合计 **47 pass**。不是全仓回归。

首次 sender 过滤器遗漏 `native_codex` 模块层级，`sender-regression.log` 实际执行 **0 tests**，不算通过；已按真实模块路径重跑并取得上面的 3 pass。
`core-only-check.log` 的 `--no-default-features --features line-qwen-official-core --lib` **失败**，15 项 E0425/E0432/E0433 位于本批未改动的 Gemini Web speech/transcription/uploads/execution owner。没有做 clean HEAD 对照，不将它冒称已复现的 clean baseline；也未越过 S06 保留 lane 去改 Gemini。
`api-off-runtime-contract.log` 使用代表性组合 `--no-default-features --features line-qwen-official-core,family-openai-compatible-official-api,line-gemini-web-reverse`，**另 5 pass**，验证 API-off/core-on/family-on 时普通 Qwen 及 Coding Plan planner 均明确 compiled-out、不回落 generic OpenAI。新生产 probe 模块也在该组合中编译，但这 5 项不是 API-off 的完整管理 probe 测试，不能把它与失败的 core-only 混为一次全绿矩阵。
可重跑命令保存在证据目录的 `run-regression.ps1` 与 `run-feature-off.ps1`；前者保留了首次无效 sender 过滤器和失败的 core-only，不伪造全部成功的历史脚本。

`checker-final.log` **31/31**；`ratchet-final.log` 扫描 **3992 文件、14 immutable、governed source >700=0**；官方 Rustfmt 与全工作树 `git diff --check` 通过。ratchet 使用相同 checker 的无报告写入入口，未覆盖既有脏的有效行报告。

## 5. 有效行、来源和兼容

本批 8 个源码均不超过 500，有 5 个新增源码均在 115–205 有效行内；没有新增 baseline、例外、锁文件、storage version 或数据库迁移。

| 文件（`src/provider_runtime/`，首行除外） | 前 → 后有效行 |
| --- | ---: |
| `src/provider_runtime.rs` | 77 → 80 |
| `console_model_probe.rs` | 356 → 359 |
| `console_probe.rs` | 144 → 152 |
| 新 `qwen_model_probe.rs` | 203 |
| 新 `qwen_model_probe_tests.rs` | 115 |
| 新 `qwen_model_probe_test_support.rs` | 153 |
| 新 `qwen_model_probe_management_tests.rs` | 181 |
| 新 `qwen_model_probe_response_tests.rs` | 205 |

`source-before.json` 记录三个既有 owner 的当前脏树起点，新增文件按 0 计；`source-frozen-v1.json` 记录最终源码。P11/P12/P14/P15 各自最后 frozen source 复核全部匹配；P13 已被 P14 有意修改的 owner 不伪称仍等于 P13 初始 hash。
最终 8 个源码、4 份本批文档严格 UTF-8 无 BOM、无尾随空白；Markdown 围栏和本地引用检查通过。5 个新源码及 4 份未跟踪文档另行执行 `git diff --no-index --check`，没有错误；未把普通 `git diff` 能自动检查未跟踪文件作为假设。
没有改动模板身份、Key 保存/keep/replace/clear、父级目标、目录 ownership 或既有数据库。正式发送仅替换普通 Qwen 的 probe 分支，其他产品继续走原 owner；已有目录不能被作为生成证据。

## 6. 已知边界与未完成项

1. 本批未验证真实 Qwen Key、官方目录、生成、额度、登录/刷新、实际浏览器、EXE 包或隔离 EXE 启动，不新增蓝色验收标记。
2. 测试的 slow HTTP 用例使用 fixture client 的 1 秒 timeout；不能冒称直接实测了生产 60 秒总截止线。
3. **共享 recording 的外层取消 audit 收尾仍有缺口。** `Recording` 没有 Drop guard；在 begin 之后直接 drop 管理 probe future，不能保证 finish 被执行，可能留下未完成 audit。内部 timeout 会正常走 finish，但这不等于外层取消闭合。本批未扩写公共 recording owner，也不以 socket 停止代替 audit 完成。
4. P13 极端 expiry 已知用例、本批未涉及的 P15 既有前端四项失败仍按原回执保留，没有修改无关测试来凑绿。
5. P13 曾触发默认 `management-ui` live 发布/清理的副作用与授权遗漏仍保留；本批不抹除历史，也不授权后续目录清理。
6. 精简 core-only feature 编译仍失败，见第 4.2 节；本批没有声称全 feature 矩阵通过。所有本批 Cargo/exec sessions 已实际结束，收尾不把观察超时当作命令完成。

## 7. 接力入口

下一最小任务仍是普通 Qwen 完整 pipeline 的 buffered/stream、响应终态、usage/结算、失败传播和取消闭环；同时确定并修复精确 probe 外层取消的 audit owner。不要把本批 probe 成功外推为 W05 `offline_ready`。
首个建议用例是 [Responses route](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/http/routes/responses.rs) 的真实非流式请求，经 `run_pipeline` 发到普通 Qwen Chat，返回 Responses 形状，并按实际账号/映射模型结算 usage。可参考 [pipeline 发送 fixture](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/tests/pipeline_send_runtime/fixture.rs)，但 P16 seam 只在 probe 内，不自动覆盖正式 pipeline；下批仍须保留合法 workspace 校验，在正式 sender 的测试边界做窄的 loopback 替换。
Bedrock 普通参数/reasoning/signature/取消 audit 收尾，R01–R16 其余 OAuth/Web/API/媒体，P50 devtest、集中录制和真实逐渠道验收全部保留；总 goal 保持 active。

# 服务商分渠道扩展：第 10 批，Qwen 普通 API 原生目录底座

> 2026-11-02；R09 / W05 / P10 的目录切片，定向离线验证通过；不是 Qwen API 模板或 W05 完成。
> `main`，HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`；未提交、推送、打包或变更运行实例。

## 1. 范围与新核实的事实

延续 [总计划](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/plan/2026-11-02-provider-template-expansion.md)：普通 API、Coding Plan、Qwen Web 不是同一凭据／额度池，API/OAuth 的存在不排除 Web 的独立价值。旧 OAuth 受限项仍保留，未被删除或替换成 Web。

本批重新联网只读核实 [List models](https://www.alibabacloud.com/help/en/model-studio/list-models) 与 [Base URL](https://www.alibabacloud.com/help/en/model-studio/base-url)：

- 官方现在明确重复 `capabilities=TG&capabilities=Reasoning` 是 OR，前批未决项已消除；本批仅面向 Qwen 的文本输入／文本输出目录，不把图像、视频、embedding 等条目变成聊天 route。
- 推理支持六地域 workspace 专属域名；目录在新加坡、香港使用文档指定的 shared origin，其他四区使用 workspace origin。
- 因此 source URL 不能单独证明 workspace。尤其同地域的两个 workspace 可能指向同一个目录 source；必须同时绑定已保存的推理 base。

未使用真实 Key、未向带凭据的官方上游请求；不以文档示例或 loopback 成功声称账号权益、免费额度、真实模型调用或平台实时可用性。

## 2. 已实现的 owner

- [endpoint descriptor](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/provider_discovery/model_catalog/qwen_endpoint.rs)：从已保存的 HTTPS workspace base 派生精确目录目标；支持北京、新加坡、东京、法兰克福、弗吉尼亚、香港，不猜默认区。不接受 userinfo、端口、query、fragment、额外域名／路径、未知 region、跨标签 workspace 或 trial 域名。
- [native reader](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/provider_discovery/model_catalog/qwen.rs)：只做 GET `/api/v1/models`，固定 Qwen / inference / TG-or-Reasoning 筛选与 `page_size=20`；读取 `output` 页码结构，不套 OpenAI `data[].id`。
- [ModelCatalog owner](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/provider_discovery/model_catalog.rs)：新增可选 `accountBaseUrl`，仅 Qwen 目录要求它与精确 endpoint 一致；fixed-source 厂商不接受额外 binding。旧数据缺字段仍可读，`None` 不序列化，不升级存储版本、不迁移真实数据库。
- 公共 `fetch_qwen_catalog(base, key)` 先验证目标与 Key；生产调用不能提供 source URL。私有 transport 仅在 unit fixtures 中替换为随机 loopback，返回证据仍使用真实 descriptor 的 source/base，不产生 localhost 配置例外。

### 2.1 完整性、资源与信任边界

30 秒总 deadline、25 秒单请求 timeout；最多 103 页、2048 个原始目录条目；单页 2 MiB、全轮 8 MiB 解码字节。未核实的 page_size 上限不被冒用，20 是本实现的固定查询大小，不是声称平台最大值。

所有页的 total 必须稳定，page_no/page_size 及期望条目数必须匹配；重复页／模型、短页、假成功、空结果、坏 ID、超预算、重定向、HTTP/JSON 错误均失败，不返回半份目录。失败共用不回显上游 message 的固定错误；不跨区、换账号或执行生成探测。

provider/capabilities 存在时验证类型并排除非 Qwen／非 TG-or-Reasoning；官方示例缺少这两个可选元数据时仍要求明确 Text 输入和 Text 输出。目录不包含 verified models、健康成功或可用额度证据。

请求和 body 流由调用 future 拥有；没有新后台刷新任务、进程、无限队列或自动重试。源 URL 永远由 descriptor 派生，不读取服务端 continuation URL，不跟随重定向携密改投其他 origin。

### 2.2 当前明确不开放的入口

本批不新增可供正常添加的 Qwen preset、模板、管理 catalog handler 或 UI 控件，也不改变 Coding Plan/Web 原实现。目录底座完成不等于可在当前桌面直接使用 Qwen 普通 API。
shared legacy、trial、Anthropic-compatible、Responses 原生 surface 不在当前 descriptor 合同中；不得借本批放宽到所有 `*.aliyuncs.com`。后续如支持这些入口，必须按真实产品合同补齐，不把未知端点当受支持别名。

## 3. 验证与来源

证据目录：[P10](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-provider-expansion-20261102-p10)。测试使用合成 Key、随机 loopback；`TEMP/TMP/TMPDIR` 均在本批 `runtime-temp`，不读取 `.ng`、历史凭据或默认本机服务。

- `red-binding.log`：新合同实际执行 3 项，**2 fail / 1 pass**，旧 reader 拒绝 `accountBaseUrl`，六区／shared-origin workspace 绑定缺口被复现；不是执行路径或编译错误。
- 正向与对抗测试入口：[目录测试](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/provider_discovery/model_catalog/qwen_tests.rs)、[对抗测试](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/provider_discovery/model_catalog/qwen_adversarial_tests.rs)、[私有 wire fixture](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/provider_discovery/model_catalog/qwen_test_support.rs)、[持久化合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/tests/qwen_catalog_binding_contract.rs)。
- `catalog-final.log`：目录 unit **23/23**，包括完整 2048 条／103 页边界、整轮 deadline、取消释放 body、实际解码字节限制与既有目录回归；编译 8m37s，测试 30.05s。
- `binding-final.log`：五个 integration targets 共 **8/8**，其中 Qwen binding 4 项、Cohere/Gemini/OpenAI/NVIDIA 管理目录回归各 1 项。合计本批 Rust 定向 **31/31**，不是全仓回归。
- `feature-check-final.log`：`cargo check --locked --lib --no-default-features --features line-gemini-web-reverse --jobs 4` 通过，证明本批目录底座不依赖 Qwen runtime line；不替代后续 Qwen 推理线的 enabled/disabled 验证。
- `source-frozen-v1.json` 冻结 11 个源码文件；有效行：`model_catalog.rs` 253→264，Anthropic/Cohere/Gemini reader 88→89 / 103→104 / 104→105，既有目录测试 315→319；六个新增文件依次为 endpoint 39、reader 170、正向测试 131、对抗测试 224、fixture 83、binding integration 86。最大 319，无例外或 baseline 改动。
- 最终 formatter、checker、ratchet、来源匹配和文档／scoped whitespace 检查由本目录的 `*-final` 收尾日志记录；未修改原有有效行报告。不把过滤掉的测试算作通过。

本批无依赖／lockfile 变更，无外部源码复制；已有 fixed catalog constructors 仅补 `account_base_url: None`。旧 frozen 证据保留为历史，不覆盖旧 hash。

## 4. 下一个必须闭合的切片

1. 实现普通 API 新 preset 和 native chat planner，按 feature gate 接线；不复活退役 `qwen` / `qwen-dashscope-openai`，不把 Coding Plan planner 的 endpoint 范围照搬过来。
2. 正常空池模板提供显式 region/workspace UI，不默认猜地域；目标由受控 descriptor 生成，底层纯函数再次校验，Key 沿普通 SecretPatch 保存。
3. catalog 管理请求复用 management/Secret Grant/slot/精确 credential target；source/base 从已保存的有效目标导出，不能接受任意客户端 URL。fetch 响应的 revision 不是 CAS；保存仍由正常 commit owner 执行 CAS。
4. 前端 schema、请求验证和 apply 同时接通动态 source + `accountBaseUrl`，包括 credential override 绑定；保留 manual/fetched 所有权、其他账号、disabled route 与 priority。失败／取消／grant或revision变化不覆盖旧证据。
5. 精确账号测试、普通 buffered/stream 请求、取消、feature enabled/disabled、正常 UI→保存→重开→请求合同全部补齐后，再判断 W05 可否 `offline_ready`。

### 4.1 下一批不可遗漏的真实接线位置

只读追踪确认，preset/profile 与 payload ownership 是两条独立识别链；仅注册名称仍会落入 generic OpenAI planner。

- [enabled surface](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/qwen/official_api.rs) 与 [disabled surface](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/qwen/official_api_disabled.rs) 目前只识别 Coding Plan，普通 workspace 必须精确纳入自己的产品分支。[payload line](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/implementation_lines/payload.rs) 也不能漏掉新目标，否则 feature 关闭时可能滑入 generic planner。
- [Qwen planner](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/qwen/official_api.rs) 现有 Coding Plan 测试要求原生 `/responses`；不能全局改成 Chat，也不能把该范围照搬给普通产品。普通 API 的默认上游 Chat path 应为 `/chat/completions`（base 已含 `/compatible-mode/v1`），客户端 Responses/Messages 则使用现有 canonical bridge。
- [endpoint policy](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/routing/candidate/endpoint_policy.rs) 当前通过“无 responses_path、有 chat_completions_path”判定 Responses→Chat；[native Codex 快捷入口](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/client/native_codex.rs) 在普通 planner 之前就可发起请求。只改 planner URL 不足以保护 path/discovery/call-flow 覆盖后的快捷入口；必须在网络前保持普通产品约束。
- 当前 enabled/disabled module 选择依赖 `line-qwen-official-core`，完整 line 还有 `line-qwen-official-api`；测试不能只验一个关闭组合便声称所有 compiled-out 路径均拒绝。

其余 R01–R16、Bedrock 剩余项、P50 devtest、集中录制和真实逐渠道验收继续保留；本批不缩小总目标、不提前要求用户逐个录制。

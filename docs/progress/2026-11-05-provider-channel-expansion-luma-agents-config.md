# Luma Agents API：后端注册、配置准入与普通 Key 保存保护

日期：2026-11-05。对应 [总计划 R14/W10/P10/P40](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/plan/2026-11-02-provider-template-expansion.md)。
本批承接 [独立执行链子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-05-provider-channel-expansion-luma-agents-owner.md)，只闭合后端配置与正常管理保存子链，不是完整服务商、正常 UI 模板、P50 或总计划完成。
工作区仍为本地 Gateway，`main`、HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`；保留其他未提交修改。

## 改了哪些内容

- 注册后端 `lumalabs-api` preset、`lumalabs_agents_api` profile、`lumalabs_agents_media` family 和独立 implementation line；服从既有 `line-lumalabs-agents-official-api` feature，不改旧 Web preset/aliases 或额度池。
- 修正 `agents.lumalabs.ai` 被旧宽匹配归入 Web 的 profile 推断。非 secret 身份使用 preset/adapter/canonical profile/effective base，完整文档另可识别已保存的 `luma-api-*` Key；通用 bearer 本身不算 Agents 身份。
- 原声明与最终合并分别校验。provider 及每个 credential 的 adapter、base、auth、flow、fingerprint、执行模式、模型和有效材料必须仍属于 Agents；禁止混入 Web/OAuth/refresh、generic discovery/catalog、任意 headers/extra_body、其他路径或运行时状态。
- 空池与清空后的空 Key 可以正常保存；非空材料沿执行器原有 Key 条件校验，没有为了配置另造不同格式。支持既有无 flow 和全 Auto flow，不新加半接线的 SpecialAdapter 选项。
- 把 Agents 纳入既有 `KeepBindings`，复用完整 resolved 文档、稳定 ID 和全部共享消费者算法。Keep 不得跨产品/目标或新增继承消费者，即使新账号 disabled 或通过 Clear 去掉自身 override；显式 Replace 新账号材料仍可保存，且不依赖 patch 顺序。
- 继续使用普通 `SecretPatch`、Secret Grant、Origin、expectedRevision/CAS 和 route-config router；没有另造密钥保存协议、迁移数据库或改存储版本。

主要实现：
[preset](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/preset/luma_agents.rs)、
[配置 guard](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/routing/config/luma_agents.rs)、
[编译接线](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/routing/config/provider_compilation.rs)、
[Keep owner](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/console/secrets/keep_bindings.rs)。

## 实际验证与证据

证据目录：[gateway-luma-config-20261105](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-luma-config-20261105)。
Cargo 均显式 `--locked --no-default-features --jobs 4`，主组合 `runtime-defaults`，不启用 `management-ui` 发布/清理 hook；TEMP/TMP 指向本批目录，不设置 `RUST_MIN_STACK`。

| 范围 | 结果 | 回执 |
| --- | --- | --- |
| 新配置 5 项、秘密材料 7 项、真实管理 router 2 项 | 14 passed | `green-management-final.json` / `.log` |
| 同次相邻 Kiro Key Console/秘密材料、Vertex Express Console、Bedrock 秘密材料 | 21 passed | 同上；合计 35 passed，无输入漂移 |
| 原 Luma 执行链 | 20 passed | `green-owner-final` |
| implementation lines / protocol registry / routing config / secret owner | 37 / 22 / 119 / 5 passed | `adjacent-lines` / `adjacent-profiles` / `adjacent-config` / `adjacent-secret-owner` |
| `line-gemini-web-reverse` 代表性 Agents 关闭组合 | 2 passed；preset/line/payload 的独立 feature 元数据合同；与主组合同名，不重复累计 | `green-feature-disabled` |
| 主组合与代表性 Agents 关闭组合的生产非测试库编译 | 两项 exit 0 | `check-production-final` / `check-feature-disabled` |
| `cargo fmt --all -- --check` | exit 0 | `rustfmt-final.log` |
| `npm run test:effective-lines --prefix scripts` | 31 passed | `checker-tests.log` |
| `npm run check:effective-lines --prefix scripts` | exit 0，ratchet；4511 文件，governed source >700=0 | `ratchet.log` / `ratchet-report.json` |

管理测试经过真实 `build_router(...).oneshot(...)` 和正常 route-config 保存 owner，使用合成认证、临时文件与已有 in-memory Redis backend。
覆盖新增 Replace 缺 grant/错误 Origin 403、成功保存/输出脱敏、旧 revision 409、metadata/model/enabled 的无新 grant Keep、Clear 需 grant；擦掉产品后 Keep 在 `/validate` 与 PUT 均返回 `secret_keep_destination_changed`，active revision 不变。
这是 router 与合成持久 backend 的集成测试，不是实际 Redis Lua、真实管理 TCP、生产 SQLite 重建、浏览器或原生 EXE 验收。

主组合按完整测试名去重共 **238 个 Rust 测试通过，14 项为本批新增合同**；代表性关闭组合另执行同名 2 项，不重复累计。所有最终阶段 3017 项选定 Rust/config 输入在运行期间及最终核对时无漂移，详见 `final-verification.json`。

17 项选定 Rust 文件最大 335 有效行，新增 5 项均低于 500；before/after、UTF-8 无 BOM、尾随空白、scoped diff 和局部 rustfmt 见 `source-audit.json`，未改 baseline/exception。
官方 ratchet 的原有共享报告先保存、运行后核对并原字节恢复，见 `static-receipt.json`。工作区完整 diff、未跟踪源码/文档的独立 whitespace、文档链接和最终已测输入来源均由 `final-verification.json` 核对。
限定范围只读复审未发现可证实新增缺陷；这是静态审查，不冒充第二份独立运行证据。

## 失败现场与结果边界

初始两次红测因测试错误调用 snapshot 私有 `get_providers` 而编译失败；仅把测试改为公开的 `RouteConfigStore::get_providers()`，没有扩大生产 API。
随后 `red-contract-v3` 实际执行为 **3 passed / 6 failed**，生产源码未改、3014 项输入无漂移；缺口包括注册/feature、raw Key 混 refresh 和跨产品/新增共享消费者的 Keep。
修补后的首轮 `green-config-secret` 为 12 passed，但运行期间补了两项测试文件内容，回执 `changedInputs=2`，不把它当最终固定输入证据；最终以 `green-management-final` 及后续收尾回执为准。
完整警告与失败日志保留，不运行 `cargo fix`、不扩展为无关清理，也不声称全 feature 矩阵、strict 债务审计或全仓回归通过。

## 下一接力点与未完成范围

1. **仍为 15 个累计新增正常模板**：本批后端 preset 已注册，但还没有开放前端 `lumalabs-api` 空池模板/普通 API Key 编辑、暂存受众与模型管理 UI；不把后端 registry 数量当正常模板数，也不新增整卡 offline_ready。
2. 接明确模型策略与精确账号 probe，再做正常 UI→真实管理 TCP/SQLite→保存/Keep/清空→同目录 runtime 重建。不能用合成 backend 代替这条链。
3. 继续公共媒体 router→普通 Key/route→发送→响应→usage/audit/本地结算与失败/取消验收。`credits_charged=null/0` 当前仍只是已接执行链的供应商报告语义，不能声称已经入账。
4. `video_edit / video_reframe / layering`、EXR、Files、LLM 操作范围及视频真实解码/播放继续；HDR+10s 官方文档矛盾仍待权威证据。旧 Web board 的剩余材料/worker/资产合同独立推进，API/Web 额度关系保持 unknown，不混池或自动互相回退。
5. 其他原定渠道、OAuth 兼容/恢复剩余、Qwen 受限项与 P50 全范围无凭据集成/devtest 包、P60 集中录制、P70 真实模型/权益/费用/额度验收均未完成；不要求现在提供凭据。

本批没有读取或试用真实/历史凭据、调用平台生成 API、创建 Redis/Docker 资源、修改在线实例、出包、部署、提交或推送。
前序 P34 单容器许可已使用并清理；重复回放的问卷回执不成为新许可，本批未复用。

# 服务商分渠道扩展：Udio 路由模型绑定与原生提交准入

日期：2026-11-06 至 2026-11-07。工作区：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`。
本批承接 [结果与公共音乐元数据子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-06-provider-channel-expansion-udio-admission.md)，修复已解析模型与实际请求脱节，不是当前平台模型、精确媒体 probe、有效音乐或完整 Udio 验收。模板仍 16，无新增整卡 `offline_ready`；P50/P60/P70 及总目标未完成。

## 1. 实际发现与修改

原 `build_generate_request(req, _model)` 完全忽略路由解析模型，caller 的 `model_type` 或旧默认值决定 wire。借用浏览器的 native Create 路径只填 prompt，更不能保证页面当前模型与配置一致。因此没有先接一个会恢复模型健康的“精确模型测试”来掩盖该问题。

### 路由身份是唯一模型来源

[model binding owner](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/udio/model_binding.rs)由原 generation builder 调用：

- `udio-music` 仅保留为既有操作别名，显式映射到既有 `udio32-v1.5` 默认值；这不是当前 native 模型可用性声明。
- 其他配置模型作为 opaque native identity，非空、最多 128 UTF-8 字节，不允许 whitespace/control；不 trim 后接受，不凭第三方代理文档发明目录、版本或权益。
- top/nested 的 `model_type / modelType / upstream_model_type / upstreamModelType` 如出现，逐个严格等于解析后的 native identity；冲突、null、数字等拒绝，不能由 first-match 或 nested key 优先级绕过。显式非对象 `gen_params` 不再默认为空。
- 验证后只写 canonical `gen_params.model_type`，不继续依赖旧的“不覆盖已有 key”插入 helper。拒绝为 400、Udio provider、no retry/Abort，错误不回显模型/正文。

兼容边界：既有 `udio-music` 默认仍可使用；过去靠附加 `modelType` 在路由许可之外另选模型的调用现在拒绝。需要其他模型时应配置账号模型与 `model_map`，不修改凭据存储格式或迁移 `.ng`。

### 借用页面：模型不符就拒绝，不改写请求

[native model gate](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/scripts/udio-browser/native-model.mjs)在精确 origin/path 的 POST 出网前检查实际 body 中 canonical model 及四个 aliases；单次槽在任何异步 route 操作前占用，首次坏请求也不能被第二次请求补成成功。不改写 native body、Cookie 或 auth，不猜 UI 模型选择器，不重提。

[native submit owner](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/scripts/udio-browser/native-submit.mjs)先等 gate ready，再启动原有 bounded capture，两者 ready 后才 arm 与唯一 click；已知 Service Worker controlled page 在 click 前拒绝。只移除自己的 predicate/handler，不调用 `unrouteAll`；延迟 registration、stop 与活动 handler 有退出 owner。新增红测还复现并修复：cleanup 期间的 capture failure 误变成功、Service Worker preflight 超时迟到后继续填写借用页面。

以下边界刻意保留，不能扩大描述为完整浏览器保真或取消：

- `route.continue()` 最终拥有这次匹配 POST，避免后续 handler 再改 body；其他请求/handlers 保留，但不声称同 URL 的后续 handler 仍参与这次 POST。
- 这是被拦截请求的模型准入，不是任意 Worker/浏览器流量的安全沙箱；已知 controller 检查不排除后续 Service Worker 注册竞态。
- cleanup 卸载 handler、取消本地等待或断开 CDP，不证明已触发 UI 的迟到 POST 或远端生成被撤销，不保证供应商退款。
- 目前只保证此模型字段，不证明 seed、lyrics、数量、auto-extend 等完整参数都已按 Rust body 在 UI 设置；精确媒体 probe/UI 仍未接，不能用 metadata 恢复音频可用性健康。

## 2. 实际验证

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-udio-model-binding-20261106/`，见 `audit.json / closeout.json / passed-test-names.json / scoped.diff`。

- Rust 真实红测 **2 passed / 3 failed**；Node 模型 gate 红测 **0/8**，收尾生命周期补充红测 **0/2**，均保留失败日志。最后两项已进入仓库的长期回归测试，不只保留临时脚本。
- 最终正常 `runtime-defaults` 的 `udio` 筛选 **214/214**，含 **6 新增**；筛选也匹配 AIStudio，不称纯 Udio 214。正常公共 router/普通 Key/SQLite/回环 executor 验证公开别名→配置 native model→实际 requestBody 一致，冲突在 executor 前 400，余额退款且无切号/fallback；原 metadata 成功/失败/取消合同仍通过。
- 最终 Udio Node **109 passed / 1 skipped**；真实隔离 Chromium/CDP **14/14**（父项及 13 场景），共享 native 相邻 **37/37**。模型不符、缺失、冲突三个真实浏览器场景生成 POST 为零；原在途 POST 在新请求已准入后先返回时，因缺本次 capture request identity 明确失败，不误收旧结果。每场景检查 worker 退出后本次 route 已卸载、原页与同 context 其他页可用。这不是用户浏览器或真实音乐。
- 正常非测试生产 lib `cargo check`、Rustfmt、合成包支持文件/hash/实际 import 合同 **1/1**、checker **31/31**、ratchet **4729 files**、UTF-8 无 BOM 与 scoped diff 检查通过。无 baseline/exception 改动，根 effective-lines 报告按直接 preimage 逐字节恢复。
- 15 项审阅源码最大 **287** 有效行；generation 288→273，模型绑定新模块 51，native gate 56，native submit 68→83。Rust 最终阶段选定 3830 个 Rust/Node 输入无漂移；后续只改 JS/文档，最终仍匹配该阶段全部 2643 个选定 Rust/Cargo/build 输入。最终 Node/CDP/生产检查等各 3831 项选定输入无漂移。generic runner 不覆盖所有 Python/PowerShell/动态夹具，不能据此称全仓来源冻结；包测试 Python preimage 另存并审阅。

未执行全仓/全 feature 矩阵、Server/PG、独立 Gateway 入站进程、原生 EXE/完整交付包或真实平台验收；未读取真实凭据、发生成、下载/播放音乐、改在线实例或数据库、创建 Redis/容器、提交或推送。没有复用历史 P34 容器许可。

## 3. 当前公开事实与下一步

本批重新联网读取官方 [public API](https://help.udio.com/en/articles/10756277-udio-public-api)、[model updates](https://help.udio.com/en/articles/10756068-model-updates)与历史 [v1.5 公告](https://www.udio.com/blog/introducing-v1-5)。这些资料不能证实当前内部 model IDs；搜索出现的第三方代理型号未当官方合同。connector 提取文本的观察记录在本批 `public-sources.json`，不冒充原始 HTTP/hash 或已登录 schema。

后续仍须当前模型/完整参数/wire 与 session/refresh 合同、精确单账号媒体 probe/逐次费用确认/UI/Keep、有效音乐及适用播放；官方下载限制和不绕行边界见 [Udio 合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/provider-udio-channel-contract.md)。远端取消/lease、原 image 下载准入、跨请求 exactly-once、Server/跨进程/整包继续。其他原渠道尤其 Luma Web、FreeBuff 公共业务、Suno 公共链及 OAuth 候选、Kiro/Qwen OAuth 未完项不被本批替代。

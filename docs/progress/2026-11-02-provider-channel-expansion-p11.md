# 服务商分渠道扩展：第 11 批，Qwen 普通 API 运行边界

> 2026-11-02；R09 / W05 的运行底座切片，定向离线验证通过；不是普通 API 模板、W05 或总目标完成。
> `main`，HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`；未提交、推送、打包或变更运行实例。

## 1. 范围与产品身份

延续 [P10 目录底座](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-02-provider-channel-expansion-p10.md) 的六区 workspace endpoint 合同。
普通按量 API、Coding Plan、Qwen Web 仍是分开的产品渠道；API Key/OAuth 的存在不排除 Web 的价值。
本批没有添加第四种认证或新额度产品，也不改变 Coding Plan `/responses` 与 Anthropic 格式的旧行为。

[workspace policy](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/qwen/workspace_api.rs) 使用内部 `auth_mode=qwen_workspace_api` 标记普通 workspace 产品身份，实际 HTTP 认证仍是 **Bearer API Key**。
标记不是最终模板 ID，不伪装 OAuth、不代表另一份额度；它用于防止后续 base/adapter merge 把普通 Qwen 材料泛化为其他产品的通用 payload。
精确 workspace base 即使没有标记，也识别为普通产品并执行相同限制。其他品牌和既有 Coding Plan 不受此标记的默认影响。

## 2. 已实现与信任边界

- [call-flow preparation](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/call_flow/preparation.rs) 在 native Codex/Claude、其他 executor 和真实请求之前校验产品；有 flow 时同时检查最终 adapter，不能通过 call_adapter 切换到 Anthropic 或其他传输。
- 只接受 P10 精确 workspace descriptor；不接受 marker 配合 Coding Plan、Web、任意 HTTPS、loopback 或错误路径。生产代码没有 localhost allowlist 例外。
- 只允许 OpenAI-compatible、默认或精确 `/chat/completions`；拒绝 responses/messages/completions/非文本 path、generic discovery、session auth、自定义认证头、Cookie/Host/认证覆盖及 browser-backed mode。
- 外部 keepalive steward 不属于普通 API 合同，明确拒绝。现 pipeline admission 先执行 preparation，后才到 ensure_payload_ready，因此不能先把 API Key/headers/body 交给外部 steward 再由 planner 拒绝。
- [endpoint policy](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/routing/candidate/endpoint_policy.rs) 固定普通产品的 Responses→Chat 语义，不依赖可编辑 paths 恰好满足通用 bridge 条件。客户端 Chat/Responses/Messages/Completions 使用既有 canonical Chat packer；不能恢复 native Responses/compact 资格。
- [Qwen planner](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/qwen/official_api.rs) 保持文本 endpoint 限制并在网络前检查有界 ASCII graphic Key；与通用 OpenAI body/响应 owner 复用，不另起网关。
- [implementation line](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/implementation_lines/payload.rs) 和 enabled/disabled protocol surface 均识别普通产品；API/family 不可用时在快捷入口之前返回 `gateway_provider_line_compiled_out`，不退回 generic OpenAI。
- [upstream module gate](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/qwen/mod.rs) 由完整 `line-qwen-official-api` 选择真实 planner；protocol surface 仍按 core 分派。修复 core-only 缺 family 的 unresolved import，也收紧此前 API-off/core+family-on 仍可直接取得真实 Coding Plan planner 的边界；完整 API/default 配置的 Coding Plan 行为保持。
- [最终 sender](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/client/streaming.rs) 禁止 workspace 请求跟随重定向，保护 buffered/stream 的同一发送 owner。

校验仅返回固定分类错误，不包含 Key、Cookie、上游 body 或可控目标文本。没有新后台任务、进程、自动重试、持久化字段、依赖或 lockfile 变更；调用 future 继续拥有网络请求和响应资源。

## 3. 验证记录

证据目录：[P11](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-provider-expansion-20261102-p11)。测试只使用合成材料、随机 loopback；TEMP/TMP/TMPDIR 置于本批 runtime-temp。

- `red-runtime.log` 是测试调用未知方法 `line.key()` 的编译错误，不算行为红测；修正为现有 `canonical_protocol_profile()` 后，`red-runtime-v2.log` 实际执行 **4 fail / 0 pass**，复现产品归属、目标改写、路径改写和缺省 Chat bridge 缺口。
- `green-runtime-v1.log` 是错误 import 的编译失败，不算绿测；已改为 descriptor 的真实公开路径，不修改 P10 frozen 源码。
- 初轮 `feature-core-on-family-off.log` 真实复现两项 common import 编译错误；不是测试失败，也不能以首个关闭组合通过就声称整个矩阵通过。`source-before-addendum-v2.json` 单独保存后来纳入 scope 的 upstream module 变更前指纹；旧 before 证据不覆盖。
- [integration contract](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/tests/qwen_workspace_runtime_contract.rs) 覆盖六区、四个文本入口、stream/buffered 计划、marker 与未标记 raw payload、Key/认证/path/discovery/flow/keepalive 拒绝和 compiled-out。
- [native/sender unit](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/client/qwen_workspace_tests.rs) 检查 native Codex/compact 资格、compact 提前拒绝、真实最终 sender 的 Bearer/Chat body/路径与 307 不跟随。
- sender fixture 先执行真实 descriptor、preparation 和 planner，然后仅在测试代码替换 transport target 为随机 loopback；不是完整生产 pipeline、管理 UI 或真实阿里云验证。
- `qwen-lib-final.log`：默认配置 Qwen 相关 unit **64/64**，包含 Coding Plan Chat/Responses 原路径与 P10 目录对抗回归；编译 6m59s，测试 30.02s。
- `runtime-final-v2.log`：默认配置新增 integration **6/6**；编译 5m27s。默认定向合计 **70/70**，不是全仓回归。
- API-off feature matrix：下列测试均增加 `line-gemini-web-reverse` 作为仓库已使用的最小编译底座；feature 依赖没有改动。

| API | core | OpenAI family | Anthropic family | 证据与实际结果 |
| --- | --- | --- | --- | --- |
| off | on | off | off | `feature-core-on-family-off-final.log`，5/5 |
| off | on | on | off | `feature-core-on-family-on-final.log`，5/5 |
| off | off | on | off | `feature-core-off-family-on-final.log`，5/5 |
| off | on | on | on | `feature-core-on-both-families-final.log`，5/5 |

每个关闭组合包括普通产品 compiled-out、marker 目标/路径/flow 拒绝，以及 Coding Plan Chat/Responses stub；重复组合执行不是 20 个独立新测试。完整 API 开启会由 Cargo 自动启用 core 与两个 family，不虚构“API-on、family-off”的不可实现组合。

四种关闭组合实际执行 **20/20**；加默认配置共 90 次定向测试执行，未计过滤或未执行的测试，不代表 90 个独立用例或全仓绿灯。

`source-frozen-v1.json` 冻结 13 个源码文件，并以 `source-scope.mjs --verify` 核对。新增 policy/native tests/integration 分别为 84/131/183 有效行；既有 owner 前后为 line payload 115→118、preparation 133→135、protocol module 10→11、enabled/disabled surface 141→146 / 55→60、endpoint policy 215→221、native Codex wiring 241→244、sender 348→350、upstream module 10→10、Qwen planner 186→190。最大 350，无 >500 新文件、例外、baseline 或 immutable policy 改动。

`format-final-v3.log` 通过；`checker-final-v3.log` 为 31/31，`ratchet-final-v3.log` exit 0：3955 文件、14 个 immutable runtime artifacts、governed source >700=0。直接使用同一 checker 的无报告写入 ratchet 入口，未覆盖原有脏报告。
`p10-source-recheck.log` 再次验证 P10 的 11 文件 frozen 来源未漂移；`document-final.log` 记录四份当前交接文档的 UTF-8 无 BOM、围栏、本地链接和 17 个 scoped 文件的 tracked/untracked whitespace 检查。

本批没有 UI 源码变更，不运行假 UI 构建或正式发布；未构建新 EXE，未执行完整生产 pipeline、真实上游、真实账号或全仓回归。已知其他批次的未处理失败不因这批定向绿灯自动消失。

独立只读审查发现未拒绝外部 keepalive 的受众缺口，本批已补 policy 拒绝；未运行真实 steward、官方上游或历史凭据，不产生费用／token 轮换。

## 4. 下一切片：尚未开放的普通 API 产品

本批**不注册普通 API preset、不新增模板或管理/UI 入口**。W05 仍 `in_progress`。

下一批必须接新 preset `qwen-official-api` 与 profile 默认，并在 [最终配置编译 owner](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/routing/config/provider_compilation.rs) 保留原始 preset 身份后校验所有有效 credential 的最终 payload，不能仅凭已改错的 base 判断是否属于普通产品。
provider 默认账号按 `uses_provider_default_account()` 判定；显式无账号空池不是可执行账号，不凭 Key 是否为空猜模板。
特别注意：正常 flow 编译会提前调用 prepare；新空池的 region/workspace 未填、provider 层 Key 不存在但凭据可覆盖等抽象模板状态，必须与实际账号分开处理，不能为通过空池编译而放宽运行时目标保护。

之后仍需显式六区 region/workspace UI、SecretPatch、动态目录管理授权/slot/精确 credential、source+accountBaseUrl 前端读写与 CAS/fences、精确 probe 和真实 pipeline buffered/stream/响应/取消合同。
本批未证明 UI→保存→重开→请求完整闭环、实际模型权益、免费余额、真实刷新、媒体或其他平台能力；不加入真实验收标蓝记录。
Bedrock 剩余项、R01–R16 其他 OAuth/Web/API/媒体、P50 devtest、集中录制与逐渠道真实验收继续保留。

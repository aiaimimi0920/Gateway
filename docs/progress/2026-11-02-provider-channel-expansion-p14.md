# 服务商分渠道扩展：第 14 批，普通 Qwen 原生目录管理与账号保存

> 2026-11-02；R09 / W05 / P01 / P10 的无凭据切片，定向验证通过；不是 W05 或总目标完成。
> `main`，HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`；保护 P01–P13 脏改，未提交、推送、打包或重启实例。

## 1. 产品边界

仍为 `templateId/providerId=qwen-api`、`preset=qwen-official-api` 的普通 workspace API Key 产品，实例 ID 可改。
普通 API、Coding Plan、Qwen Web 不混凭据、目录或额度；API Key/OAuth 存在不排除 Web 的独立价值，未知额度关系保持隔离。
本批沿 P10 六区 descriptor/native reader、P11 sender、P12 keep 受众与 P13 正常模板/材料链接线；没有新建转发框架、认证类型或存储版本。
公开产品事实继续引用 [API 合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/plan/provider-api-contracts.md) 已记录的联网来源；本批无新的第三方代码复制、依赖升级、上游请求或历史凭据使用。

## 2. 前端实现

### 2.1 动态目录与旧格式 ownership

- `qwenEndpoint.ts` 派生六区目录来源，新加坡/香港使用各自 shared source，同时保留 canonical workspace inference base。
- `officialModelCatalog.ts` 同时验证 `sourceUrl + accountBaseUrl`；不往 fixed `catalogSources` 塞伪固定 Qwen URL。固定目录必须没有 account binding；未知字段不再被剥离以洗白错误目录。
- `catalogMatchesProvider` 再将目录绑定普通产品和账号最终目标；credential override 优先于 parent，同一 shared source 的其他 workspace 不可混用。
- `providerCredentialDraft` 完整读取旧 native 目录；没有 `model_selection` 时仍按目录区分 fetched/manual，元数据保存不丢目录，复制账号只复制公开目标和 manual。
- 不匹配/非法的旧 Qwen 目录使 reader 返回 null，阻止普通编辑，不静默删掉证据或将 fetched 洗成 manual。此时普通提示仍沿既有找不到账号提示；专门的修复 UX 尚未新增。
- 应用目录仍沿原 model-selection/route owner，保留 manual、其他账号、禁用路由和既有优先级。目录不是 entitlement、剩余额度或生成成功记录。

### 2.2 新材料与异步结果

- 正常 CredentialDialog 开放 native Qwen 拉取按钮，仍不开放 generic discovery、后台 generic job 或其他产品目录。
- keep 只发 `{credentialId}`，且不允许把已保存 Key 用于修改后的目标。Qwen 有 dirty/busy 配置时，stored 读取先拒绝；显式新材料仍可配合已验证草稿目标读取。
- replace 发送 `providerId + accountBaseUrl + apiKey`，由原 client 携管理 token/Secret Grant 访问同源目录 endpoint；来源不能由客户端指定。
- `qwenModelCatalog.ts` 以 WeakMap 保存 dialog-local receipt，匹配当前 provider/credential、canonical base、Key 和 operation。它不是持久化授权或额度证据，比较材料不进入文档/日志，目录对象释放后可回收。
- 新 Key 的新目录经过 receipt 验证可保存，不被“replace 清旧证据”误删；改 Key/身份/目标、clone 旧 receipt、clear、replace 回 keep 或撤权会丢弃不再适用的 fetched/catalog，保留 manual。
- 复用请求 abort、active-request、editor/revision/token/grant fence；Qwen parent 目标/auth/preset 也加入 dialog fence。迟到结果不能覆盖当前草稿。

没有新后台循环、浏览器依赖或大数组状态副本；目录限制继续沿 2048 项/ID 256 bytes。React 改动保持事件触发、派生目标和受控字段，撤权 effect 只负责生命周期清理，没有增加 effect 驱动的网络 waterfall。

## 3. 后端管理边界

新增 POST `/v1/internal/gateway/console/qwen-model-catalog`，沿正常 management router。
两种互斥 DTO 都 `deny_unknown_fields`：Stored 只允许 `credentialId`；Draft 必须同时显式提供 `providerId/accountBaseUrl/apiKey`。
Stored 用精确 credential 的已编译有效 payload，核对显式所属 provider/普通 preset，不从 parent 取代 override，也不借用其他账号或 provider-default。
Draft 只能使用本次显式输入的新 Key；目的地仍限定 `QwenEndpoint` 的六区 workspace grammar，不接受任意 base、source、重定向或客户端 transport override。

顺序为 management auth → Secret Grant → 共用 discovery slot → snapshot/精确目标 → 原 native reader → 再验 grant/revision → 目录目标验证 → no-store 响应。
目录读取不写配置。返回 revision 和 fetch 后检查只是读取 fence，最终保存仍经原 SecretPatch/CAS；不能用读取成功代替保存 CAS。
feature 被关闭时，在网络前返回 `gateway_provider_line_compiled_out`，不降级到 generic discovery。
`read_with_fetch` 只是私有合成测试 seam；公开 handler 固定调用 `fetch_qwen_catalog`，没有新增运行时注入参数。

slot guard 贯穿 fetch/后验检查，future drop 时 RAII 释放；本批没有 spawn 新 worker、后台任务或改变既有 native reader 的分页/字节/deadline预算。
`src/lib.rs` 的 cfg(test) self alias 仅使 lib 单测复用同一 integration 合成 fixture，不改变生产库。

## 4. 实际验证与证据

证据目录：[P14](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-provider-expansion-20261102-p14)。命令的 TEMP/TMP/TMPDIR 全部指向本批 `runtime-temp`，只用合成材料。
所有 Cargo test 明确 `--locked --jobs 4 --no-default-features`，本批 runtime 组合使用 `--features runtime-defaults`，关闭 `management-ui`；不重复 P13 的 Web 发布/清理 hook。
前端编译用已检查的 RSBuild API 隔离脚本，移除 live publish 插件、关闭 clean，产物只写本批新 `web-bundle-*`，不覆盖旧目录。

### 4.1 红测、修正与最终结果

- `ui-schema-red.log`：**10 fail**，实际复现六区 native schema/reader/apply 未接，以及固定来源剥离错误 binding 的问题。补严格 schema、descriptor 和 ownership 后对应测试转绿。
- `ui-fetch-red.log`：**4 fail**，实际复现普通 Qwen native 拉取入口尚未开放；不声称每项后续断言都已单独到达。实现后再扩充错误目标、关闭、parent 变化、撤权与 receipt 用例。
- `management-red.log` 的缺 `base_url` 是合成 fixture 错误，不计行为红测。修正真实 preset/base 后，`management-red-v2.log` 实际得到缺路由 **404 而非预期 403**；随后接入正常 router。
- `handler-unit-v1.log` 的 E0373 是测试线程 closure 缺 `move`，修正测试后继续；不计生产行为红测。独立后端复审还指出全局 slot 测试并行竞态，已在该 handler 测试组加测试专用串行锁，最终以 `--test-threads=4` 执行通过，不把它外推成全 lib 并行稳定性证明。
- `ui-regression-v1.log` 为 **127 pass / 1 fail**；失败用例同时改了 workspace 却保留旧非 shared source，schema 正确提前拒绝，但测试预期的是下一层账号目标错误。仅修正合成响应，使其自身来源有效但属于另一个账号；不是放松生产验证。
- `ui-regression-v3.log` 最终为 **19 文件 / 128 pass**，涵盖新 Qwen schema/reader/ownership/dialog/hook、P13 目标与材料绑定、共享 model-selection，以及既有固定目录回归。v2 和早期局部绿测是重复执行，不再次累加。
- `management-green-v1.log`：真实 router 的 Qwen 输入/授权负向 1、Qwen 六区目录绑定 4、共享 Cohere 负向 1，共 **6 pass**。
- `handler-unit-final.log`：**5 pass**，覆盖 stored disabled override 精确 Key/目标、draft 新材料、读取不落盘、实际 PUT SecretPatch 保存与旧 revision CAS 冲突、fetch 后 logout/revision 变化/错误共享来源绑定拒绝、busy 准入和 future drop 后 slot 释放。
- `native-reader-final.log`：原 native reader **11 pass**，包含 2048 条/103 页边界、精确查询、Text 筛选、缺页/重复/错误 schema、禁止重定向、总 deadline、解码后字节预算和取消 body 释放；只访问本地合成 HTTP。
- `feature-disabled-final.log`：`--no-default-features --features line-gemini-web-reverse` 下 **1 pass**，确认普通 Qwen API 被关闭时管理入口不转 generic。该组合仍保留 Gemini Web 最小底座，不称全 features 关闭或完整 feature 矩阵。
- 本批最终 Rust 定向合计 **23 pass**，不是全仓回归；lib 名称 filter 未选中的测试不是通过，重复的 handler-unit-v2 5 项不再次计入。未重跑 P13 已知未修的极端 expiry 用例。
- `typecheck-final.log` exit 0；`web-build-v2.log` / `web-build-receipt-v2.json` 记录最终隔离 Web bundle **14 文件、published=false**。v1 bundle 是调整共享 hook 检查范围前的中间产物，保留不覆盖；它们均不是 EXE 包或浏览器人工验收。
- `checker-v1.log` **31/31**；`ratchet-final.log` exit 0，3981 文件、14 immutable、governed source >700=0。使用 checker 无报告入口，未覆盖原有脏的有效行报告。
- `format-final-v2.log` 的官方 Rustfmt check、全工作树 `git diff --check`、源码/文档严格 UTF-8 无 BOM、untracked whitespace/围栏/本地链接检查通过。没有更换 JS formatter 或为此重排其他前端文件。
- 已做两次窄范围只读独立复审，前端未发现阻断性问题，后端的测试并行风险按上述修正；这是静态审阅，不等于新的动态执行数量。

### 4.2 验证边界

handler 成功读取使用私有合成 fetch seam；保存/旧 revision 冲突使用真实管理 PUT route。前端用合成 client/响应测试正常事件链；本批不是一次真实浏览器到真实 Qwen 的端到端验收。
grant 后验覆盖 fetch 期间 logout 引起的撤销，不冒称独立测试了自然 TTL 时钟到期；取消覆盖 handler future 被 timeout drop 和原 native body 释放，不冒称实际 HTTP 客户端断连必然取消 reader 的端到端证明。
没有调用真实 Key/登录/刷新/生成/额度接口，没有运行付费上游、devtest 出包、正式 Release、安全发布门禁、实例切换或全配置矩阵。
所有本批 Cargo/exec session 已实际结束后才收尾，不把观察超时视为完成或另起重叠 Cargo。

## 5. 有效行与来源

本批 18 个源码文件均按官方 lexer 测量，最大 473；无 baseline、例外、lockfile 或 immutable policy 变更。
初始 hashes/有效行见 `source-before.json` 与不覆盖的 addendum；最终 `source-frozen-v1.json` 记录全部 18 个源码。P11/P12 各自 frozen source 复核仍匹配；P14 有意修改的 P13 owner 以本批 before/frozen 为准，不伪称 P13 全部来源未变。验证收尾再次核对冻结来源和五份本批文档，不用 hash 匹配替代旧批全部测试重跑。

| 文件 | 前 → 后有效行 |
| --- | ---: |
| `qwenEndpoint.ts` / `officialModelCatalog.ts` / `nvidiaModelCatalog.ts` | 33→42 / 93→109 / 10→10 |
| `providerCredentialDraft.ts` / `qwenCredentialDraft.ts` | 150→152 / 37→47 |
| `CredentialDialog.tsx` / `useAccountDiscovery.ts` | 456→473 / 95→104 |
| 新 `qwenModelCatalog.ts` | 29 |
| `src/lib.rs` / `internal_console.rs` / `management_config.rs` | 58→60 / 397→399 / 284→288 |
| 新 backend handler / lifecycle tests / router contract | 171 / 313 / 113 |
| 新四份前端 catalog/ownership/hook/dialog 测试 | 55 / 47 / 54 / 83 |

前端文件位于 `apps/desktop/src/features/console/`。313 行测试集中验证单一管理 handler 生命周期，无新增 >500 文件。

## 6. 下一步与未完成项

W05 仍 `in_progress`，不标 offline_ready，不显示真实验收完成色。下一最小切片：

1. 已有 provider 父级六区/workspace 目标的正常编辑入口，保留精确 credential override 和 keep/replace/clear 授权；不将 raw JSON 当正常入口完成。
2. 普通 Qwen 精确账号 probe，沿现有测试 owner，不借号、不跨区、不把目录可见当生成成功。
3. 当前普通 preset 的完整 pipeline buffered/stream/响应/取消闭环，以及正常管理/UI 的合成集成；本批目录测试不能代替推理管线验收。

Bedrock 参数/reasoning/signature/取消 audit，其他 R01–R16 OAuth/Web/API/媒体，P50 devtest、集中录制与真实逐渠道验收继续保留。
没有读取或试用历史凭据，没有登录/刷新/付费调用，没有交付新 EXE、全仓回归、真实 Qwen 成功或总目标完成结论。
P13 的构建副作用及授权遗漏仍保留在其第 3.1 节，本批记录不抹除历史，也不授权后续工作区发布/清理。

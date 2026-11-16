# 服务商分渠道扩展：第 15 批，普通 Qwen 父级默认目标的正常编辑

> 2026-11-02；R09 / W05 / P01 / P10 的无凭据切片。父级默认目标编辑及定向验证完成；不是 W05 或总目标完成。
> `main`，HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`；保护 P01–P14 脏改，未提交、推送、打包或重启实例。

## 1. 身份与范围

仍为 `qwen-api` 模板、`qwen-official-api` preset 的普通 workspace API Key 产品，不是 Coding Plan、Qwen Web 或 OAuth。
API Key / OAuth / Web 必须分别评估，Web 不因存在其他认证而排除；未证实额度共享关系保持隔离和 `unknown`，不制造独立额度结论。
本批没有新产品、依赖、数据库版本或第三方源码复制；产品公开事实沿已联网冻结的 [API 合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/plan/provider-api-contracts.md)，没有新增上游请求或历史凭据使用。

本批只实现 P14 下一步中的正常父级编辑切片，并完成精确 probe 的只读缺口调查。完整推理管线、其他 R 项和集中验收继续保留。

## 2. 正常界面与事务

- 普通 Qwen 池的卡片背面增加“Qwen 默认目标”，复用已有六区/workspace 字段，推理 URL 只读派生，不以 raw JSON 代替正常操作。
- 输入只保存在 dialog-local state；打开、输入、取消、翻面和保存元数据均不发目录、模型测试或生成请求，不读取 Key。
- “应用到草稿”复用 `useProviderCatalogEditor` → `replaceEditorDocument` → 原自动保存 → 原 SecretPatch/revision/CAS，不新建持久化入口。
- 应用后只有正常保存成功才生效；保存失败沿控制台错误显示，保留尚未生效草稿，可重开查看和继续编辑，不冒称 active 配置已变。
- provider 的打开时 public JSON snapshot 必须与当前草稿一致；期间账号/目标/公开配置变化使对话框 stale，禁用输入/应用并要求关闭重开。等值 refresh 不抹掉本地输入。
- mutation permission 被撤销时关闭并丢弃输入；恢复后重开读取当前草稿，不复活旧输入。

没有新增网络 effect、后台任务、worker、浏览器或存储依赖。仅一个 effect 负责 locked 生命周期清理；Radix Dialog 复用既有 focus/modal/close 机制，字段和确认项保留可访问 label。

## 3. 现有账号不能因默认目标编辑而换受众

`qwenProviderEndpoint.ts` 先核对唯一普通产品 provider、snapshot、显式账号池、六区目标和账号身份/旧有效目标；所有检查通过后才 clone 并返回完整草稿，不产生部分编辑。

1. 默认目标变化且存在继承账号时，必须明确勾选“固定原目标”。这些账号补上旧有效 `base_url`，不授权旧 Key 发送到新地域/workspace。
2. 既有显式 override 原样保留；包括 disabled 账号，不因当前禁用就放宽身份/受众。
3. 账号 Key、目录、fetched/manual、声明模型、禁用状态、路由、优先级、alias 和其他 provider 不变。这里保留证据的理由是**账号实际受众没有改变**，不是将旧目录重绑给新目标。
4. 已暂存 Key 的 binding 不被重算。最终 `buildCredentialSecretPatches` 对同一旧有效目标继续认可；真正修改账号目标后仍拒绝旧 staging。
5. 空 parent 且没有继承账号时可首次设置默认目标，包含空池和全部显式 override 两类。原目标未知而仍有继承账号时继续拒绝，不凭空“固定”未知受众。
6. 要迁移某个账号，仍须使用账号编辑显式 replace/clear Key；本编辑器不提供一键携旧 Key 跨区迁移。

此切片不迁移 provider-default 秘密：非显式池或 active parent 含认证秘密时，正常按钮禁用；打开期间出现该情况也禁用输入/应用并提示。无关 storage password 不误判为认证材料。
helper 对直接存在的 parent Key/token 另行拒绝，但不宣称它能够单独看到 vault descriptor；最终后端 `keep_bindings` 仍是受众授权守门。
账号身份去重及旧目标检查使用一次线性遍历和 Set，不为每个账号重新全表搜索；编辑时 clone 一次现有文档，不影响请求热路径。

## 4. 实际验证

证据：[P15 目录](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-provider-expansion-20261102-p15)。所有命令显式把 TEMP/TMP/TMPDIR 放到本批 `runtime-temp`，只用合成材料和内存管理 fixture。
Gateway Cargo 串行运行，`--locked --jobs 4 --no-default-features --features runtime-defaults`，未开启 `management-ui`，不触发 P13 的默认 Web 发布/清理 hook。观察超时续等同一 handle，没有启动重复 Cargo。

### 4.1 红测、修正和最终结果

- `ui-entry-red.log` 首轮错误定位在旧 card 名称：实际为 `Qwen Ordinary-api`，不是测试假设的 `Qwen Ordinary`。这是 fixture/selector 失配，不计功能红测。
- 修正 selector 并临时撤回本批自己的 Browser 接线后，`ui-entry-red-v2.log` **2 fail**，实际复现缺少“Qwen 默认目标”正常入口；随后恢复接线并转绿。
- 独立只读复审发现合法空 parent 无法首次应用，已修正并增加空池/全部 override 的 pure 与组件用例；原目标未知的继承账号继续拒绝。第二次复审未发现新阻断，随后补充默认材料状态变化的可见提示。
- `ui-green-v1.log` 为 **19 pass / 1 fail**；测试 `it.each` 将空数组/对象展开为参数，造成 fixture 不再是账号数组，已改成具名对象。`typecheck-v1.log` 同时暴露了该参数问题和测试对 unknown provider 缺少窄化；均已修正，不计生产行为红测。
- `ui-regression-v2.log` 为 **84 pass / 5 fail**。新增 Browser 用例在同时编译 Rust 时超过原 5 秒测试预算；未单独证明资源竞争是唯一原因。该集成用例改为显式 15 秒，并将最终 jsdom workers 限为 2，不修改产品 deadline 或通过条件。
- 另外四个失败来自既有 `BrowserConsoleApp.draft.test.tsx` / `BrowserConsoleApp.model-mapping.test.tsx`：`coalesced-provider` 文本匹配、多个“编辑”按钮、两项旧 `Mapping Pool` 名称匹配。临时撤回本批自己的 Qwen Browser 接线后，`ui-existing-without-wiring.log` 仍为相同 **4 fail / 7 pass**。这是当前脏工作树中既有用例的对照证据，不是 clean HEAD 全仓 baseline；未修改这些测试或无关生产逻辑来使其变绿。
- 最终 `ui-regression-final.log`：Qwen 父级 helper/组件/正常 Browser 自动保存、Key staging、目录 ownership、账号编辑/模板和 site-health 相邻回归 **11 文件 / 79 pass**；不把上面的四项已知失败从记录中抹除，也不称全前端回归通过。此前局部 pass 不重复累加。
- `management-v1.log` 首次 Rust 编译在新测试读取私有 `snapshot.document()` 上报 `E0624`，未开始测试，不计行为红测。改为检查真实管理响应的公开 document，并保留 compiled payload 检查后继续。
- `management-final-v2.log`：**8 pass**。新增真实管理 PUT 1 项验证无 Secret Grant 的 keep 保存、旧受众固定、显式 override、原 Key/disabled/catalog/fetched/路由优先级及旧 revision CAS 拒绝；既有 Qwen 管理 2 项和秘密受众 5 项通过，继续证明真正换目标时 keep 拒绝、replace 需授权。
- `typecheck-final-v2.log` exit 0；`web-build-final.log` / `web-build-receipt.json` 为隔离生产 Web bundle，**14 文件、published=false、cleanDistPath=false**。移除 live publish 插件，输出仅在本批新目录，未覆盖 P14 证据或工作区 live Web。
- `checker-final.log` **31/31**；最终 ratchet 扫描 3987 文件、14 immutable、governed source >700=0，官方 Rustfmt check 和全工作树 `git diff --check` 通过。ratchet 使用相同 checker 的无报告写入入口，未覆盖原有脏的有效行报告。

### 4.2 证明边界

Browser 集成调用合成 ConsoleApi；Rust 使用真实管理 router/SecretPatch/CAS 与合成内存状态。这两组分别证明各自接线，没有把它们包装成同一次真实浏览器→后端→Qwen 的 E2E。
本批没有实际 Qwen 登录/刷新/生成/额度请求、历史凭据读取、真实浏览器手测、EXE 包、隔离 EXE 启动、正式发布安全门禁或全 feature 矩阵。
所有本批 Cargo/exec sessions 已实际结束；收尾复查没有残留 Gateway Cargo，不把观察超时当完成。
P13 的极端 expiry 已知用例本批未运行或修复。P13 发布副作用与授权遗漏仍保留在其原回执，本批不抹除历史或授权后续 live 清理。

## 5. 有效行与来源

共 8 个源码，最大 491；无 baseline、例外、锁文件或 immutable policy 变更。

| 文件 | 前 → 后有效行 |
| --- | ---: |
| `BrowserConsoleApp.tsx` | 488 → 491 |
| `useProviderCatalogEditor.ts` | 28 → 36 |
| 新 `qwenProviderEndpoint.ts` / 对应测试 | 32 / 60 |
| 新 `QwenProviderEndpointSettings.tsx` / 对应测试 | 58 / 56 |
| 新 `BrowserConsoleApp.qwen-target.test.tsx` | 71 |
| 新 `tests/qwen_parent_endpoint_management_contract.rs` | 98 |

前端文件均位于 `apps/desktop/src/features/console/`。`source-before.json` 记录两个既有 owner 的修改前 hash/行数；新增文件按 0 计，不把既有脏改当本批新增。
最终使用 `source-frozen-v2.json` / `source-verify-final.log`；v1 保留为修正 Rust 测试私有 API 之前的中间快照，不声称 v1 是最终来源。
P11/P12/P14 frozen source 全部重新核对匹配；P14 曾有意变更的 P13 owner 不伪称仍等于 P13 初始 hash。所有本批源码/文档严格 UTF-8 无 BOM，未跟踪文件独立检查 whitespace，Markdown 围栏及本地链接检查通过。

## 6. 下一最小切片与未完成项

普通 Qwen 精确 probe 的只读调查发现两个真实缺口，**本批没有修复**：

1. `src/provider_runtime/console_model_probe.rs` 的普通 `openai_compatible` 显式请求直接调用通用 request planner 并 `request.send()`，没有沿 P11 的正式 Qwen prepare/planner/sender；空 Key 等边界与正常发送不一致，测试计划也继承此链。错误文案还残留 NVIDIA。
2. 无 `ConsoleProbeRequest` 时进入通用 connectivity probe，尝试 compatible `/models`；不能把它当已证实的普通 Qwen native 目录合同或生成成功。

下一批应在原精确账号/测试计划 owner 中接通普通产品的正式发送边界，明确无 request 的支持策略；覆盖空 Key 不触网、disabled 不触网、精确 credential override/Key、Chat-only、成功/错误脱敏、单点/计划一致。
复用 `console_nvidia_model_probe_contract.rs` 的管理/loopback 结构和正式 Qwen sender 的私有测试 seam，但**不能为测试把 workspace validator 放宽到 loopback host**。

随后仍需普通 preset 完整 pipeline buffered/stream/响应/取消闭环及正常管理/UI 的合成集成；不因父级编辑完成把 W05 标 `offline_ready` 或标蓝。
Bedrock 参数/reasoning/signature/取消 audit、其余 R01–R16 的 OAuth/Web/API/媒体、P50 devtest、集中录制与真实逐渠道验收继续追踪，总 goal 保持 active。

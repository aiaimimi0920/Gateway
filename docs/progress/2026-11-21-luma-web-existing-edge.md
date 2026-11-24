# L2 Web：复用本机已登录 Edge，读取现有 Board（2026-11-21）

## 实际操作与来源

通过 Windows Computer Use 接入用户已经打开的普通 Microsoft Edge 窗口。最初窗口最小化，恢复后确认地址栏为 `https://app.lumalabs.ai`，登录后的 `Boards`、`Assets` 等入口可用。没有新建浏览器会话，没有要求用户重新认证。通过现有 `Boards` 入口打开一个已有 Board，随后打开其 `Assets` 和三类创作选项菜单；未选择模型、填写提示词、生成、归档或恢复任何项。

Network 证据来自该标签页中通过 Edge 普通菜单打开的内置 DevTools。观察正常导航、刷新和资产侧栏触发的请求；未主动调用 App 接口、执行网页脚本、读取浏览器 profile 或导出 HAR。下表只记录当场脱敏后确认的请求元数据，不含真实账号、Board、realm、资产标识或参数值。占位符类型按路径位置标注，不据此认定不同路径标识相等。

| 页面动作 | 当场观察到的请求 | 确认范围 |
| --- | --- | --- |
| 打开 `Boards` | `/team/{team}/boards` | 路由和 App 页面可见；未读出方法、状态或正文 |
| 打开或正常刷新已有 Board | `/board/{board}` | 观察到同源条目类型 `fetch`、HTTP `200`；未读出方法或结构化业务正文 |
| 打开 Board 的 `Assets` | `/api/vespa/teams/{team}`、`/api/vespa/realms/{realm}/artifacts`（可见查询参数名 `limit`） | 同一操作后 Network 列表出现上述路径，资产侧栏显示已有 `Image`、`Video`、`Audio` 卡片；未核对对应行的方法、状态及 JSON 字段 |

## 现有菜单

- `Create Image` 可见 `Uni-1.1`、`Nano Banana 2.1`、`GPT Image 2.5 Flare`、`GPT Image 2.5 Sunburst`、`Ideogram 4.6` 等条目；`Seedream 5.5` 显示 `Upgrade`。
- `Create Video` 可见 `Ray3.2`、`Ray3.14`、`Ray3.14 HDR`；`Veo 3.1`、`Gemini Omni 1.2 Flash` 等显示 `Upgrade`。
- `Create Audio` 可见 `Speech`、`Music`、`Sound Effects`。仅记录显示项，不推断服务商、动作 ID 或账号额度。

## 续查：已保存 Network 请求与响应

仍使用同一已登录的普通 Edge；没有新建浏览器会话、重新认证或清空录制。把 DevTools 独立停靠后，验证筛选框确实选中了旧输入，再输入筛选词；通过请求表的 `Method` 列确认方法，通过 `Preview` 确认抽样正文结构。将 Network 概述恢复到完整时间范围，避免时间窗口把已有请求隐藏。以下 `{realm}` 均为同一已核对的 realm；只留字段名、类型、静态枚举和菜单标识，不保存私有正文、认证头、真实资源 ID、签名值或 HAR。

| 已录制的同源请求 | 观测结果 | 可证边界 |
| --- | --- | --- |
| `GET /api/vespa/realms/{realm}/menus.json?v=1` | `200`、XHR，DevTools 显示资源大小约 109 kB。根是对象：`version: string`、`menus: object`、`announcements: object` | `menus.toolbar_image.items`、`toolbar_video.items` 是动作数组；`toolbar_audio.items` 按 Speech、Music、Sound Effects 分成 `submenu`，再通过 `items` 放实际动作。动作项的 `type: "action"`、`id: string`、`label: string` 已确认。这是菜单 ID，不是选择、授权或 POST 受理证明 |
| `GET /api/vespa/realms/{realm}/signature` | `200`、XHR | 核对路径、方法、状态；未确认签名字段或有效期，未提取签名值 |
| `GET /events/api/vespa/realms/{realm}/events/stream/grip` | `200`、fetch | 核对路径、方法、状态；未取得可归属原任务的事件帧，也未单独核对媒体类型响应头 |
| `GET /api/vespa/realms/{realm}/artifacts?limit=…` | `200`、XHR，顶部为数组 | 样本 `type: "image/png"`、`source: "task"`、`state: "completed"`；`id`、`realm_id`、`object_ref` 为字符串，`object_url: null`。样本的 `realm_id` 与列表请求的 realm 一致，未取得原创作任务 ID 或签名媒体链接 |
| `GET /api/vespa/realms/{realm}/agent-options` | `200`、XHR，顶部为数组 | 样本对象含 `id: string`、`label: string`、`is_default: boolean`；它不是模型动作目录 |
| `GET /api/vespa/teams/{team}` | `200` | 核对路径、方法及状态；不据此推断成员或额度 |

`artifacts` 筛选另命中 `POST /api/vespa/realms/{realm}/artifacts/abandon-stale-uploads`（`200`）；末段是固定操作名，不是 `{artifact}`，不能当作单项详情或创作提交。本轮未重发任何已录制请求。用上述已知路径作阳性对照筛选时，未在当前录制窗口发现 `/actions`，不能据此断言接口已移除。没有用已完成资产猜测 `output_artifacts`，也没有把列表接入 worker 作为结果回退。

实测菜单映射节选（只证明页面返回该条目）：

| 分类 | 显示名 | `id` |
| --- | --- | --- |
| Image | Uni-1.1 | `create_image_uni_1` |
| Image | Nano Banana 2.1 | `create_image_nano_banana_2_1` |
| Image | GPT Image 2.5 Flare／Sunburst | `create_image_gpt_image_2_5_flare`／`create_image_gpt_image_2_5_sunburst` |
| Video | Ray3.2 | `create_video_ray3_2_via_api` |
| Video | Ray3.14／Ray3.14 HDR | `create_video_ray3_14`／`create_video_ray3_14_hdr` |
| Audio · Speech | ElevenLabs v4／Seed Audio 1.0 | `text_to_speech_eleven_labs`／`text_to_speech_seed_audio` |
| Audio · Music | ElevenLabs Music v1 | `text_to_music_elevenlabs_v1` |
| Audio · Sound Effects | ElevenLabs SFX v2 | `text_to_sfx_elevenlabs_v2` |

现有 worker 的三个默认 ID 在本次菜单均存在；无需改默认映射。以脱敏的嵌套菜单形状补了 4 个本地 worker 回归（图、视频默认、视频指定 Ray3.2、音频跨子菜单选择 Music），验证 `GET …/menus.json?v=1`、单次合成 POST 的动作 ID 和事件流释放。`node --test scripts/tests/lumalabs-browser-operation.test.mjs scripts/tests/lumalabs-task-contract.test.mjs scripts/tests/lumalabs-browser-operation-sse.test.mjs`：**29 passed**。这不证明官网接受合成参数或能完成创作。

## 结论与未验证边界

本次证明**普通 Edge 登录态可用、菜单和三个默认菜单 ID 与现有 worker 一致，资产列表含已完成的任务来源产物**。未取得创作 POST 的当前请求体和回执、原创作任务到资产的绑定、会话材料生命周期、SSE 完成字段或签名媒体合同。不能把菜单 ID 当作 POST 受理证明、把列表资产当作单任务回执，或用 MCP／Agents REST 合同猜测 App Web 字段。继续只读查找已有历史，不发生成补样本。

仅更新证据和状态文档，未改 Gateway 生产逻辑、运行实例或凭据，未生成或扣费。

## 再续查：签名结构、SSE 响应类型、Board 资产回读

重新绑定本机原有 Edge 和对应 `app.lumalabs.ai` 的 DevTools，保留已有录制，无需重新登录。通过同一浏览器中的站内链接打开团队 `Boards` 和一个已存在的 Board，侧栏可见历史 `Image`、`Video` 和 `Audio` 卡片。仅检查已有记录及正常导航产生的请求；未选用创作按钮、拖入资产、主动调用接口或重放请求。

| 来源 | 本轮新确认 | 仍不能证明 |
| --- | --- | --- |
| `GET /api/vespa/realms/{realm}/signature`，`200`，Preview | JSON 对象包含 `cdn_url`、`query_params`、`ws_token`、`ws_url`、`expires_at`，均为字符串。现有 worker 使用的前三项在样本中非空；`expires_at` 是可解析且抽查时尚未到期的带时区时间戳 | 不记录值，不推断固定 TTL、刷新策略、`ws_url` 与 SSE 的替换关系，未独立验证签名 URL 可取回原任务媒体 |
| `GET /events/api/vespa/realms/{realm}/events/stream/grip`，`200` | 在 **Response Headers** 区段确认 `Content-Type` 的媒体类型为 `text/event-stream`，不是由请求端 `Accept` 推断 | 没有读到可归属某一创作任务的完成帧，未验证 worker 所依赖的 `artifact` 包装和输出 ID 关联 |
| 已有 Board 导航录制中的 `POST /api/vespa/realms/{realm}/artifacts/bulk/read/{suffix}`，`200` | Preview 顶层对象含 `found_artifacts: array`、`missing_artifact_ids: array`、`tombstones: array`。`found_artifacts` 非空；首个可见条目有字符串 `id` 和 `realm_id`，后者和请求所属 realm 相同。抽样中的另两个数组均为空 | 回读既有资产不等于接受创作；没有核实请求体、后缀语义、原创作回执，也未取得同任务的完成事件及最终媒体关联 |

用包含 `grip` 和 `artifacts` 的组合筛选重新核对追加录制，阳性对照均命中；同源匹配项只有 `grip`、资产列表、固定的 `abandon-stale-uploads` 和上述批量读取，没有 `/actions`、`/tasks` 或 `/generations` 样本。不能据此认定这些路由不存在。自然导航带来的后台 POST 不当作创作回执。

现有 worker 的签名字段及 SSE 媒体类型检查与本次实测一致。保留“提交回执给出唯一原输出 ID → SSE 的完成资产匹配该 ID → 签名媒体”成功条件，不以历史资产批量读取回退。不改生产逻辑、测试、运行实例或凭据，不生成、不扣费；L2 创作核心链仍未验收。

## EventStream 与同源 POST 复查

继续使用原普通 Edge 的 `app.lumalabs.ai` DevTools 和仍保留的录制；只切换录制详情和筛选。`url:/events/stream/grip` 命中 6 条同源 `GET 200`。逐条打开 EventStream，六条连接各取一次消息表的可读快照，按当时列表顺序分别读到 20、16、57、6、44、3 条消息，共 146 条，类型均为 `ping`；抽查的 `ID` 和 `Data` 列为空。部分消息表首次打开后才填充，初始空表未当作无消息。结果只覆盖 DevTools 可访问性树在各次快照提供的消息，不宣称完整流没有其他事件。检查结束恢复 `grip` 筛选时，浏览器现有录制已增至 7 条同源 `GET 200`；新增的一条未计入上述帧样本。

再用 `domain:app.lumalabs.ai method:POST` 检查已保留的同源录制，不只查 `/actions` 等已知关键词。可见请求包括 Board／团队页面、遥测 `ingest`、画布健康、realm 元数据、workspace skills、entity 类型／schema、团队树列表及资产读回／清理。排除对应 URL 类别（含 `/board/`、`/team/` 和 `/artifacts/` 等广义路径）后，DevTools 显示 **0 个剩余匹配**；这只是路径级覆盖，未逐个验证这些类别下请求的业务语义，也不覆盖跨域请求或其他方法。单独筛 `/board/` 得 31 条 POST；用 MIME 筛选，`text/x-component` 仍匹配 31 条、`application/json` 匹配 0 条。这个对照可区分响应类型，不能据此把 Board POST 判成创作受理或断言它们都无副作用。

此次没有找到可归属于同一创作的提交回执、非心跳完成帧和最终媒体，因此 L2 核心链仍未验收。未刷新页面、重放 POST、发起生成或扣费；不据此更改 worker 合同。若无保留了原创作全过程的录制，获得新样本需另行授权真实生成，不从历史资产推导任务绑定。

## 一次真实提交：上游拒绝，credits 不足

在同一已登录的普通 Edge、原 Board 和持续录制的 Network 中，按用户对单次 30 credits 的明确确认，选 `Uni-1.1`，用无个人信息的简单图片提示词提交一次。实际点击前的同源 `/actions` POST 列表只有两条 `/actions/preflight`（均为 `200`），没有创作 POST；点击后新增且仅新增一条创作 POST。未刷新、重放、切换账号、升级或充值。

| 录制位置 | 脱敏事实 |
| --- | --- |
| 预检 | `POST /api/vespa/realms/{realm}/actions/preflight` 返回 `200`。预检成功不是创作受理，也不代表余额足够。未核实其 JSON 合同，不把预检加入 Gateway 请求链。 |
| 唯一创作请求 | `POST /api/vespa/realms/{realm}/actions`，XHR。Payload 根对象含 `type: "create_image_uni_1"`、`fields: object`、`entry_point: "chat_panel"`、`optimistic_output_ids: string[]`（一个非空 8 字符串）。`fields` 含 `prompt`、`aspect_ratio`、`seed`、`style`、`output_format`，均为字符串；此样本为 `16:9`、空种子、`auto`、`png`。未保存提示词、真实资源 ID、签名、认证头或 HAR。 |
| 唯一提交的响应 | `HTTP 402`，JSON Preview 包含 `Not enough credits`。未见成功受理回执或可绑定的输出 ID，未验到完成事件或最终媒体。30 是界面报价，未核对实际扣费账单，不记为已扣 30 或已验证零扣费。 |

这确认了 App 当前单图提交路径、发出的请求形状和额度拒绝，**不证明** `entry_point`、空 `seed` 或预检是成功提交的强制条件，也不把客户端的乐观 ID 当作上游受理回执。Gateway 保留原提交、SSE 和媒体合同；只有创作 POST 返回 `402` 时改用固定可操作的 `lumalabs_action_payment_required`，不读取上游失败正文，不重试，仍释放原 SSE。新增合成回归在变更前因通用错误失败，变更后通过；真实 402 来自官网正常控件，不是远端 worker 或 Gateway 端到端通过。L2 核心链仍需在有足够额度的团队内、另行确认一次新提交后验收。

验证：45 项 Luma Node 回归与 `node --check` 通过；有效代码行检查器 31 项用例、仓库 ratchet 均通过。未打包、部署或修改运行实例。

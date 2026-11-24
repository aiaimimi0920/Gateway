# Luma Agents 视频重绘

日期：2026-11-17。范围是独立 Agents REST 的 `video_edit`，不包含 App Web、MCP 创作或整个 Luma 交付。本批源码、公共／管理本地合成链、UI 及定向回归通过；现有 4400 包尚未包含这些变更。

## 官方合同与实现

本轮匿名 GET 官方 [Video editing](https://docs.agents.lumalabs.ai/guides/videos/editing/)，HTTP 200，未登录或执行远端脚本。公开 HTML、正文、时间及 SHA-256 存于 `../evidence/luma/gateway-luma-video-edit-20261117/sources.json` 及同目录文件。搜索工具无法打开该页后使用明确标识公开合同调查用途的 User-Agent 获取，没有调用真实生成或读取凭据。

- 公共入口仍为 `POST /v1/videos/generations`，显式 `type: "video_edit"` 才进入新映射；仅接受解析后的 `ray-3.2`，保留别名和精确账号路由。
- 单个视频 source 支持同 client 的 completed generation ID、File ID、HTTPS URL 或 inline Base64。URL/inline 必须带 `video/*` MIME；ID 不携带 MIME。不新增上传 Files，也不在网关本地获取输入 URL。
- `video.edit` 支持自动控制和九档强度：`adhere_1..3`、`flex_1..3`、`reimagine_1..3`。支持 pose／depth／normals／trajectory／face 五类 conditioning，校验布尔、枚举及 0–1 数值范围；显式 controls 不能与 `auto_controls:true` 混用。
- Guide 可用单个 `video.start_frame`，或 `video.edit.keyframes`／`keyframe_indexes` 配对数组，1–64 项、等长、位置为非负唯一整数；两种形式互斥，实际源帧范围由上游验证。
- 分辨率为 360p／540p／720p／1080p；HDR 仅接受 720p／1080p 或省略分辨率并使用供应商默认。输出比例由源视频决定，拒绝被忽略的显式比例、loop、end_frame、create keyframes、source_position 等错误操作参数。
- 本地 profile 暂不开放 EXR 与 `video.duration`，请求明确失败，不静默丢弃。官方页面介绍提及 duration，但重绘 validation 表没有明确列其控制合同；没有直接照搬视频生成的时长组合。
- 复用原一次 POST／同 UUID 轮询、无凭据资产验证、成功与失败的实际 credits 审计以及本地结算。不新增后台任务、数据库、存储版本或迁移。

公共请求示例（结构示例，不代表已调用）：

```json
{
  "model": "ray-3.2",
  "type": "video_edit",
  "prompt": "将原片重绘为月夜胶片风格",
  "source": { "generation_id": "d290f1ee-6c54-4b01-90e6-d701748f0851" },
  "video": { "resolution": "720p", "edit": { "strength": "flex_2" } }
}
```

## 正常账号界面

现有媒体测试弹窗增加“视频重绘”，选择源视频、分辨率及“自动”或九档重绘强度。自动仅提交 `auto_controls:true`，手动提交 strength；不混入比例／时长。切换操作清空旧素材引用；仅显示精确账号已配置的 `ray-3.2`。草稿保存、逐次收费确认、原 Secret Grant 和取消行为保留。

UI 当前不提供细粒度 conditioning、guide frames 或 HDR 表单；这些是 API 能力，不将后端支持写成已有全部界面。

## 验证结果

证据目录：`../evidence/luma/gateway-luma-video-edit-20261117`。

- UI 红测：新增两项流程均以 `Value "video_edit" not found in options` 失败，见 `ui-red.log`；实现后 `LumaAgentsMediaProbe.test.tsx` **13 passed / 0 failed**，见 `ui-green.log`。TypeScript `npm run typecheck` 通过。
- 六项新增协议测试与四项新增业务测试全部通过，见 `rust-edit.log`。公共成功／失败保持精确账号和原任务事实、分别扣一次普通 Key／退款；正常管理 router + Secret Grant 的成功／失败保留审计，不扣普通 Key。非法请求零网络、零扣次；上游错回其他操作类型时不下载、不重试、不采纳费用 receipt。
- 复用本次 test EXE 跑定向回归：`upstream::luma_agents::` **59 passed / 0 failed / 1 ignored**；`protocol::luma_agents::` **18 passed / 0 failed**，见 `rust-regression.log`。包含上面的十项新增测试，不重复累计；唯一 ignored 是既有显式 browser-host 专项。
- Suno、Udio、Luma Web、ChatAIBot 各一项媒体准入回归通过，见 `adjacent-admission.log`；合计 **81 个不同 Rust 测试通过**。相邻测试不等于分别动态测试新增枚举；只读审查确认各产品使用正向操作白名单，拒绝 VideoEdit。
- 生产 `cargo check --locked --offline --lib -j 4` 通过，耗时 1m41s，见 `production-check.log`。保留既有 warnings，不做无关依赖升级或自动修复。
- Edge 以真实组件及原 CSS、合成账号和禁止上游提交的回调检查 1280×900／420×850：收费确认、Escape 返回、内部滚动和错误显示正常；窄屏 document 宽 420、dialog 客户／滚动宽均 386，无横向溢出，控制台零错误／警告。见 `video-edit-wide.png`、`video-edit-narrow.png`、`video-edit-narrow-bottom.png` 与 `.playwright-cli`。这是组件浏览器验证，不是完整 main→后端联合验收。
- 已用独立可见 Edge 窗口打开 `http://127.0.0.1:4443/` 并选中视频重绘。复用前一批两小时自动退出的临时服务，网页 title 尚为“视频扩画”，正文及操作实际是重绘；没有复用真实浏览器 profile，也没有提交服务商请求。
- checker **31 项通过**，effective-line ratchet 通过：5787 个文件、14 项 immutable runtime artifacts 单列，governed source 无超过 700 行。见 `scripts/artifacts/luma-video-edit-effective-20261117.json`。request mapper 221→223、probe request 162→167、dialog 85→93、UI tests 132→161 有效行；新增 edit mapper／协议测试／业务测试分别为 176／166／227 行。本轮直接改动源码均小于 250 行，前后计数与哈希见证据目录的 `affected-before.json`／`affected-after.json`。
- 受影响 Rust 的 rustfmt、`git diff --check` 和本轮文本 UTF-8 无 BOM 检查通过。

## 尚未验收与下一步

源视频实际内容、≤18s、同 client 归属及源帧范围由供应商验证，本地参数通过不证明这些事实。API inline 沿原 8 MiB Base64 字符串和 16 MiB 聚合序列化限制，UI inline 上限 180000 字符；不宣称覆盖官方全部 200 MB 容量或所有编解码器。

本轮未执行真实生成／扣费、视频解码与播放、Server／PG、原生包或完整浏览器→后端联合验收。取消期间尚未上交 receipt、截止后／跨进程追账、USD/cash 延续前批边界。没有重打 EXE、提交／推送、重启 4200／4400 或导入真实数据。

后续保留 layering、Files、LLM 等其他操作及重绘未开放参数的范围确认；整项供应商计数不因本子批通过而减少，不把 Agents REST Key 成功当成 App／MCP 创作已完成。

# Luma Agents 视频扩画

日期：2026-11-17。范围是独立 Agents REST 的 `video_reframe`，不包含 `video_edit`、App Web 或 MCP 创作。本批源码、公共与管理本地合成链、UI 及定向回归通过，未标记整项 Luma 完成。

## 合同与实现

本轮重新匿名 GET 官方 [Video reframing](https://docs.agents.lumalabs.ai/guides/videos/reframing/)，HTTP 200。公开 HTML、正文提取、时间及 SHA-256 存于 `../evidence/luma/gateway-luma-video-operations-20261117/sources.json` 及同目录文件。搜索工具无法打开该页后使用明确标识合同调查用途的 User-Agent 获取；没有登录、执行远端脚本或调用生成。

- 复用 `POST /v1/videos/generations`，仅显式 `type: "video_reframe"` 进入新映射；只接受解析后的 `ray-3.2`，保留别名及精确账号路由。
- 接受单个源视频的 Generation ID、File ID、HTTPS URL 或 inline Base64。URL/inline 必须带 `video/*` MIME，ID 不带 MIME；来源与类型不可混用。未新增上传 Files 的功能，也不在网关本地获取输入 URL。
- 目标比例必填：9:16、3:4、1:1、4:3、16:9、21:9；时长为 5s/10s，分辨率为 360p/540p/720p/1080p。拒绝竖向比例的 1080p、HDR/EXR、loop、edit、图像 anchors 与 keyframes。
- API 可选 `video.source_position`，四项归一化坐标完整提交；x/y 在 [-2,2]，w/h 在 (0,2]，允许官方规定的画布外部分。不把合法负坐标误判为越界。
- 复用原视频 sender、一次 POST/同 UUID GET、无凭据输出资产验证、成功和失败费用审计及本地次数结算，不新增数据库或后台任务。
- 正常账号测试弹窗增加“视频扩画”，输入源视频、目标比例、时长和分辨率；仍要求精确单账号、匹配的已配置视频模型、已保存路由及逐次费用确认。UI 使用默认居中位置，自定义 source_position 当前通过 API 提供。切换操作清空旧素材引用，避免误用源图片。

公共请求示例（只展示结构，不代表已执行）：

```json
{
  "model": "ray-3.2",
  "type": "video_reframe",
  "prompt": "将场景向左右扩展为宽屏",
  "source": { "generation_id": "d290f1ee-6c54-4b01-90e6-d701748f0851" },
  "aspect_ratio": "21:9",
  "video": { "duration": "5s", "resolution": "720p" }
}
```

## 验证

证据目录：`../evidence/luma/gateway-luma-video-operations-20261117`。

- UI 红测：新增两项操作流程均以 `Value "video_reframe" not found in options` 失败，确认入口缺失，而非环境失败。
- UI 绿测：`LumaAgentsMediaProbe.test.tsx` 11 passed / 0 failed，见 `ui-tests.log`；含三项新增回归。TypeScript `npm run typecheck` 通过。
- Edge 中加载真实 `LumaMediaProbeDialog`、原主题及 CSS，使用独立合成 document 和禁止上游提交的回调。宽屏 1280×900、窄屏 420×850、内部滚动、逐次费用确认及 Escape 返回已检查，控制台零错误/警告。见 `reframe-wide.png`、`reframe-narrow.png`、`reframe-narrow-bottom.png` 及 `.playwright-cli` 记录。这是组件浏览器验证，不是整个管理 main 与后端的浏览器联合验收。
- 本轮临时预览在 `127.0.0.1:4443`，已用独立可见 Edge 窗口打开“视频扩画”；预览服务约两小时后自动退出。没有复用真实浏览器 profile，点击确认也不会生成作品。
- 新增五项 Rust 参数合同和三项公共/管理流程测试全部通过，见 `rust-reframe.log`。公共成功/失败均保留原任务和实际 credits，分别扣一次普通 Key/沿原退款；正常管理 HTTP probe 带原 Secret Grant，使用精确账号和视频模型、不扣普通 Key；非法参数不提交、不收费。
- 完整定向回归：`upstream::luma_agents::` 55 passed / 0 failed / 1 ignored，`protocol::luma_agents::` 12 passed / 0 failed，见 `rust-regression.log`；包含上面的八项新增测试，不重复累计。唯一 ignored 是既有专用 browser-host 测试，本批未运行它。
- Suno、Udio、Luma Web、Chataibot 各一项既有媒体准入回归通过，见 `adjacent-admission.log`；与 Luma 两组合计 71 个不同 Rust 测试。它们证明原准入未回归，并不等于分别动态测试了新增枚举；只读审查确认各自正向操作白名单拒绝 VideoReframe。
- 生产 `cargo check --locked --offline --lib -j 4` 通过，耗时 2m11s，见 `production-check.log`；既有 warnings 保留，没有自动修复或依赖升级。
- checker 31 项及 effective-line ratchet 通过，5784 个文件、14 项 immutable runtime artifacts 单列，governed source 无超过 700 行。
- 原 request mapper 212→221、media reference 96→121、probe request 157→162 有效行；新 reframe mapper 87、新协议测试 116、新公共/管理测试 183 行。所有本批直接改动源码均小于 250 行。受影响 Rust 的 rustfmt、`git diff --check` 及 UTF-8 无 BOM 检查通过。

## 尚未证明的范围

源视频实际内容、所有权及 ≤10s 由供应商验证；本地映射通过不等于这些事实已验证。API inline 延续 8 MiB Base64 字符串及 16 MiB 聚合序列化限制，UI inline 上限仍为 180000 字符；不宣称支持官方全部 200 MB inline 容量或所有视频编解码器。

没有真实上游生成、扣费、视频解码/播放、Server/PG、原生包或完整浏览器→后端联合验收。费用取消期间未上交 receipt、截止后/跨进程追账仍沿前批边界。没有重打 EXE、提交/推送或改动 4200/4400 的程序、数据库与凭据；现有 4400 包不包含本批代码。其他官方操作、MCP 创作及剩余服务商计数不因此关闭。

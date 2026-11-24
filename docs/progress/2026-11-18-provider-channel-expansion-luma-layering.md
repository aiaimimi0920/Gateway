# Luma Agents 图片分层

日期：2026-11-18（Asia/Shanghai），2026-11-19 补齐 Rust 验证。范围是独立 Agents REST 的 `layering`，不包含 App/MCP 创作、图片编辑器或自动合成导出。源码、UI 和相关离线回归已通过，不据此标记整个 Luma 完成。

## 官方来源与本地合同

本轮联网核对官方 [Image layering](https://docs.agents.lumalabs.ai/guides/images/layering/) 与 [Create a generation](https://docs.agents.lumalabs.ai/api/resources/generations/methods/create/)。两页匿名 GET 返回 HTTP 200，原始 HTML、正文、抓取时间与 SHA-256 在 `../evidence/luma/gateway-luma-layering-20261118/sources.json`。使用明确标识公开合同调查的 User-Agent，未登录、执行远端脚本或调用生成。单独 Get 文档的原文抓取因传输断开失败，本批未把它计作抓取成功；响应合同由前述 Create schema 与 guide 的完整正文核对。

- 沿用公共 `POST /v1/images/generations`，显式 `type: "layering"`；Gateway 路由仍要求配置并选择模型／别名，仅解析后的 `uni-1` 可用，不能自动换成 `uni-1-max`。
- 要求单个 source，复用 HTTPS URL、inline Base64+图像 MIME、File ID 或同 client Generation ID。不在网关获取输入 URL，不增加 Files 上传。
- prompt 字段必须存在，但可为空，最长 500 Unicode 字符。公共显式 layering 和管理显式 layering 具有该例外，普通图片及其他媒体的非空 prompt 规则不变；默认／命名／自动文本测试不转成分层。
- `layering.resolution` 为 1k／2k，省略交给上游默认。未知 options、image_ref、web_search、video 及本地不提供的 aspect_ratio/style/output_format 等控制明确拒绝，不静默忽略。
- 一次 POST／同 UUID poll，completed 后才读取图层。分层内部时间上限为 600s，仍取原调用预算的较小值；没有延长外层路由、管理接口或浏览器等待预算。processing 不因图片已准备好就被当成完成；没有为文本迟到另开后台工作或自动重提生成。

示例（仅结构，不代表已执行）：

```json
{
  "model": "uni-1",
  "type": "layering",
  "prompt": "",
  "source": { "url": "https://images.example.test/source.png" },
  "layering": { "resolution": "1k" },
  "response_format": "b64_json"
}
```

## 图层返回与资产验证

完成时接收 1–10 个有序图层，先校验整个 manifest，再下载资产。每项仍在 `data[]`，提供 URL 或 b64_json／mime_type，并附带白名单 `layer`：index、label、description、alpha_hint、可空 bounds、contains_text 和 text/font（含可选字体目录匹配）。未报告的可选值保持未知，不造位置、OCR 或字体；未知额外字段不向调用方或审计透传。

要求 index 与数组前后顺序一致，最后一层为不透明背景；已报告 bounds 须为可放置在共同画布内的像素矩形，并与 PNG 实际尺寸一致，背景矩形须覆盖画布。bounds 缺失/null 是官方允许的未知，不猜偏移，也不提供自动复原功能。

每张资产复用无凭据公网 HTTPS、有界下载及共享 PNG/JPEG 完整解码所有者。分层进一步仅接受 RGBA PNG，校验原生 8/16 位 alpha 的背景不透明性；不重复解码。原共享解码信号量、内存／尺寸上限和取消语义保留，单图解码分配最多 64 MiB、尺寸最多 8192、整次输出下载合计 64 MiB。CPU 解码不能硬中断，原 permit 随 bounded blocking work 持有，不创建新持久后台任务。

任何图层失败都不返回部分 data、不重提 POST。普通 Key 成功扣一次而非按图层扣次，失败走原本地退款；供应商真实费用沿原任务 credits 审计，不根据分层数或 guide 的临时价估算。不把本地失败／退款当成上游退费证明。

## 正常账号界面

媒体测试增加“图片分层”、可选短指引、源图和 1k／2k 分辨率。只选精确账号已配置的 uni-1，切换操作清空旧素材，每次运行仍经收费确认。正常草稿状态、Secret Grant 和精确账号准入不变；不保存费用授权，不开放免认证入口。

临时预览 `http://127.0.0.1:4443/` 使用真实组件／主题和合成账号，回调只显示“未向服务商提交请求”。已打开独立可见 Edge 窗口并选中图片分层，服务两小时自动关闭；这不是完整管理 main→后端联合验收。

## 验证

证据目录同上。

- UI 红测两项以 `Value "layering" not found in options` 失败，见 `ui-red.log`；绿测 15 passed / 0 failed，见 `ui-green.log`。类型检查在修正测试夹具的 unknown 类型访问后通过，见 `typecheck.log`。
- 宽屏 1280×900／窄屏 420×850 的表单、空指引、收费确认、Escape、错误显示和内部滚动已检查。document 宽 420、dialog 客户／滚动宽均 386，控制台零错误／警告；见 `layering-wide.png`、`layering-narrow.png` 和 `.playwright-cli`。
- 11 月 18 日 Rust 运行最终为 13 passed / 2 failed；`rust-layering.log` 保留失败现场。两项均为字体匹配的整数 weight 被 `f64` 往返成小数，导致 metadata 精确保留断言失败。11 月 19 日先复现旧测试 EXE 的失败，再改用 `serde_json::Number` 保留整数／小数表示，并通过 `as_f64()` 做既有 1–1000 范围校验；新增合法小数、上下界及非法值回归。layering 定向 16 passed；完整受影响 `luma_agents` 为 95 passed / 0 failed / 1 ignored（包含前述 16 项，不相加）。ignored 为需要独立浏览器宿主的专项。
- checker 31 项及 effective-line ratchet 通过，5794 个文件、14 项 immutable runtime artifacts 单列，governed source 无超过 700 行。最终源码计数／编码／格式检查待 Rust 终态后一并补记。

11 月 19 日补验：生产 lib `cargo check --locked --offline --lib --no-default-features --features runtime-defaults -j 4`、两项修改 Rust 文件的 rustfmt、checker 31 项与 ratchet 通过；`layering_text.rs` 有效行 55→57，`layering_tests.rs` 196→223。该阶段 ratchet 测量 5811 项、14 项 immutable runtime artifacts，governed source 无超过 700 行。关联视频 UI、合成浏览器及最终检查见 [续做回执](2026-11-19-provider-channel-expansion-luma-video-controls.md)。本次通过结果不覆盖原失败日志。

## 边界

没有真实生成、账号权益／扣费、源图归属或 1k/2k 画质验收。字节与 metadata 校验不证明拆分内容、OCR、字体匹配或复原视觉正确；未提供编辑器、PSD／SVG 导出、自动复原或字体下载。不新增数据库／存储版本，不改 App/MCP wire；既有取消中 receipt、截止后／跨进程追账等边界仍保留。

没有重打 EXE、提交／推送或重启／导入 4200/4400，当前 4400 程序不包含本批代码。Files、LLM 等其他操作及完整产品验收继续登记在剩余清单。

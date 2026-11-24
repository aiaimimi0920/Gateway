# Luma MCP 模型参数与编辑操作

本批延续 Local MCP 显式创作任务，补齐已取得模型详情对应的图片参数、参考图和视频编辑操作。没有调用供应商真实生成，没有修改现有账号、配置 revision 或运行中的 4200 实例。

## 改动范围

| 模型 | 本批新增能力 |
| --- | --- |
| UNI-1 | 图片编辑源 URL、最多 9 张生成参考图／8 张编辑参考图、图片比例、auto/manga 风格、PNG/JPEG |
| GPT Image 2 | 独立模型选择、最多 16 张参考图；1K/2K/4K、low/medium/high、PNG/JPEG/WebP、背景参数 |
| Ray 3.2 | 视频重绘、扩画；声明源视频 MIME；分辨率、时长、HDR、首尾帧、最多 64 个带位置关键帧；重绘强度／自动控制、扩画源位置 |

模型、操作和参数通过结构化字段选择，不接受任意 MCP 工具参数。第三方图片请求使用 `medias` 的 `reference` role 和 `parameters`；UNI-1 使用 `source/image_ref`；视频源使用明确的 `video/mp4`、`video/webm` 或 `video/quicktime`。图片 URL 不伪造 MIME，由供应商摄取时判定。

所有输入 URL 必须通过现有公网 HTTPS 边界检查；Gateway 不下载输入素材，也不为这些 URL 附加账号凭据。用户提供的源图／视频、参考 URL、提示词和具体参数只参与原请求摘要，不进入持久任务正文。任务和审计记录请求模型，仍不把请求模型当作已验证的实际产出模型。

准备任务不调用上游。完整选项纳入请求摘要，准备后修改模型、素材或参数不能提交；重复提交仍被拒绝。生成前的费用确认、固定账号／revision、独立 workspace/session、同 run 查询与取消／结束机制沿用原实现。可选 `options` 未提供时不参与序列化，旧版本已准备任务的摘要保持兼容；新增任务展示字段均有兼容默认值，不升级数据库版本。

模型／操作切换会清空旧选项；已准备任务冻结输入。修改输入会撤销尚未使用的确认。API 总请求仍受 32 KiB 上限约束。视频 keyframe 索引按 24fps 校验：5s 为 0–120，10s 为 0–240，拒绝重复位置及首尾帧与关键帧混用。已知无效组合在发送前拒绝，不静默删除参数。

## 验证与交付

Rust MCP 定向回归 **40 passed / 0 failed / 1 ignored**，正常管理路由和 SQLite 合成上游覆盖四种新增请求的出站参数、错误输入不创建会话／审计、摘要篡改、重复提交、关键帧边界、旧请求序列化兼容和请求模型审计；原生命周期、费用与恢复回归保持通过。首次本批编译 17m21s，测试运行 13.72s。

前端定向回归 **22 passed**，包含原任务单次提交、账号切换隔离、新模型参数、视频源和操作切换；TypeScript、正常 Web 构建、Rust formatter、有效行检查器 **31 passed** 及 ratchet 通过。新增生产模块最多 236 个有效行，没有新增例外或增长历史大文件。

独立 Edge 合成组件验收通过：GPT Image 2 配置／准备／费用确认／单次提交／查询结果，以及 Ray 3.2 扩画／取消／结束共七次操作；宽屏和 390px 窄屏无越界，零外网请求，浏览器和预览服务已关闭。持续脚本为 `tools/luma/mcp-creation-preview-check.mjs`，夹具位于 `apps/desktop/e2e/fixtures/luma-mcp-creation-preview.tsx`。它验证真实组件，管理路由由上述 Rust 集成测试验证，未把两份独立证据称为正常网页到管理 TCP 联合验收。

`cargo check --lib --no-default-features` 通过。本批桌面 **devtest** 包已交付：`Neuro/release/Gateway/gateway-devtest-20261120-luma-mcp-options/`，启动入口 `gateway-ui.exe`。正式发布流程未执行。

标准打包入口完成源码指纹、6129 项文件完整性与 EXE 哈希验证；后端 devtest 编译 21m49s、桌面壳 1m35s，总打包流程 1833.53 秒，不将它称为冷构建或宣称加速倍数。随后独立数据／51883 端口启动验收通过：桌面父子进程关系、SQLite ready、管理页 200、零服务商请求、收尾后零残留测试进程。原 4200 实例和已保存账号未改动。

后端 SHA-256：`6330896aa16cd8292865026fb5ca917d335126461d9aaefd78cea9b7d3f92a33`。桌面 SHA-256：`7dd015de9c66710ef4443ef632b077e58893a13ea083eab4d89790fd205407be`。包内凭据与 manifest 标记 `buildKind=devtest`。只读脚本与源码回执见[长期验证记录](../evidence/luma/gateway-luma-mcp-options-20261120/verification.json)，没有复制临时数据库或凭据。

首轮新增前端测试因“创作模型”控件尚未实现而失败，实施后通过。一次组合 PowerShell 检查的参数转义错误使其退出码不足以证明通过，已保留原日志并分别重新执行 formatter 和 `git diff --check`；不采纳该组合回执为成功证据。

## 尚未关闭的范围

本批不是 L1 全部完成：仅开放有本账号详情证据的三种模型，未宣称其余目录模型、MCP 分层布局、run 用户交互、workspace 内资产 ID 复用或 Server 持久身份已完成。EXR/PNG 导出、1440p、细粒度视频 conditioning 和编辑多关键帧未纳入当前支持范围，未知字段拒绝。MCP 资产仅交付校验后的链接，不宣称完成媒体下载、解码或播放。

L2 浏览器入口本次再次在初始化失败，原文 `Module not found: @oai/cua/tinyskyAlt`；未触及账号或读取当前登录 wire。L4 核心 SQLite/PG 追账证据复用，完整 Server/Redis 与操作系统级强杀未新增验证。L6 真实收费生成继续暂停。所有本批源码、测试和维护文档位于仓库；关键结果归档到 `docs/evidence/luma/gateway-luma-mcp-options-20261120/`，不依赖临时目录长期存在。

# Luma MCP 全目录模型接入

本批继续 L1 的既定模型范围，新增原三种模型之外的 15 种模型，合计 15 个图片与 3 个视频模型。没有将 MCP 创作范围扩大到通用工具代理、任意参数转发或其他渠道，也没有运行真实收费生成。

## 当前账号合同

正常管理 API 最初返回 HTTP 400、`luma_mcp_material_unavailable`／`luma_oauth_expired`。用户随后明确允许“一次刷新并保存”，本批通过已有 prepare/import/poll 接口执行了一次显式 OAuth 刷新；结果为 `succeeded`，原 credential ID 保持不变，新 revision 保存成功。没有重放刷新、不输出凭据或整个账号配置。

刷新后通过正常管理 API 读取 18 项模型列表及全部 18 项 image/video 描述，读取前后 revision 一致。合同及脱敏回执在[长期归档](../evidence/luma/gateway-luma-mcp-models-20261120/contracts/verification.json)。描述与列表分别约束模型参数、素材角色、比例和参考数量；不用模型名称猜测能力。工具完整 schema 继续复用同日已取得的 26 项工具合同。

## 改了哪些内容

- `manifests/luma-mcp-creation-models.json` 保存已核实的模型配置，Rust 与表单读取同一份；运行时 discovery 不会自动扩大写入准入。合同回归逐项比对已归档 list/describe。
- 图片新增 Seedream 三种、Nano Banana 四种、Ideogram 三种、GPT Image 1.5／2.5 Flare／2.5 Sunburst，保留 UNI-1 与 GPT Image 2。按模型开放分辨率、质量、格式、背景、尺寸、思考程度、搜索增强、温度和提示词增强；只发送用户实际选中的参数，保留显式 `false`／`0`。
- 第三方图片保持 `type=image` 与 `medias[].role=reference`；有序参考图可以来自公开 URL 或已保存任务素材，合并计数执行每个模型的真实上限，不伪装为原生 `image_edit`。Ideogram 带参考图片时拒绝显式 magic prompt，避免上游静默忽略。
- Seedream 5 Flash 支持 1K／1.5K／2K 或合法 `WIDTHxHEIGHT`；像素总量与比例执行当前说明约束。Lite 限 1K，Ideogram 4.6 最低质量与 4K 预设组合被拒绝，透明 JPEG 被拒绝。每个模型使用自己的比例列表，不沿用 UNI 的统一比例。
- 视频新增 Ray 3.14、Ray 3.14 HDR。HDR 型号显式固定 HDR、5s、720p／1080p；Ray 3.14 的 10s 限文本生成且不循环，不接收会转入 Ray 3.2 的多关键帧。Ray 3.14 的循环控制单独开放；原 Ray 3.2 支持范围保留。
- 所有模型沿用已有预算、逐次费用确认、任务摘要、单次提交、查询、取消／结束、资产和费用路径。新 optional 字段在为空时省略，保持先前已准备请求的摘要兼容。
- 新的长期合同采集脚本为 `tools/luma/capture-mcp-models.py`；只允许显式 loopback origin、无代理／重定向、响应与超时有界、只读 list/describe。错误输出仅允许固定本地标识，不打印供应商正文。浏览器模型流程复用 `mcp-creation-preview-check.mjs --models`，脚本和测试都在仓库。

## 验证

- 前端四个相关文件：**36 passed**；包含 18 个模型默认草稿的精确选择、模型参数切换、数量和组合限制、既有创作／素材复用回归。
- TypeScript 检查通过。
- 实际 Edge 运行真实组件加合成 client：Nano Banana、Seedream 尺寸、Ray HDR 三条 prepare/start/read 流程，共 9 次合成调用；逐次确认、请求一致性及宽窄屏通过。浏览器关闭，180 秒预览服务已退出。此项不代替管理 HTTP 或真实上游业务。
- 合同采集错误脱敏测试：**3 passed**。
- checker tests：**31 passed**；effective-line ratchet、Rust 格式、UTF-8 无 BOM 与 diff 检查通过。本批生产源码文件最多 209 行有效代码；原 `options.rs` 261 → 209，`video.rs` 171 → 195；新增 Rust 测试 277 行，单一模型准入职责。
- Rust MCP 创作定向测试：**20 passed / 0 failed / 6428 filtered out**。新增用例逐一通过正常管理路由验证 18 个模型准备、单次提交及查询，另验模型合同、参数映射、拒绝组合和历史摘要兼容；继承创作／素材复用回归同批通过。一次 Cargo 编译约 15m44s，实际测试 19.89s，总命令 965.02s。编译开始后仅对测试模块声明排序做了 rustfmt 修正，不改变测试或产品行为，未为这一格式差异重编。
- 完整回执与源码 hash 见 [verification.json](../evidence/luma/gateway-luma-mcp-models-20261120/verification.json)。无全量回归，无新原生包。

## 剩余边界

L1 的“其余目录模型”实现缺口由本批收口，但模型可调用范围仍以实际账号当时权限为准，供应商可能拒绝。这里不宣称所有高级视频控制、SVG 编辑导出或真实输出完成。

L1 仍缺分层布局、run 用户交互、Server 持久身份和配置变更后的任务恢复；L2 当前 Web wire/session 仍缺可用读取入口；L4／L6 的适用完整 Server／Redis、操作系统强杀及真实收费业务仍未验。仍为四个未完全关闭的工作包，不将本批 15 个新增模型换算为四个工作包的完成百分比。此前包未包含本批和素材复用增量，用户未要求重新出包。

新增参数只在已有 32 KiB 管理请求边界内处理，URL 继续经公共 HTTPS 校验，不由 Gateway 下载或附带账号凭据。共享模型配置为编译时已审核数据，外部响应不能覆盖。未新增后台任务、队列或重试机制；数据库和任务保存格式没有迁移。

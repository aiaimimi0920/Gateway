# R14 Luma Web 单账号图片任务测试

日期：2026-11-11 开始，2026-11-12（Asia/Shanghai）完成本批离线验证。
本批接通正常账号测试入口、管理 HTTP 和既有图片 sender；不证明供应商模型绑定或真实平台验收，不新增模板或整卡 `offline_ready`。

## 实现

- 后端将 `lumalabs_compatible` 注册为独立媒体测试产品，不借用 Luma Agents 的请求或审计。
- 只接受一个精确账号、已配置且该账号可调用的模型/别名、显式费用确认、prompt-only `image` 和空 parameters。固定 App audience、typed Luma Web special adapter、browser-backed 模式及 executor；发送前复用既有媒体计划预检。
- 默认/文本测试缺少显式媒体授权时零生成；不增加保存计划、自动生成、批量生成、编辑、视频或音乐。请求级 action、realm、数量、来源图片等覆盖不开放。
- 走正常 sender 的一次提交、原任务上下文校验和匿名公网图片读取/完整 PNG/JPEG 验证；不通过公共 Key 的路由结算，不重放或换账号。
- 图片完成返回中性 `unsupported`，清楚说明“已验证 1 张图片；模型绑定未验证，未恢复模型健康；供应商扣额未知”。不输出 signed URL、base64 或评分。
- 成功与失败都只完成审计，不写模型健康。失败仍为 `failed`，并返回 `failure_not_penalized`，阻止管理入口随后进行账号级健康失败写入；不伪装成功。
- 审计只增加固定 `lumaWebImage={verified:true,count:1,modelVerified:false,supplierCharge:null}`，不写 prompt、session、realm、signed URL 或任意供应商结果。
- 前端从原 `PilotActionDialog` 按 `preset:lumalabs` 进入独立图片任务表单；每次重新确认可能收费，明确取消本地等待不保证撤销生成或退款。revision、Grant、账号材料、映射、提示词和草稿状态变化均撤销旧确认；关闭后不接受迟到结果。

配置模型仅是 Gateway 的路由身份。现有 board 自动发现及默认 action 不构成供应商精确模型回执，本批刻意不将图片成功提升为模型健康成功。

## 证据与验证

目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-luma-image-probe-20261111`。
主要文件为 `final-receipt.json`、`direct-scope.diff`、`passed-tests.json`、`lines-before.json`、`lines-after.json`、各阶段日志与源码哈希快照。

| 检查 | 实际结果 |
| --- | --- |
| 后端红测 | 1 failed；原实现拒绝该媒体产品，未返回已验证图片事实 |
| 新增后端定向测试 | 6 passed，含正常管理 HTTP、费用/账号边界、现有健康保留、失败 sentinel、取消 |
| Rust 相邻回归 | 去重合计 176 passed / 0 failed / 3 ignored，包含上面 6 项 |
| 前端红测 | 未接线时 11 failed |
| 新增及相邻前端测试 | 45 + 19 = 64 passed；费用确认失效、精确账号中性展示、关闭丢弃迟到结果 |
| 前端类型检查 | 通过 |
| 隔离正常 `main.tsx` 构建 | 通过；输出在证据目录 `safe-web`，无线上发布/清理 hook |
| 生产 lib 编译 | `runtime-defaults` 和从该集合移除 Luma Web feature 均通过 |
| Rust formatter、checker 测试、ratchet、diff | 通过；checker 31 项；共享报告恢复原 bytes |

Rust harness 来自本次 Cargo 实际输出并复制冻结，SHA-256：
`4c54ab9544b2691a22203f7e6e096f63e9363a07a76bbbbbf1831db6c40110c8`。
构建关闭默认 `management-ui`，使用 `--locked --offline --lib --no-default-features --features runtime-defaults -j 4`。
红测构建期间另一工作线仅修改了 `src/provider_credential_probe_scheduler/products_tests.rs`，保留原始漂移回执，不把红测构建说成全树无变化。
Green 构建期间 Rust 输入未变；同时完成的 6 个前端文件变更逐项记录，该构建不含 management UI。前端最终另有独立测试、类型和构建输入哈希。
首次本批红/绿 test-harness 构建分别实测约 861/1146 秒，包含共享 Cargo 锁等待与编译；不是纯编译测速或冷/热性能对比。

有效行：`console_model_probe.rs` 489→490、`media_model_probe.rs` 137→160、`model_probe_recording.rs` 242→249、`lumalabs.rs` 109→122；新增 canonical owner 49 行，新增 Rust 测试模块 121/86/81 行，图片表单 65 行。无新增超限文件或例外；scripts/desktop 未配置专用 JS formatter，复用现有风格，没有安装新工具。

## 未验证边界

- 3 项 ignored 是既有 Suno/Udio/Luma Agents 的显式浏览器宿主，不当作本批浏览器验收通过。
- 未执行真实浏览器 UI→管理 TCP→合成 executor 联合验收或视觉/窄屏验收；本批为 DOM 交互测试、正常管理路由集成测试与独立前端构建，不冒称三者已联合跑通。
- 未联网刷新 Luma 当前 wire、未读取真实凭据、未调用真实或付费上游；真实 session、模型/action、费用和资产发行方仍待集中验收。
- 未实测 Server/PostgreSQL、原生包或跨进程部署；此前范围外 Gemini 极简 feature 编译失败未重跑或修复。
- 未出 EXE、未操作在线实例、未提交或推送。总计划 P50/P60/P70 仍未完成，新增模板仍为 17，无新增整卡 `offline_ready`。

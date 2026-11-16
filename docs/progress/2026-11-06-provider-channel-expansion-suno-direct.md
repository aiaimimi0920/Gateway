# R15 / W11：Suno 直连 Web 安全轮询子链与官方 OAuth 新发现

日期：2026-11-06。工作区：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`。
本批只闭合直连 Web 的提交/轮询元数据、重试策略、总时限和响应资源约束，不是完整音乐功能、整卡 `offline_ready`、P50 或总目标完成。

## 1. 认证与费用合同修正

联网确认 Suno 已公开官方 MCP/OAuth：官方帮助页、受保护资源 metadata 与授权服务器 metadata 均已固定。
AS 公开支持 authorization code / refresh、S256 和多种客户端认证方式；MCP resource 为 `https://api.suno.com/mcp`。
这些只证明 discovery，不证明客户端注册、redirect、工具 schema 或业务调用已经完成。
保留 `suno-web`，另列 `suno-oauth` 候选接力，**没有先建假模板**，累计模板仍 16。

官方说明 MCP 生成按账号套餐消耗 credits，歌曲进入同一 library；但全部 Web/OAuth 权益和额度关系仍需验证，不能据此自动合池或宣称完全独立。
官方另列 download credits；生成、播放和收费下载/转换分别处理。本批未增加下载计费、WAV 转换或真实平台请求。
详细端点、scope、SHA256、未决项和来源见 [Suno 渠道合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/provider-suno-channel-contract.md)。

公开 Web 参考固定为 OpenCLI commit `36bbbd7f60bd361a96e62e417a28ea09d6c2266b`，许可文件为 Apache-2.0；只参考协议事实，未复制实现或引入其运行依赖。
参考实现的 feed body / 当前模型与本仓旧合同存在差异，未凭单一第三方来源替换既有 wire 或模型。
匿名 create 页面重定向登录页，静态 chunk 获取遇到 403 后停止；GitHub API 403 和两个根 discovery 404 均保留，不当有效合同或真实平台验收。

## 2. 实际改动

- `src/upstream/suno.rs` 的直连入口沿用原 `UpstreamClient`/调用框架，分出 `direct / direct_state / transport` 三个明确 owner；未另起代理服务。
- 在任何请求前构造并验证生成计划；未知模型不先访问 challenge。challenge 仍按原端点检查，不绕过或自动解题。
- 只提交一次 generation；后续 feed 只携带原始 ID 集。原始 IDs 必须非空、唯一、格式正确；坏 clip 不能 `filter_map` 后冒充成功。
- 空或部分 feed 可继续等待，但不能证明全部完成；不接受外来或重复 IDs，不拼凑不同时点的部分终态；完整返回按提交顺序排序。
- 任一 clip 明确失败/取消即失败；默认等待模式超时返回 `suno_timeout` / 504，不返回原来的超时成功 envelope。
- 兼容显式 `waitCompletion=false`：保留 pending IDs 和 `completed=false`，不是最终成功证明；已失败 clip 即使 no-wait 也不伪装成功。
- 一个 deadline 覆盖 challenge、提交、轮询和最终响应处理，取配置 timeout 与请求 wait budget 的较小值；直连不再把短配置隐式提升到至少 240 秒。
- 元数据每响应最多 128 KiB、累计最多 1 MiB，按实际解码流字节检查。复用既有 `public_assets::read`，只扩充其 Suno feature 接线，没有改写共享 reader。
- 每次刷新 browser-token timestamp，保留本次准备好的 device-id。全程禁止重定向；提交路径及后续错误均 `retryable=false` / `FallbackHint::Abort`，Gateway 不自动重提或换号。
- 元数据 HTTP/HTML/坏 JSON 错误省略上游正文；超时/取消会丢弃在途响应。没有新增 detached task、队列、进程或持久数据迁移。

本地停止等待不等于远端任务取消、供应商未扣费或已退款。未伪造 usage、token 数、供应商实际费用或音频可播放性。
Browser-backed/远程 executor、本地 worker 和旧图片资产路径未修改；不能把本批直连保证外推到这些路径。

## 3. 红绿与验证证据

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-suno-20261106`。
最终机读回执为该目录下 `closeout.json`，源码/文档审计为 `audit.json`，每阶段均保留命令、日志及 SHA256 输入快照。

| 阶段 | 实际结果 |
|---|---|
| `red-v1`，生产尚未修改的四个执行器回归 | **0 passed / 4 failed**：外来 ID、部分坏 clip、失败成功 envelope、提交后仍可重试 |
| `green-v1`，正常运行 feature 下 `cargo test --lib ... suno` | **137 passed / 0 failed / 0 ignored**，其中本批新增 **17** 项；四个原红测均转绿 |
| `production-v1` | 正常 `runtime-defaults` 的生产非测试库 `cargo check` 通过 |
| `off-suno-v1` | 仅从默认运行 feature 集关闭 Suno 的生产库 `cargo check` 通过；不是关线业务测试 |
| `suno-gemini-v1` | 仅启用 Suno + Gemini Web 的生产库 `cargo check` 通过，ChatAIBot/Luma feature 均关闭；未改 Gemini 源码 |
| `suno-only-v1` | **失败，15 个 E0425/E0432/E0433**，发生于 Gemini Web 的 feature 收缩缺符号；完整失败保留 |
| `fmt-v1` | `cargo fmt --all -- --check` 通过 |
| checker / ratchet | **31/31**；扫描 **4687** 文件，14 个 immutable runtime artifacts 分报，governed source >700=0 |
| 差异 / 编码 / 来源 | `git diff --check`、所选新文件空白/UTF-8 无 BOM、固定公开文件 hash 和现存本地链接审计见最终回执 |

所有 Cargo 编译/测试均 `--locked --no-default-features --jobs 4`，不启用 management-ui，不升级依赖。
137 项通过使用默认线程栈，没有设置 `RUST_MIN_STACK`；真实 HTTP fixture 检查提交/轮询 body、稳定 device-id/更新 token、坏/部分/重复身份、失败、重定向、正文预算、卡住正文和取消后的 source Drop。
还通过实际公共 retry owner 验证 503 不再次提交，但**没有声称跑过完整 Suno 公共 API pipeline、管理/UI 联合或入站 TCP**。

红/绿阶段早期选定输入分别为 2609/2614（Rust 源码及 Cargo）；随后格式/生产和 feature 检查扩大到 3311 个选定输入。
各阶段无输入漂移，最终收尾将重叠 SHA256 对齐；不把不同冻结口径写成同一个数，也不把重跑旧测试累计成更多独立通过项。
最小 Suno feature 的缺符号位置不在本批文件；未做修改前最小配置基线运行，不能直接下因果结论，也未接管 S06/Gemini scope 或修改旧断言来隐藏失败。

根 `artifacts/effective-code-lines.json` 运行门禁前直接备份，门禁后逐字节恢复；不覆盖 inherited dirty report。
`src/upstream/mod.rs` 的修改前备份由本批红测冻结 SHA256 精确匹配重建并标记，不把晚备份伪装成事前备份。

## 4. 文件规模与审阅

共 11 个所选 Rust 文件：7 个新增、4 个原文件修改。新增模块最大 190 有效行；全部所选文件最大 382，无需新例外。

| 原文件 | 修改前 → 后有效行 |
|---|---|
| `src/upstream/suno.rs` | 343 → 258 |
| `src/upstream/suno_runtime_helpers.rs` | 379 → 382 |
| `src/upstream/suno_runtime_helpers/tests/execution.rs` | 355 → 355 |
| `src/upstream/mod.rs` | 200 → 201 |

审阅了信任边界（不可信 metadata/IDs/status）、秘密错误展示、重试/fallback、累计分配、超时/取消、在途流生命周期和算法复杂度。
每个 snapshot 最多 16 clips，身份检查及排序即使按小集合二次查找也有固定上界；没有无界队列或 detached 轮询。
未因此宣称旧 Web 会话/受众、旧图片下载的 SSRF/解码、worker 生命周期或音频有效性已经修复。

## 5. 后续接力与未验边界

1. 核实当前 Web 模型与参数、生成和 feed schema；补持久 device-id、短期 Clerk bearer 刷新、Cookie/受众及 challenge 的真实生命周期。
2. 在官方播放/导出权益和费用边界明确后，接受控音频读取与真实有界解码；不能拿 MP3 magic、URL 或元数据 complete 代替可播放音频。
3. 继续 browser worker 的 stdout/stderr 预算、取消/超时进程树退出及打包依赖；本批没有启动浏览器或验证 worker 清理。
4. 接精确媒体 probe、逐次费用确认和正常管理/UI→公共调用的本地审计、余额、取消与重建；独立 TCP/Server/进程/包仍待验。
5. 为官方 OAuth/MCP 冻结客户端注册/redirect/最小 scope 和真实 tools schema，复用最新 grant/持久刷新 owner 后创建正常模板，不照抄其他厂商身份。
6. 保留最小 feature 组合失败及后续 owner；不把代表组合编译通过外推成完整 feature 矩阵绿。

Luma Web、Udio、FreeBuff 公共消费执行、Kiro OAuth/兼容、Qwen OAuth/其他渠道缺口及 P50/P60/P70 均保留。
本批无真实凭据读取/录制/试用、认证/生成/收费调用、容器、在线实例、迁移、包、提交或推送操作；P34 单容器授权已消费，不复用历史问卷。

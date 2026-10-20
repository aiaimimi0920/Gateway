# AIGateway fork 能力采纳：第二批

日期：2026-10-19。范围：用户“将剩余你认为值得推进的能力继续推进”的授权。当前为本地源码开发，不包含提交、推送、发行、部署或真实供应商调用。

## 已实施的范围

### C01：有界模型声明观测

新增 requested / 实际发送 upstream / 原始 declared model 的独立表示。原始 JSON parser 的 optional metadata 与既有 canonical model fallback 分离；旧格式序列化可继续读取。其他 adapter 的构造器只补 `None` 契约接线，不接管 S06 的 Gemini 行为。

原始 HTTP SSE 在任何协议转换前观察，错误与 Bytes 原样透传，EOF/error/Drop 发布最多一次。无原始 provenance 的 Bytes、合成 envelope 和未接入路径保持 unknown；DashScope 投影及 buffered Anthropic→Responses 桥接显式避免把本地填入的模型名当证据。

复用进程内 metrics owner 的最近 100 条 ring，并接入已鉴权的 operations summary。没有新增服务、数据库版本、持久化队列、供应商探测或公开 metrics 标签。记录不包含 prompt、response、reasoning、tool input 或 credentials；不会改变路由、授权、健康或计费。

详细字段、支持路径、unknown 和资源限制见 [模型声明观测合同](../model-observations.md)。该能力不是模型真实性验证，也不是全部 adapter 的完整请求审计；本批没有增加桌面专用面板。

### D01 的独立小缺口：reasoning 生成配置

`pack_responses` 原已保存 `req.reasoning`，但实际桥接使用的 `pack_responses_bridge` allowlist 会删除它。本批补入该字段，验证 `effort/summary` 在真实 Chat→Responses request planner 的 stream=false/true 下保留，仍过滤无关 vendor 字段，并区分 absent 与显式 null。

**这不等于完整 D01 完成。** 历史 `reasoning_content`、公开 Responses summary 的跨协议往返和双向 SSE 转换仍未实现；opaque `encrypted_content` 也没有新增账号 provenance 或跨账号复用能力。本批没有把明文 Base64 伪装成加密状态，没有引入 Raw carrier 后让其他 adapter 静默变正文。

## 工程边界

- 修改前保存 HEAD、46 项既有 dirty 状态及 1766 个 tracked Rust 文件的 hash / 有效行数；不回滚已有修改。
- 新责任位于短模块；既有大文件只增加 optional 字段、`None` 构造或少量接线。不再建立一套 finalizer、重试预算、breaker 或 local-data owner。
- 管理鉴权、短 ID 校验、bounded ring、64 KiB 帧 / 1024 行 / 8 MiB 观察预算、并发写入、EOF/error/Drop 清理均有聚焦保护测试。
- `observationEnd=source_dropped` 仅表示 raw observer 被释放，不把合法终态后的主动 drop 误判为用户取消或健康失败。
- 不读取 live config / credentials 实值，不改端口 8317、计划任务、部署目录或本地持久状态。

## 验证回执

本批最终源码已完成以下本地验证；不是借用第一批的成功结果。Rust 验证清除 `RUST_MIN_STACK`，使用默认线程栈及 `--test-threads=1`，未扩大生产限制。

| 检查 | 结果 |
| --- | --- |
| `cargo test --locked --offline -- --test-threads=1` | 97 个 suite，4050 passed / 0 failed / 104 ignored |
| `cargo check --locked --offline --all-targets` | exit 0；仍有 unused / dead-code 等 warnings，不宣称零警告 |
| 三个聚焦合同测试 | 8 passed / 0 failed；最终全量回归再次覆盖 |
| Node scripts 测试 | 2122 passed / 0 failed / 2 skipped |
| Python 测试 | 358 passed / 0 failed / 4 skipped |
| Desktop TypeScript typecheck | exit 0 |
| line manifest 校验 | 44 checked / 0 errors |
| 有效代码行数 checker tests / ratchet | 31 passed；ratchet exit 0，未运行 strict |
| `cargo fmt --all -- --check` / `git diff --check` | 均 exit 0 |

ignored / skipped 不计为实际通过；本轮未进行真实供应商、Redis live、打包 EXE、部署实例或人工业务验收。

本批相对开始快照新增或修改 40 个 Rust 文件，主要为 optional canonical 字段的构造接线；最大为 474 有效行，没有新增超限文件或修改 baseline。全量回归后的 40 个文件 hash 与冻结快照一致。原始 16 项现场文件内容未变；参考 fork 的 219 个源码文件、HEAD、branch 和 index 均与审计快照一致。Gateway HEAD 仍为 `08634912809fea48b45fd888936cd688356fdfac`，没有提交或推送。

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-fork-adoption-phase2-20261019/`。包括修改前 `before.json`、新合同的失败测试 `red.log`、`focused.log`、源码范围 `scope.json` / `frozen-scope.json`、`preservation-check.json`、汇总 `verification-summary.json` 以及 `static/`、`rust/` 下的完整验证日志。

## 仍需独立推进的候选

1. 完整 D01：公开文本、opaque、历史消息归属、双向 SSE 去重、foreign-wire admission 与可信账号 provenance 必须一起确定合同，不能用一个 Base64 carrier 冒充闭环。
2. A04：OAuth-first 需要明确 auth class 的可信来源、strict / prefer / fallback 和 pin / 权限边界，不能按 token 字符串或模型名猜。
3. B01/B02：共享错误预算和单次恢复许可需扩展现有 attempt gate，并覆盖 Local/Server 原子 admission、崩溃及取消结算，不能外挂第二套 breaker。
4. B03/B04/B05：外部健康源先做 advisory；还需专属 SSRF、同源认证、保鲜和 bounded worker 合同，不因外站宕机影响正常路由。
5. E01/E02：维护预览/选择/部分失败回执和定向恢复应复用归档、revision CAS、精确 model health owner，不做全量解除冷却或文字匹配 401 后自动删除。

这些是明确尚未实现的候选，不算本批成果；C02 指纹鉴真、无限恢复、请求外 ticket ping、DOM 面板补丁、周期重启 workaround 等原“不采纳”结论不变。

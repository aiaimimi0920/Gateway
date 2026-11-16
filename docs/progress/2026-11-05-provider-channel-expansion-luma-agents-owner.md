# Luma Agents 官方 API：独立生成、轮询与资产执行链

日期：2026-11-05。对应 [总计划 R14/W10/P10/P40](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/plan/2026-11-02-provider-template-expansion.md)。
本批是独立执行链的源码与定向离线验证，不是完整服务商、P50 或总计划完成。
工作区仍为本地 Gateway，`main`、HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`；保留其他未提交修改。

## 实际改动

- 官方 [合同与联网证据](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/plan/provider-luma-agents-contract.md) 已冻结新 `agents.lumalabs.ai/v1` 产品；`lumalabs-api` 是后续模板方向，不改旧 Web preset。
- 新 adapter `lumalabs_agents_compatible`、profile 常量 `lumalabs_agents_api`、feature `line-lumalabs-agents-official-api`，沿既有 `call_flow::prepare` 与 JSON passthrough dispatcher 接独立 owner；没有另起网关或复用 Web board wire。
- `uni-1 / uni-1-max` 图像生成/编辑和 `ray-3.2` 视频提交，保留显式模型、Unicode、reference、source、关键帧及适用控制；不支持的字段/组合拒绝，不静默丢弃。当前是单结果请求，不支持批量 `n>1` 或实时流式。
- 一个 `POST /generations`，之后只 `GET` 原 UUID；严格核对任务 ID/model/type/state/output。同步 HTTP 错误、异步 failed、未知结果和不完整载荷均不能触发自动重提或渠道回退。
- completed 后实际读取所有准入资产；图片是 PNG/JPEG 有界完整解码，视频仅检查 MP4 容器结构，不冒充 codec 解码或播放。返回 URL 与 Base64 两种方式都必须先读资产，HTML/空载荷/损坏图片不算成功。
- `usage.credits_charged` 保留供应商已报告数值，未知仍为 null、确认零仍为 0，不估算 tokens。这只是结果扩展字段，尚未接供应商费用/公共结算账本。

主要 owner：
[请求映射](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/luma_agents/request.rs)、
[视频控制](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/luma_agents/video.rs)、
[执行入口](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/luma_agents.rs)、
[任务状态机](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/luma_agents/generation.rs)、
[资产读取](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/luma_agents/assets.rs)。

## 安全、预算与生命周期

固定 API 目标，只发送该产品的 Bearer/Accept；调用者、provider 和共享 client 默认 Cookie/Authorization 不进入请求。已知 `luma-api-*` 材料或官方目标不能逃入 generic/Web executor。
资产使用新建、无凭据、无代理、不跟随重定向的 client；只准入公网 HTTPS，无 userinfo/fragment/自定义端口，DNS 所有返回地址检查后固定连接解析。公共 IP 判断从原 site-health owner 原样抽出复用，没有改变该策略。

提交、poll、DNS、下载、验证等待共用一个总 deadline：不超过外层 timeout，并有图片 120 秒/视频 600 秒的本地上限。
每份 metadata 64 KiB、累计 4 MiB；资产累计 64 MiB；序列化请求 16 MiB、单 inline Base64 字符串 8 MiB；最多 300 次 poll。这些是本地预算，不冒充平台完整能力或官方限额。
PNG/JPEG 解码最多两项并行，每项分配上限 64 MiB、尺寸上限 8192×8192；取消后已开始的 blocking decode 可继续有界收尾，permit 跟随工作，不宣称 CPU 解码可抢占。
本地取消/超时停止 HTTP/轮询，不证明远端生成已取消或没有扣费；不自动 DELETE 远端对象，不宣称跨进程 exactly-once。

## 实际验证

证据目录：[gateway-luma-agents-20261105](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-luma-agents-20261105)。
后端均显式 `--locked --no-default-features --jobs 4`，不启用 `management-ui` 发布/清理 hook；TEMP/TMP 指向本批临时目录，未设置 `RUST_MIN_STACK`。

| 命令选择范围 | 结果 | 回执/日志 |
| --- | --- | --- |
| `cargo test --features runtime-defaults --lib luma_agents -- --test-threads=1` | 20 passed，0 failed | `green-owner.json` / `.log` |
| `cargo test --features runtime-defaults --lib protocol::call_flow:: -- --test-threads=1` | 9 passed | `adjacent-call-flow.json` / `.log` |
| `cargo test --features runtime-defaults --lib site_health::transport:: -- --test-threads=1` | 8 passed | `adjacent-site-health.json` / `.log` |
| `cargo check --features runtime-defaults --lib` | exit 0，最终源码的生产非测试分支编译 | `check-owner-final.json` / `.log` |
| `cargo check --features line-gemini-web-reverse --lib` | exit 0，代表性 Agents 关闭组合 | `check-feature-disabled.json` / `.log` |
| `cargo fmt --all -- --check` | exit 0 | `rustfmt-final.log` |
| `npm run test:effective-lines --prefix scripts` | 31 passed | `checker-tests.log` |
| `npm run check:effective-lines --prefix scripts` | exit 0，ratchet | `ratchet-v2.log` / `ratchet-receipt.json` |

按完整测试名去重共 **37 个 Rust 测试通过，20 个为新增 Luma 合同**，不累计历史重复运行。3,012 项选定 Rust/config 输入在 green、相邻检查和 feature-off 编译期间无变化；最终核对见 `final-verification.json`。
成功路径包含真实 loopback HTTP 的 submit→poll→读取资产、图像编辑和合成 MP4 结构；负测包含错误模型/类型/ID/状态、HTTP/异步失败、超时/Drop 后连接关闭、metadata 限额、坏图片、意外资产状态及不重提。
fixture 仅可重定向两个声明的合成 origin，不转发公网；测试模式不执行生产 DNS/TLS 固定解析。因此公网 DNS/TLS/真实 CDN 和生产 poll 间隔没有得到真实网络验收，不以本地成功外推。

21 项选定源码/config 的来源、UTF-8 无 BOM、尾随空白与局部格式见 `source-audit.json`，20 项 Rust 文件最大 431 有效行，新增 Rust 文件均低于 500；site-health IP 函数抽取前后相同。未改 baseline/exception。
本轮保留编译警告，不运行 `cargo fix` 或扩展为无关清理；未声称全仓、全 feature 或 strict 债务审计通过。

## 红测与审查修补

初始四项 owner 合同在未实现执行入口上 **0 passed / 4 failed**，实际错误包括 `Luma Agents API is not implemented`；红测源码哈希和完整失败保留为 `red-owner-inputs.json` / `red-owner.log`。
既有两位只读审查者指出的三项已修补并进入最终绿测：multi-keyframe 的 prior `generation_id` 被误拒绝；已知 API Key 可经错误 adapter/target 逃逸；意外 2xx/3xx 错误状态可能被公共错误边界返回为成功/跳转。现在后者映射 502，合理 4xx/5xx 保留原状态。
官方视频 guide 对 multi-keyframes 的 `10s + hdr` 与其通用校验/价格表自相矛盾；当前仍明确拒绝该组合，记录为待权威证据，不用付费生成试探，也不称完整组合保真。
公开资料的 403/404、rustfmt 初始语法错误、patch anchor 不匹配及 checker 绝对报告路径错误均保留在 `tooling-errors.json`，没有删除失败现场或把工具失败当测试成功。
最终来源审计发现早期 `check-owner` 与审查修补后的九项输入不同，因此重新执行生产库编译，而不复用旧绿回执；初次审计保留为 `final-verification-precheck.json`。审计脚本也按 Git 契约接受无 whitespace 输出的 `diff --no-index --check` exit 1（文件不同），不把它误当 whitespace 失败。

## 下一接力点与未完成范围

1. 先接正常 preset/profile/implementation-line 注册、原声明到最终配置身份保护和 SecretPatch/Keep/清空受众绑定，再开放空池模板与普通 API Key 管理 UI。当前没有可宣称完成的 `lumalabs-api` 模板，累计新增模板仍为 **15**。
2. 明确模型目录策略、精确账号 probe，接公共媒体 router→正常 Key/route→send→usage/audit/本地结算，并补失败/取消及 runtime 重建；本批 dispatcher 测试不冒充这些完整业务链。
3. 继续 `video_edit / video_reframe / layering`、EXR、Files 和 LLM 操作的产品范围/映射及视频实际解码/播放需求，不把当前 image/image_edit/video 子集说成覆盖全部官方 API。
4. 独立推进原 Web board 的能力、材料、worker/取消与资产边界；API/Web 额度关系仍为 unknown，绝不混池或自动互相回退。
5. 真实账号、授权、模型权益、费用/额度与公网资产验收在 P60/P70 集中安排；其他原定渠道、OAuth 刷新/兼容/恢复剩余和 P50 无凭据全功能集成/包仍未完成。

本批没有读取或试用真实/历史凭据、调用平台生成 API、录制、创建 Redis/Docker 资源、修改在线实例、出包、部署、提交或推送。
P34 单容器许可已在前序消费并清理；恢复时再次收到的同一问卷回执不成为第二次授权，本批没有复用。

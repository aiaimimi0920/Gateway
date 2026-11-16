# magpie 对照与原生接入：首批 checkpoint

日期：2026-11-08。这里只记录当前首批，不是整个截图范围完成回执。
用户指定的后续优先级为 **OAuth／订阅账号接入**。

## 范围与现状

- 最初三张截图的 42 项已逐项对照 Gateway 的实际入口与实现线，见
  [42 项对照](../providers/magpie-expansion-2026-11-08.md)。本轮前 46 个控制台入口与
  26 条 adaptation receipt 都不是“全部真实调用通过”的数量；余额受限、文档合同、媒体范围和
  `flow-web` 暂停状态分别保留。旧 17 项扩展未被本轮自动升级为完成。
- 后来五张截图仅取红框的 23 张第三方卡片，已联网核对仓库、固定 SHA、许可和协议，见
  [23 张红框来源与订阅排序](../providers/magpie-third-party-2026-11-08.md)。
  与原范围重合的产品及两套 QwenWork 不重复统计；ModelScope 签到插件不算推理服务商。
- 参考源码用于理解产品合同，没有安装或运行 magpie 插件，也没有执行其登录、自动下载、
  签到或 tunnel 指令。Gateway 继续拥有凭据、路由、传输、取消和保存。

## 首批代码落地

| 产品 | 本批实现 | 尚未证明 |
| --- | --- | --- |
| River | 新 preset/profile/feature/manifest；Bearer key 账号池与 SecretPatch；官方 live 目录；Chat/tools/图片输入/SSE、Responses JSON/SSE 复用新框架 | 真实账号目录、模型调用和付费状态；状态型 Responses 不能伪造支持 |
| Cloudflare Workers AI | 新 preset/profile/feature/manifest；Account ID 与 Token 绑定；官方有界分页目录；Chat/SSE、Embeddings；严格限定模型与非流式 Responses | 真实账号、Token 权限、目录和推理；不是 Cloudflare AI Gateway BYOK |
| Grok Build | 在既有 `grok-oauth` owner 上补 device 管理 API 与 UI；旧 code/manual callback 兼容；显式一次 poll、上游节拍、取消/过期/容量、一次性 receipt 与显式保存 | 真实 client 注册资格、device 登录、refresh、订阅推理及完整产品验收 |

没有新增数据库版本或依赖，没有新建 adaptation receipt 或点亮整卡验收状态。
Grok 的 client ID/scopes 必须有权供本集成使用；不借官方 CLI 身份，不读取现有客户端登录态。
device 没有 code-flow nonce，因此不会宣称已验签用户身份。

## 当前验证证据

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-magpie-expansion-20261108`。
后端使用锁定离线依赖，未启用默认 `management-ui` 构建；
测试 TEMP/TMP 在证据根内，使用合成 token、loopback 和隔离数据库，不访问真实 `.ng`。

| 检查 | 已观察结果 | 日志 |
| --- | --- | --- |
| River 定向 Rust | 5 passed | `river-final.log` |
| Workers AI 定向 Rust | 7 passed | `cloudflare-rust.log` |
| 管理 API → loopback → audit/model state 集成 | 2 passed | `console-catalogs-final.log` |
| Grok 当前 device/API/旧 flow UI 定向 | 5 files / 42 tests passed | `ui-device-final.log` |
| River/Workers AI/相邻目录及链接 UI | 7 files / 45 tests passed；与上一行有重叠，不相加 | `ui-expanded-final.log` |
| 当前 UI typecheck | passed | `ui-typecheck-device-final.log` |
| implementation manifests | 47 checked / 0 errors | `manifests.log` |
| effective-lines checker 自测 | 31 passed | `effective-tests-final.log` |
| 收口 effective-lines ratchet | passed；未改 baseline/exceptions | `effective-ratchet-close.log` |
| 含完整 `runtime-defaults` 的最终候选 | 受并行 Suno 测试模块编译错误阻断，不算通过 | `backend-final.log`、`backend-final-r2.log` |
| 排除独立 Suno feature 的本批后端候选 | 同样受 Suno 未受 feature guard 保护的引用阻断，不算通过 | `backend-without-suno.log` |
| 后续完整 `runtime-defaults` 编译及定向联合回归 | 189 passed / 0 failed / 5372 filtered out；包含 Grok 最终修正、River/Workers AI 邻接合同和 DIM 首批基础测试，不是全库测试 | `dim-grok-green.log` |

首轮 Grok 回归为 90 passed / 1 failed；失败来自本批测试误断言旧式 `refresh_endpoint` 应落盘。
既有 `persistence.rs` 明确清空旧字段，专用 `grok_oauth` 上下文编译产生刷新配置，
对应既有 `grok_context_creates_dedicated_refresh_without_legacy_endpoint` 也通过。
本批已修正为断言旧字段为空且编译后的 endpoint/client ID 正确，没有为测试改变生产语义。
该首轮早于最终长间隔修正，不能当作当前源码全绿证据。

最终含 Suno 的候选先后遇到 `suno_media_probe/tests.rs` 缺失、子模块路径缺失和
`upstream::public_assets` 私有访问错误。没有修改或撤销并行 owner 的文件。
后续隔离验证的精确 feature 列表记录在 `backend-without-suno-features.json`；
它也在 `suno_media_probe.rs` 的 `resolve_model` / `normalize_music_generations` 引用处失败，
因为对应 Suno 导出在该 feature 关闭时不存在。没有修改 Cargo feature 默认值，也没有给并行源码补 guard 来制造本批通过。
后续并行 owner 完成其模块后，完整 `runtime-defaults` 候选已编译，定向联合回归为 189 passed / 0 failed，
覆盖最终长间隔修正、取消在途 poll 和修正后的 router 刷新断言。原失败日志保留，不再把已消失的 Suno 阻断列作当前阻碍。
此回执早于 DIM 后续身份验证与持久刷新接线，不能替代那些新增路径的验证。

`source-checkpoint-final.json` 保存本轮文件哈希、写前/写后有效行、UTF-8 无 BOM、
rustfmt 与 scoped diff 检查结果；不是干净 HEAD 的构建声明。共享工作区的其他改动不属于本批交付。
本批源码均不超过 500 有效行，没有通过修改代码规模豁免表放行。

## 后续未完成范围

除上表首批外，其余截图产品的完整原生接入仍未完成。优先完成 OAuth／订阅产品，
新增红框候选先从 DIM 开始，再按各自生命周期推进 AtomCode、Copilot、Nous、MiniMax Code、Lobster 等。
QwenWork/Qoder/COSY 和 CodeArts 签名链必须独立实现；普通 Qwen API Key 不能算替代。
DIM 的国内/海外 issuer、PKCE、userinfo、受众绑定、轮转刷新、逐模型协议目录和账号调用应作为一个完整切片，
客户端注册与 redirect 权限未知处必须先核实，不照搬社区 client ID。

本批没有真实 OAuth 登录或续期、付费推理、桌面前台视觉验收、Server/跨应用验收、出包、提交或推送；
没有切换在线实例，也没有操作真实凭据或数据库。

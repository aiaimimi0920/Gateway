# Luma Web：有效图片、公共调用与本地结算

日期：2026-11-07（Asia/Shanghai）。仓库：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`。
分支 `main`，HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`；实现位于继承 dirty tree，
不是该 HEAD 的发布。总计划仍执行中；累计新增模板仍 16，无新增整卡 `offline_ready`。

## 1. 本轮完成的业务子链

承接[worker 子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-07-provider-channel-expansion-luma-web-worker.md)，
补齐 prompt-only 单图的“远端生成结果 → 安全取图 → 完整解码 → 公共响应 → Local 结算”离线闭环。
这不冻结当前登录后的 board wire，也不证明 URL 属于当前原始任务；视频/音频仍是旧 metadata 路径。

### 改了哪些内容

- [媒体 sender](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/lumalabs.rs)
  的 URL、`b64_json` 两种响应都必须先取得并完整验证同一图片。URL 模式不再直接返回字符串成功；
  b64 模式不再用账号共享 HTTP client 无界读 body、按 URL 后缀猜 MIME。
- 新 [Web 图片资产 owner](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/lumalabs/assets.rs)
  复用已有 `public_assets`：独立无凭据/无代理客户端、公网 HTTPS 目标与 DNS pin、禁跳转、仅 200、
  解码后累计读取最多 16MiB。PNG/JPEG 必须完整解码，实际格式与 MIME 匹配（允许 octet-stream）；
  沿用 8192×8192、64MiB 解码分配和两个 blocking decoder owners。未新增平行下载器/线程池。
- 分派、下载、等待解码与组装保留同一个绝对 deadline。取消不能抢占已开始的 blocking decode，
  既有 owner 继续持有 permit 到实际退出；本轮未独立计时验证跨阶段总预算，不能把接线当作该实测。
- 资产错误用固定 Luma 身份/错误码投影；不回显签名 URL、body、DNS/连接细节，资产 403 不冒充
  凭据失效。生成后失败统一禁止 retry/fallback/换账号重提；本地退款不代表 Luma credits 退款。
- 新 [图片预准入](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/lumalabs_response_helpers/image_request.rs)
  在 action/网络提交前验证 `response_format`、显式整数 `n=1`、`output_format`/`format` 为 png/jpeg，
  拒绝其他显式格式及畸形 `images`。保留 `images: []` 兼容；上传和 mask 仍明确不支持，不静默忽略。
  当前不支持 WebP/GIF，也没有为未知的当前上游格式制造支持声明。
- 旧未验证的图片 helper 仅供既有纯 envelope 单测，不再编入生产路径。
  `public_assets` 的 cfg 补入 Luma Web，不依赖 ChatAIBot、Agents 或 Suno 恰好开启。

## 2. 正常公共入口实际验证

测试走真实 `build_app_state` Local bootstrap、正常管理接口创建普通 Key、公开
`/v1/images/generations` router oneshot，以及真实 loopback HTTP executor/资产 peer 和 SQLite：

1. PNG/JPEG × URL/b64 四组都执行一次 executor POST、一次无凭据 asset GET，再返回成功；
   b64 比较完整原 bytes 并实际解码，URL 保留原签名地址，不把地址本身当有效图片证明。
2. 精确 alias → native model、账号和 credential 均写入原 audit；排除不相关/禁用凭据，
   备选 provider 存在但没有被调用。未知 token usage 保持 `None`，不把图片/credits 换算为 token。
3. 成功本地次数余额 2→1；HTML、损坏图、MIME 不符、403、302、16MiB 超限等失败 2→2，
   audit 为 failed、不可 fallback，错误中无合成签名秘密；unsafe URL 零资产请求。
4. 等待 asset body 时 caller Drop：源连接被释放、audit cancelled、本地预占退回 2，
   active_count 归零、并发上限不变、模型 failure_count 不增加，不把取消判成账号损坏。
5. 非法响应格式在 executor 前拒绝，不消耗生成调用；其他 n/格式/畸形输入由预准入单测覆盖。

资产 HTTPS URL 仅通过已有 test-only task-local 映射到不转发的 loopback peer。
这不是生产 DNS/TLS/CDN、独立 Gateway TCP、真实浏览器或当前平台的生成验收；没有真实凭据。

## 3. 红绿、生产编译与来源

证据目录：
[gateway-luma-web-images-20261107](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-luma-web-images-20261107)。
包括直接 preimage、source snapshots、各阶段 command/exit/耗时/log、有效行、scoped diff、
去重测试名和最终 `audit.json`/`closeout.json`。TEMP/TMP/TMPDIR 显式指向该目录下 temp，未自动清理。

- 首次 `red` 是测试 fixture 访问私有字段的编译失败，不作为行为红测；将测试移入已有 client 测试
  owner 后，`red-runtime` 真实运行 **5 passed /6 failed**。5 是前批已有回归；6 个新增失败分别证明
  提前 URL 成功、坏图成功、unsafe 地址成功、迟到格式校验、取消前提前结束和缺少本地预准入。
- 最终 `green-final`：`luma` 名称筛选 **141 passed /1 ignored，7 项新增**。
  包含 Web、Agents、协议/helpers 等，不称纯 Web 141，也不把前后两次绿测累计成 282。
  1 ignored 是已有 Agents 正常浏览器 host opt-in，未运行不计通过。
- 代表组合关闭其他三个共享资产消费者、保留 Luma Web 时，先真实出现唯一
  `E0432`（`public_assets` 未注册）；补 cfg 后生产编译通过。最终正常 runtime-defaults、
  该代表组合、仅关闭 Luma Web 的代表组合三项生产 lib 检查均通过。不是完整 feature 矩阵。
- Rust 均使用 `--locked --offline --lib --no-default-features`、两个 Cargo jobs；
  不启用 management-ui 发布/清理 hook。Rustfmt、checker **31/31**、ratchet **4750** 项通过；
  14 个 immutable runtime artifacts 另列，governed source >700 为 0。未改 baseline/例外/依赖。
- 11 项受审源码最大 204 有效行；主 sender 125→104，规划 owner 149→143，
  client 测试入口 137→140，共享模块入口 201→202。新文件均≤250；未动 S06。
- 最终验证各阶段 4732 项所选输入无漂移，并与最终源码逐项核对。
  选集不涵盖全部脚本/配置/外部工具链，不称全仓冻结。编码、引用、diff 及根有效行报告
  按直接 preimage 恢复的结果以 `audit.json` 为准，不覆盖继承现场。

## 4. 仍需接力，不能报整卡完成

1. 当前 board actions/events、原任务 ID/完整终态、模型/参数、Cookie/JWT/refresh/challenge，
   以及签名资产的精确受众/任务归属。旧 `finalizing` 和“可解码图片”都不证明正确原任务完成。
2. 有效视频/音频、当前完整操作、精确图片/媒体 probe、逐次供应商费用 UI、完整 Keep 联合路径。
   本轮只闭合 prompt-only 单图 Local 次数结算；真实扣额、cash/晚到费用未验证。
3. 远端取消/lease/恢复、跨请求 exactly-once、独立 TCP/进程/Server/PG、原生浏览器和新源码整包。
4. [官方 OAuth/MCP](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/provider-luma-channel-contract.md)
   的 client/工具/任务/授权刷新仍待接，不借 Agents REST 或本轮 Web 图片成功替代。
5. FreeBuff、Suno/Udio、Kiro OAuth、Qwen OAuth 等原范围缺口及 P50/P60/P70 继续保留。

没有读取历史凭据或真实 `.ng`、登录、注册 client、真实生成/付费、新容器、在线实例/数据库变更、
出包、提交或推送。P34 容器授权已在此前消费并清理，本轮没有复用。

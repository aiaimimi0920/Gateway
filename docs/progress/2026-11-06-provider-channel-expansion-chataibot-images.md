# ChatAIBot：正常图片调用与受控结果读取

2026-11-06。本批为 R13 的无凭据图片执行子链，不代表完整渠道、精确媒体 probe、P50 或总计划完成。工作区 `C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`；保留既有 dirty edits，未提交/推送/部署或出包。

证据根：[gateway-chataibot-20261106](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-chataibot-20261106)；公开来源与边界：[contract.md](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-chataibot-20261106-contract/contract.md)、[sources.json](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-chataibot-20261106-contract/sources.json)。最终以该证据目录的 `closeout.json`、冻结输入和各阶段 receipt/log 为准；不能以代码已落地或编译成功替代公共调用验证。

## 1. 当前官方静态合同与认证类型

联网 GET 官方公开 HTML/固定构建 `2RL4nEJMFRhG_IoiFz62H` 的静态 JS，没有登录或生成。固定 API base、Web same-origin Cookie、native 条件 Bearer、async 图片能力 header、三种图片提交与同 job 查询均有来源、获取时刻及 hash；不复制官方代码，只取协议字段事实。Web/native 的分支不是独立推理 API Key 或第三方开放 OAuth 的证据，本批不凭第三方登录按钮新增渠道，也不推断额度关系。

当前公开构建没有证实旧 `imageAspectRatio` 设置属性。typed `chataibot_web` 不写该旧账号设置，暂只支持服务端默认比例；显式非 auto 的 size 和未实现参数在网络前拒绝。旧 generic 配置保留其历史设置行为，不自动迁移为当前 typed 产品；其设置并发、未知比例合同不由本批认定完成。已有模型/编辑模式映射沿用仓库参考，不冒充当前账号目录、权益或计费依据。

## 2. 实现边界

- 复用正常图片入口、call-flow/选路/attempt gate、Local auth/余额、最终审计与取消 owner；没有另建网关、凭据 store、自动生成或自动账号切换。
- 统一先验证 endpoint、prompt、n=1、response format、图片数量/累计编码预算、MIME、base64 和编辑/合成型号能力，再做任何 settings/submit。JSON 与公共 multipart 上传的 `data_base64` 投影均进入同一执行器。
- typed 材料固定 HTTPS ChatAIBot 受众、原账号 token/Cookie/可选 UA/有效期；忽略 caller 的认证和 Cookie，拒绝混入 OAuth/运行 seed。Web 请求不伪装 native Bearer。
- 声明 `x-supports-async-image-jobs: true`；只提交一次，pending 后立即 GET 同一 `/api/jobs/{id}`，pending 间隔 2 秒，completed 必须带可解析 data；error/未知状态/冲突 ID 拒绝。ID 字符边界是本地策略，不冒充服务端幂等保证；requestId 只作正常请求字段。
- 一项总 deadline 覆盖 submit、poll、DNS、资产和完整解码；不自动 retry/fallback/redirect。轮询错误直接停止本地等待，不借此重提生成；本地取消不意味着撤回远端任务或未扣供应商额度。
- JSON 单响应 decoded 128 KiB、累计 1 MiB；每操作只处理一张结果图、下载上限 16 MiB。URL 和 base64 响应都先验证真实图片，不能直接把 URL 或 generation ID 当成功。
- 复用并提取 Luma 已有的公网 HTTPS/DNS pin/无凭据 direct client 与 PNG/JPEG 有界完整解码策略。图片尺寸不超过 8192×8192、解码 allocation 64 MiB、共享最多两个 blocking decoder；取消后 permit 随仍在完成的有界解码存活。Luma 保留原产品错误和 MP4 结构策略，不扩大成播放保证。
- 没有真实 Token/credits 证据时不伪造 0 或换算图片数；普通次数 Key 成功结算一次、失败取消退款，Token Key 沿既有预占契约处理未知 usage。

主要 owner：[executor](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/chataibot.rs)、[准入/材料](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/chataibot/admission.rs)、[transport](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/chataibot/transport.rs)、[jobs](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/chataibot/jobs.rs)、[共享受控资产](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/upstream/public_assets.rs)。

## 3. 验证记录

生产执行器改动前，`red-v1.log` 实际执行 3 项，**0 passed / 3 failed**：pending 被误报 missing image；不支持的编辑已经先写 settings；无效图片的 URL 被直接判成功。新增的共享资产模块当时尚未被原执行器使用。3295 项选定 Rust 输入前后 hash 无漂移；不是编译失败替代红测。

- 最终 `green-v2`：ChatAIBot **39 passed / 0 failed / 0 ignored**；包含本批新增 14 项永久测试，以及既有协议、材料、preset 与 header 合同。正常生成/单图编辑/多图合成和公共 multipart 上传均走生产 bootstrap、正常管理创建/充值普通 Key、public router oneshot、真实 SQLite 和显式 task-local 回环 HTTP；没有真实平台转发。
- 相邻 `luma-adjacent-v1` 在媒体 probe 的 HTTP 失败测试触发 `STATUS_STACK_OVERFLOW`；`luma-stack-v2` 尝试在生产 ChatAIBot future 边界 heap-pin 后仍失败，该尝试已撤回，保留源码快照和失败日志。新增的双 task-local remap 曾按值嵌套完整大 probe future；只在本批改动的 Luma 测试作用域对内层 future heap-pin 后，`luma-stack-v3` 原用例通过，随后 `luma-adjacent-v3` **41 passed / 0 failed / 1 ignored**。不提高 `RUST_MIN_STACK`、不新增线程、不跳过失败用例、不改变生产请求和取消语义；该 ignored 为已有显式浏览器 host，本批未运行。
- 上述最终正常测试按完整名称去重为 **80 项**，不是将多次 red/green 或相邻重复运行相加。`production-v1` 的非测试生产库编译、`fmt-v1` 通过；只关闭 `line-chataibot-web-reverse` 的 `off-luma-v1` 亦为 **41 passed / 1 ignored**，共享资产模块没有绑定为必须开启 ChatAIBot。六个最终成功阶段各冻结 3304 项选定 Rust 输入，前后无漂移；最终当前源码核对见 `closeout.json`。
- 补充 `off-chataibot-v1` 的旧命名测试为 **6 passed / 2 failed**：`missing_chataibot_quota_propagates_error_instead_of_fallthrough` 仍期待启用线的 `leftAnswersCount` 错误，`converts_chataibot_credential_to_cookie_image_adapter` 仍期待被关闭的 preset 注入 `token` Cookie 配置。两项在正常配置均已通过；保留完整关线失败，不改变生产关闭语义迎合它们、不改旧断言或悄悄排除后报全矩阵绿。本批仅证明上述已选子链，不证明完整 feature 矩阵。
- checker **31/31**、effective-line ratchet **4680 项**、`git diff --check` 通过；14 项固定 hash 的 immutable runtime artifacts 分报，governed source 没有 >700。本批审阅 22 项源码，其中 13 项新模块、6 项原文件修改、3 项既有协议参考保持不变，最大 283 有效行。原执行器 402→149、Luma 资产 transport 132→73、图片/视频校验 122→94；没有修改 baseline/exception。根有效行报告由本轮运行前直接备份并逐字节恢复，最终来源、UTF-8 无 BOM、链接和 scope 审计见 `audit.json`。

## 4. 明确未完

精确账号显式媒体 probe/逐次费用确认与正常 UI 联动、当前 Cookie/challenge/refresh/登录恢复、模型/编辑模式/比例/output preset 的完整当前合同、其他认证可能性及额度关系、完整参数和真实供应商费用终态仍需后续。正常配置/Keep 全链、入站独立 TCP、跨进程/Server/PG、原生 EXE/整包与真实生成尚不能由本批 router oneshot 推定。之后继续其余尚未完成的 Luma Web/Suno/Udio 等原始渠道，不用扩展 ChatAIBot 可选参数替代其他渠道的核心实现。

累计模板仍 16，无新增整卡 `offline_ready`。其他原始渠道与 P50/P60/P70 保留。未读真实 `.ng`/历史凭据，未发认证/生成/购买请求，未操作在线实例或 Docker；P34 容器批准早已消费并清理，本批没有重放历史授权。

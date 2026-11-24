# Command Code Provider API Chat 合同

本页记录 2026-11-10 联网核实的官方合同与 Gateway 当前切片。实现线 `commandcode-provider-api`，preset `commandcode-api`，profile `commandcode_chat`，auth mode `commandcode_api_key`。这不是 Go 套餐的私有生成接口，也不是完整 Command Code 产品验收。

## 官方来源与实际目录

- [Provider 文档](https://commandcode.ai/docs/provider)：Provider API 的 Chat、Responses、Messages、Models 与 System One 五条路由；Bearer API Key 可用于所有 Provider 路由，`x-api-key` 是 Messages 的另一种认证方式。
- [Studio API Keys](https://commandcode.ai/docs/studio#api-keys)：用户在 Studio 获取自己的 Key。官方允许 API 套餐在第三方兼容客户端使用；没有已核实的 Gateway OAuth client 注册合同。
- [Go 套餐](https://commandcode.ai/docs/plans/go)：Go 不具备第三方 API 权限；GOAT、Pro、Max、Team、Provider 的具体权限仍取决于真实账号。
- [官方 Pi provider](https://github.com/CommandCodeAI/pi-commandcode-provider/tree/26e5fe68c2ab4c4d92216cc6114b27eb9401b516)：固定 SHA `26e5fe68c2ab4c4d92216cc6114b27eb9401b516`。README 对单路由的描述与当前多路由目录不一致，以目录的 `supported_endpoints` 和源码处理为准。源码采用 `max_tokens`，并把 developer 转为 system；其默认 reasoning 标记不能当成每个模型的官方能力证据。
- [公开 Models API](https://api.commandcode.ai/provider/v1/models)：本轮匿名 GET 得到 97 条，85 条包含 `/chat/completions`；12 条仅 Messages，72 条同时 Chat/Responses，13 条仅 Chat。数量不写死，未来刷新按实际目录筛选。

目录字段为 `id/object/created/owned_by/name/context_length/supported_endpoints`，没有价格、reasoning、模态或最大输出承诺；`created` 随请求变化，不作为模型发布时间。官方文档承诺流式末尾 usage，但本轮没有用真实账号生成，不能将合成 SSE 测试当成真实帧采样。

六份官方资料已保存 URL、UTC、SHA-256 和原文，位于 `C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-commandcode-20261110/contracts/`。目录快照时间为 `2026-11-10T14:51:29.792679+00:00`。

## 2026-11-10 初始实现

1. 固定生成地址 `https://api.commandcode.ai/provider/v1/chat/completions`，只发送账号自己的 Bearer Key，清除 ambient/caller 身份，禁跳转，不读取 CLI 凭据、不刷新、不回退 `/alpha/*`。
2. 目录固定 `https://api.commandcode.ai/provider/v1/models`，不发送 Key。控制台仍沿用管理身份、secret grant、显式账号或草稿 Key、revision fence 和普通 CAS 保存。目录读取本身不保存 Key 或改动路由。
3. 目录只导入 `supported_endpoints` 数组明确包含 `/chat/completions` 的条目。Messages-only、Responses-only、缺失/未知路由不猜测转换。目录是公开路由元数据，不证明账号权益或推理成功；用户手工模型仍由普通配置机制管理。
4. 公共 Chat／Messages／Responses 的文本和现代客户端函数工具转为 Chat wire，支持 JSON/SSE 和两回合工具。模型 ID 原样发送，不根据 `o1/o3` 前缀改写参数；system 消息逐条保留顺序及文本片段。
5. 复用共享 Chat JSON/SSE 完成校验；失败响应固定脱敏诊断，HTTP 4xx/5xx 保留状态且不自动重放，3xx 转为 502 且不跟随。坏 JSON、截断、错误尾部不能制造成功结束或完成本地计费。取消释放请求并恢复本地预扣，不代表上游退款。
6. Key 创建、替换与 Keep 使用既有秘密槽和稳定账号 ID；Keep 绑定产品/目标。通用识别、后台 discovery job、通用目录和默认连通性探测不接管这个产品。2026-11-11 已补显式专属控制台模型测试：复用生产 sender、精确账号及配置模型／别名、严格有界响应、答案脱敏与取消安全审计；临时／命名／账号保存策略本地验证通过，见 [控制台测试](../progress/2026-11-11-commandcode-console-probe.md)。

## 2026-11-10 初始切片未覆盖

原生 Responses/Messages wire、System One/Jev、浏览器签发 Key、完整 reasoning/thinking、媒体、hosted/custom tools、远端会话/后台请求及 ZDR 控件未实现。

对通用转换已证会丢语义的输入直接拒绝：reasoning 历史、富文本扩展、工具结果 `is_error:true`、旧版 functions、其他产品的历史封套等。公共 Chat normalizer 尚不接受 developer；Responses developer 也明确拒绝，避免降为 user。含被现有 Chat normalizer 修复的 HTML entity 的函数参数历史明确拒绝，普通 JSON 参数原样传递。这些限制属于本切片，不能宣称完整兼容官方 Pi 客户端。

真实账号权限、订阅消费、真实流式响应、独立 feature 构建、浏览器视觉、原生 EXE、Server/Redis 和部署未验。manifest/inventory 保持 `metadata_only`，本地合同测试不自动提升为 live acceptance。

## 2026-11-23 公开合同复核：图片与 reasoning

已读取 [Provider 文档](https://commandcode.ai/docs/provider) 与匿名 [Models API](https://api.commandcode.ai/provider/v1/models)。文档明确接受文本和图片，排除音频、文件和文档片段；模型不具备视觉能力时由上游拒绝，不按模型预筛。匿名目录本次返回 104 条，仅含 `context_length/created/id/name/object/owned_by/supported_endpoints`，没有逐模型视觉或 reasoning 能力字段；数量不作接入常量。公开说明指向各原生协议 schema，但没有给出逐模型的 Chat reasoning 参数或历史扩展合同。

Gateway 原生 Chat 路径允许用户消息中的标准 `image_url`（HTTP(S) URL 或 `data:image/...;base64,...`），保留图片描述字段 `detail`。Gateway 不下载图片，也不根据模型名称猜测视觉能力。此变更不放宽跨协议 Chat fallback、音频／文件／文档或非用户角色图片；原生 Messages／Responses 的现有媒体与 reasoning 能力、账号权限和真实上游结果继续按各自的验收证据判断。总体状态以 [首批收口清单](first-three-closure-2026-11-13.md#1-command-code) 为准。

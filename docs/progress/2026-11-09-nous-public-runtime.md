# Nous Portal：独立产品与公共推理切片

本次继续 `01a1ba79-f536-78d0-92f1-75b10fdc94c4` 的 OAuth／订阅优先目标。
原三图 42 项、红框 23 张卡及聚合子渠道仍全部保留；此文只关闭 Nous 的本地文本／函数工具推理切片，
不是 Nous 完整产品、真实账号或全看板完成证明。

## 实际改动与验证入口

- 新增独立 `nous-portal-oauth` preset、`line-nous-portal-oauth` feature、profile 与 payload 准入。
  加入 `runtime-defaults`；空 explicit pool 不合成凭据、默认模型或猜测的原生 Responses 路径。
- 公共 Responses、Messages 和 legacy Completions 转换为 Portal Chat wire，并按实际 Chat response kind 回流。
  函数调用／结果保持 ID，opaque `org/model` 不触发 XML 文本工具注入；不借 Codex／Claude native shortcut。
- 拒绝 `previous_response_id`、非 null `conversation`、`store:true`、`background:true` 和非 function hosted tools。
  其中 `conversation` 原本会在规范化时静默丢失，已修正为明确拒绝，并验证字符串／对象两种输入。
- 最终 profiled sender 在所有头变更后重新检查 Nous 固定目标、路径、body 和唯一所选 Bearer。
  拒绝迟到 Cookie／Host／外来鉴权头，重建产品 headers，禁共享 Client 默认 headers 与 redirect。
  Subscription、Welcome 两个固定目标均经过真实 loopback sender 检查。
- 公共链路使用真实 LocalRuntime、普通管理 API 创建的 prepaid Key 和余额调整，不绕过 Key／计费入口。
  Nous 账号由合成 routes document 提供；这不是管理 device 登录或凭据保存流程的验收。
  三公共入口的 JSON／live SSE、两回合双函数调用、精确 credential／model／usage／审计、取消退款与未知拒绝均已验证。

合成上游只监听随机 loopback 端口；生产没有可配置任意 origin 的测试接缝。
未读取 Hermes profile、真实 `.ng` 或账号材料，未借用 `hermes-cli`，未进行真实授权或付费推理。

## 实测结果与失败保留

日志根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-magpie-expansion-20261108`。

| 证据 | 实际结果 |
| --- | --- |
| `nous-runtime-resume-green-r2.log` | 编译通过；Nous 库筛选 **41 passed / 4 failed**。四项在夹具 admission 的 `InvalidAccessJwt` 处失败，尚未发送公共请求；此命令整体仍是失败 |
| `nous-runtime-public-wire-r3.log` | 修复夹具后只重跑上述四项，**4 passed / 0 failed**：三入口文本、两回合工具、取消退款、未知拒绝不重放 |
| `nous-runtime-integration-r3.log` | 产品合同 **3 passed**，请求／目标／身份合同 **3 passed** |
| `nous-runtime-adjacent-call-flow-r2.log` | 共享调用流程 **3 passed** |
| `nous-runtime-adjacent-senders-r2.log` | Grok／Kiro／Vertex sender 与 DIM 公共链路 **24 passed** |
| `nous-runtime-resume-checker-r1.log` | checker tests **31 passed**；checker 本身未再变化，复用本次结果 |
| `nous-runtime-resume-ratchet-r4.log` | ratchet 通过：5184 files、14 个 classified immutable runtime assets、governed source >700=0 |

41 项与修复后的 4 项是分批证据，不声称存在一次全绿的 45 项命令，也不与历史 refresh／storage 筛选重复相加。
四项失败的共同原因是新夹具使用空 JWT header 和非法 Base64URL 签名段；只修正合成 JWT，
没有放宽生产 reader、增大 `RUST_MIN_STACK`、跳过身份检查或改变计费语义。

此前 red 仍保留：`nous-runtime-resume-red-20261109.log` 的独立 preset 缺失，
`nous-sender-resume-red-20261109.log` 的迟到 Cookie 未被拒绝。
`nous-runtime-resume-green-r1.log` 是编译失败：提取 sender 后缺显式模块路径；另有现有 DB 测试所需的根模块导出缺失。
修正 `#[path]` 和 crate-private re-export，不修改数据库行为、不扩大对外 API。

## 大小、安全与来源

`streaming.rs` 从 492 降至 **425** effective lines，新 final sender **81**；Nous runtime **132**，
公共 wire tests **171**，fixture **196**，transport tests **123**。DB 根导出 **428 → 429**。
未修改的 `client.rs` 仍为 **499**，本批没有 >500 的源码，不改 baseline／exceptions。
定向 29 份 Rust 的官方 rustfmt check、UTF-8 无 BOM、`git diff --check` 均通过。

来源根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-magpie-expansion-20261109-resume`。
最终源码范围为 `candidate-r6.json`；`final-runtime-r6.json` 记录范围漂移、实际结果与 EXE hash，
历史实际执行的二进制另见其引用的两个 receipt。工作区含其他 owner 的继承改动，
明确 `wholeTreeProvenance=false`，范围证明不等于干净 HEAD／完整发布来源证明。

最终发送不新增后台任务或另一套重试器；沿用既有 Pending、取消与错误传播。
测试源 body drop、账号健康和 prepaid reservation 退款经过实际链路检查。
没有停止其他 owner、清空 target、升级依赖、提交、推送、修改在线实例或数据库。

## 明确保留的后续范围

1. Nous 管理 device session → 接收回执保护 → 用户明确保存 → 精确目录读取／应用 → UI 与 manifest。
   当前 preset 可见及合成 routes 可用不代替该用户流程。
2. `Embeddings`、legacy Completions 的公共 wire、媒体、Welcome 公共推理全流程仍未验；两目标 sender 检查不扩大为这些能力验收。
3. 当前未知拒绝夹具没有反射所选 JWT；不将“不含 JWT”断言冒充上游反射 JWT 的脱敏验收，尚未发现实际泄漏缺陷。
4. feature compiled-out 构建分支未运行；本次验证的是默认启用分支。
5. 真实 Redis／Server、客户端注册资格、真实账号／订阅／旋转／费用、桌面／跨应用均另验。
   没有出手测或正式包，也未做原生启动；Cargo 的 debug 测试构建不当作手测包交付。

下一完整切片优先贯通 Nous 管理登录与保存／目录／UI，再按全范围看板推进其余厂商。

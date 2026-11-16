# DIM 管理入口与逐模型发送：已验证的本地切片

用户明确优先 OAuth／订阅账号，并确认暂时没有允许 Gateway 使用的 DIM client 注册信息。
本切片只使用合成授权材料，不借用参考插件的注册身份，不读取真实 `.ng`。
早期公共 API 诊断发生过一次测试隔离缺陷：Responses 分支误入 Native Codex sender，
将合成 token 发送到真实 DIM 域名并得到 401。没有使用用户凭据或完成付费推理；
这不是账号连通验收，也不能再将全部历史测试描述为仅 loopback。当前候选已排除该分支，
增加 dispatch 前断言、最终 sender 的显式 loopback scope 和非转发 loopback proxy；
最终 r14 候选的合成联合回归已通过，历史误连记录仍保留。
原三图 42 项、五图红框 23 张卡片和聚合子产品仍由完整看板逐项追踪；本记录不是全目标完成回执。

## 当前代码边界

- 国内／海外各有独立 preset、产品入口、issuer、账号池与固定目标；不把同品牌两站账号混用。
- 实际管理 router 已接创建、读取、手动回调、取消、显式保存。Loopback listener 只绑定操作者
  明确登记的端口；PKCE/state、单次 code 消费、session TTL、容量、取消、shutdown/join 复用各自 owner。
- 前端复用 `useManagedOAuth` 的 Secret Grant 续接、版本冲突、显式提交、响应丢失恢复及保存回执。
  授权 ready 不自动保存；userinfo 与已验签 ID token 的保证明确分开。
- 显式“拉取目录并保存到凭证池”使用账号 token 读取 `/v1/models?type=dim`，固定同源、禁重定向、
  有界 decoded bytes 和总超时；读取失败不提交账号。账号秘密及目录一起沿原有 SecretPatch 和
  `commit_document` 的 revision/CAS 通道保存；不另建写库或配置发布系统，不自动生成／测试模型。
- 目录保存为 credential-local `dim_catalog`，绑定 site/client/subject；不写 `verified_models`。
  重新授权保留稳定 ID、操作者元数据、手工模型与其他账号秘密；目录字段不可从父账号继承。
- YAML、DB／Redis 投影以目录的显式协议选择 Chat、Responses、Messages、Gemini，不猜模型名。
  请求局部改变 adapter/path/base，并清除不适用的 Chat／Responses path。没有声明的模型不准入。
- DIM 最终 sender 只允许四种会话目标，统一使用账号 Bearer，清除其他 key headers，拒绝 `key`
  query、目标漂移、重定向、额外 headers 和配置执行器替换。不是开放 Gemini 媒体接口。
- YAML candidate、DB／Redis prepared payload 与管理 probe 使用同一个逐协议 profile 映射；
  原始 target 仍用于完整结构身份核对，投影 payload 只用于发送，不放松健康证据的账号身份检查。
- 管理 probe 使用真实 request planner 和最终 sender 检查声明 wire 的原生完成事件，避免把
  Gemini 的聚合 OpenAI SSE 当原生 Gemini 解析。错误、截断、空回复、工具回复不冒充文本连通成功。

## 已修正的本批失败

1. 保存任务等待 publication lock 时 RAII guard 提前析构，导致过早 `save_unconfirmed`。
   Guard 改为在返回 future 前建立并随 future 持有，完成回执后显式 disarm；未 poll／被 drop 仍保留未知结果。
2. Gemini 管理探测的返回表示不一致，改为检查真实已声明 wire，不修改通用 Gemini discovery 或成功标准。
3. 首个模型路由集成测试误用了 crate-private `document()`；改验公开 compiled credential 不含 discovery。
4. 前端测试 fixture 把 nullable Secret Grant 推断成仅 `null`；修正类型注解，不修改生产授权语义。
5. 目录读取加入 save future 后触发 Rust partial-move 编译错误；先保留非秘密 credential ID，避免再次访问已移出的 prepared document。
6. `dim-local-r2.log` 实际运行 53 tests，50 passed / 3 failed：重新授权模型列表的旧断言未含目录；
   Responses buffered 夹具误把既有 `completed` 当成 `stop`；刷新/重启夹具没有新准入契约要求的协议目录。
   分别修正合成输入及预期，不放宽生产准入、不修改 Responses 完成语义；原失败日志保留。

## 上一轮 r5 验证（早于独立目录／公共 API）

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-magpie-expansion-20261108`。

- 管理入口／保存锁修正的上一候选：`dim-management-r2.log`，47 passed；早于本批目录与发送接线。
- UI：`dim-ui-close.log`，7 files / 49 passed；`dim-ui-typecheck-r2.log`，typecheck EXIT 0。
- Manifest：`dim-manifests.log`，48 checked / 0 errors。
- Checker 自测：`dim-checker-close.log`，31 passed；`dim-ratchet-r5.log`，ratchet passed。
- Formatter／scoped diff：105 个相关 Rust 文件格式检查及 scoped diff 检查通过；
  `dim-close-static-r5.json` 保留静态核对回执，119 个可计源码的最大有效行数为 497。
- `dim-close-candidate-r5.json` 记录当前候选 124 个相关文件的 hash、字节数和 UTF-8 无 BOM 检查。
  范围含既有刷新和共享接线，不是干净 HEAD，也不把其他 owner 的改动算作本批原创。
- `dim-local-r3.log`：完整 `runtime-defaults` 编译和定向联合回归 **EXIT 0**；
  库内筛选 **56 passed / 0 failed / 5556 filtered out**，管理合同 **3 passed**，逐模型路由合同 **1 passed**。
  56 个库内测试包含两个因名称带 `dimension` 而匹配 `dim` 的相邻测试，不能宣称 60 个均为 DIM 专属测试。
  实际管理闭环使用国内站合成 authority；两站隔离另有协议/管理准入合同，不扩大为海外真实调用验收。
- 编译开始晚于 r5 范围快照，完成后 `dim-local-r3-verified.json` 核对 **124 文件，drift=[]**，
  并记录三份实际测试 EXE 的 SHA-256。共享工作区的范围凭据不是整树唯一来源证明。

不调用付费推理，不修改在线实例，不出包、不提交或推送。没有真实 client 注册、登录、续期、
订阅推理、桌面前台视觉、Server 实例或跨应用验收；以上 r5 合成调用不能扩大为公共流／工具回合证据。
独立目录读取与公共 API 的后续本地验证见下，更多产品仍须继续。

## 后续目录／公共 API：r14 本地切片已验证

- 已接独立管理目录读取及前端“读取→应用到表单→普通保存”；读取不写库、不刷新 token，
  请求前后均复核 management、Secret Grant 与 revision，保留普通 SecretPatch/CAS 保存语义。
- 已补普通 Local Key／SQLite 审计和余额的公共三入口合同；四种原生 wire 的 JSON/SSE、两回合工具、
  任意字节分片、取消和 Gemini 完成/历史拒绝均已在最终候选中实际运行通过。
  工具用例包含同名同参的两个调用、逆序结果、逐调用 ID/ordinal 对应及 Gemini 原顺序恢复。
- `dim-public-r1.log` 因默认测试栈溢出中止；增大栈的诊断另暴露 UTF-8 分片夹具、错误 sender、
  opaque 模型名触发 XML 工具降级问题。生产修正复用现有 Gemini 有序历史/完成解析，
  但使用独立 `dim_user_oauth` source 和 DIM 最终发送边界，不借用 Vertex 凭据或身份。
- `dim-public-r2.log` 为修正候选而只取消本批 Cargo，未取得测试结果。
- `dim-public-r3.log` 编译至库测试链接失败，`LNK1104` 无法写入旧测试 EXE，EXIT 101，
  **没有执行本轮联合测试**。同名旧失败测试残留 PID 的创建时间与旧 EXE 吻合；
  精确终止尝试被 Windows 拒绝访问，没有停止其他 owner 的编译或实例。
- 新回归使用命令行 `profile.test.package.gateway.debug=0`，只改变本 package 的调试信息与
  Cargo 产物指纹，绕开被占用的旧文件；不修改仓库构建配置，不增大测试栈，不复用旧 EXE。
  使用正常测试线程栈完成验证，不将大栈诊断当验收。
- `dim-public-r4.log` 为 **65 passed / 2 failed**：原生 Messages 夹具漏发命名 event 行，
  用量观察器没有捕获 message_start 的输入计数，因此正确保留 unknown。按现有原生事件合同补齐
  `event: message_start` 等字段，保留原有 usage、审计和余额断言，没有改生产计费逻辑。

### 最终实际结果与来源

- `dim-public-r5.log`：完整 `runtime-defaults` 编译及联合合同 **EXIT 0**；
  库内 **67 passed / 0 failed / 5561 filtered out**，目录合同 **3 passed**，
  OAuth 管理合同 **3 passed**，模型路由合同 **1 passed**。库内仍含两个因名称带 dimension
  匹配的相邻测试，不把全部 74 个筛选测试称为 DIM 专属。
- 同一份库测试 EXE 的 `dim-public-vertex-r14.log`：**196 passed / 0 failed / 2 ignored**，
  包括 Express/Full Vertex 的公共 API、工具、有序历史、取消和结算；两个显式浏览器 fixture 跳过，
  不算通过。该筛选与 DIM 有一条重叠测试，不能直接相加成唯一测试数。
- `dim-public-dependent-r14.log`：用量捕获、既有 XML 工具桥接及 Native Codex 正向选择，
  **8 passed / 0 failed**。这不是全库回归。
- 独立目录 UI：`dim-catalog-ui-final.log`，**7 files / 46 passed**；
  `dim-catalog-ui-typecheck-final.log`，typecheck EXIT 0。`dim-ui-reuse-r14.json` 核对从 r7
  到 r14 的 24 个相关前端文件无漂移，复用该范围结果，不宣称共享工作区的最新整树前端来源稳定。
- Checker 自测 `dim-public-checker-r10.log`：31 passed；最终
  `dim-public-ratchet-r14.log`：ratchet EXIT 0，5,128 files、14 immutable runtime assets，
  governed source above 700 为 0。`dim-close-lines-r14.json` 的 162 个可计源码最大 **497**，
  over500=[]；未改 baseline/exceptions。
- `dim-public-r5-verified.json`：`dim-close-candidate-r14.json` 的 **167 文件 drift=[]**，
  137 个相关 Rust 文件 rustfmt、scoped diff 和 UTF-8 无 BOM 检查通过；保留实际执行的四份 EXE
  SHA-256。范围凭据不是干净 HEAD 或整树 build provenance，也不把其他 owner 的历史改动算本批原创。

Gemini 当前使用有界原生聚合后输出公共 SSE，**不是逐 token 转发**。取消证明普通 SSE body drop
及 Gemini public request future drop 会释放原生源、留下 cancelled 审计、无最终 usage 并退还预扣；
它不是 DIM 特有的真实客户端 TCP 断连验收。没有真实 client/账号、海外订阅调用、媒体能力、
桌面视觉、Server 或跨应用验收；没有出包、提交、推送或切换在线实例。完整截图目标仍未完成。

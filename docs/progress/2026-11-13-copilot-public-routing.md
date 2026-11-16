# Copilot 显式 SDK 构建的公共单轮文本接线

本块将配置编译器的无条件禁用改为受限资格，不启用默认 feature，不修改运行实例。
真实 CLI／订阅账号、产品包和 Server 验收仍另列。

## 开放条件与现有拒绝边界

- 构建必须显式启用 `copilot-sdk-runtime`；普通构建继续不可路由。
- 专用 credential 必须明确 `enabled: true` 且保存有效 `copilot_github_token`；缺省
  enabled、false 或 token 缺失均不可路由。秘密仍只在原有私密 route document，不投影到
  通用 HTTP payload。没有放松专用容器的空 endpoint／禁止混合认证约束。
- 执行仍要求可信 Local Key、无 PG、冻结身份与 revision；CLI 和 workspace 必须显式
  配置。compiler 不读取进程环境，不声称静态资格已证明 runtime 可用。
- 仅 Chat／Responses buffered 单轮纯文本；流式、历史、工具、媒体、续轮和额外参数
  继续拒绝。现金准入继续拒绝。缺失 CLI 不做发现／下载、不回退到 HTTP 或另一个账号。
- Server／非 Local 身份在既有执行身份边界拒绝；本块不声称它们从模型列表／候选队列
  中被隐藏，也不把该边界测试当作 Server 部署验收。

## 公共验收路径

测试从已有专用 credential fixture 开始，经管理 Secret Grant、SecretPatch 和
expectedRevision 保存秘密／模型配置，然后使用真实 Local Key 调用真实 router。
没有手造 PreparedAttempt 或绕过 eligibility。它不是账号创建 UI 验收。

公开 Chat／Responses 成功必须保留 caller alias、未知 usage／上游状态、准确账号审计，
各单次执行并实际消费消息额度。拒绝请求必须退款且在生成前停止；现金请求无 reservation。
缺失 CLI 请求使用同一公共链，要求固定错误和额度退还。

旧“禁用资格”测试改为明确禁用 credential，仍验证公共拒绝／零 attempt；配置测试覆盖
feature、enabled 三态与 token 有无，而非删除原有安全断言。

## 回执

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-copilot-public-20261113`。
`public.log` 已通过：1 passed，真实公共 Chat／Responses 各一次 SDK 执行并完成消息额度
扣减与审计；另覆盖四种公共拒绝形态。两个合成 child 各发送／shutdown 一次并核对退出，
见 `public-process-exit.json`。

`copilot-regression.log`：25 passed / 0 failed / 17 ignored；ignored 不计成功，包含
尚未在该次调用执行的 opt-in child 测试。实际已独立执行的公共成功／缺失 CLI 测试
使用各自回执。通过项包含真实公共现金拒绝、显式禁 credential、真实 eligibility、
身份边界（无 session／Hosted／DB provider credential 拒绝）和 feature-on 配置开关。
PG 存在时的源码 guard 仍保留，但本轮未把它作为已执行的 Server 公共入口测试。

`public-missing.log`：1 passed，Chat／Responses 缺失显式 CLI 均返回固定错误、退款，
不替换账号或进入 HTTP；该请求的执行准入计数不是已发生的 SDK 生成。
`feature-off.log`：关闭 SDK feature、保留 runtime-defaults 的独立编译，4 passed，确认
静态资格保持禁用；该命令不包含 management-ui，不宣称验证了桌面默认包。feature-on
公共构建 16m55s，feature-off 9m34s，含锁等待／原生编译，不作增量提速承诺。
本块未重跑全部历史 opt-in 场景。

checker 31 项、最终 ratchet、本轮源码 rustfmt 和 scoped diff 通过，UTF-8 无 BOM。
生产配置 owner 80 有效行、compiler 248，新公共成功测试 190、拒绝测试 104；其他本轮
测试文件均小于 250 行，没有改 baseline。源码与 feature-on／off 测试 EXE 哈希分别见
`source-final.json`、`binaries-feature.json`、`binaries-feature-off.json`。

执行所需环境为绝对路径 `GATEWAY_COPILOT_CLI`、已存在的
`GATEWAY_COPILOT_WORKSPACE_ROOT`；不会自动寻找已登录客户端或下载 CLI。SDK 取消后的
保守隔离、真实 CLI 版本／许可、订阅账号、完整公共撤销／取消及原生产品包验收仍开放。
没有提交、推送、出包、部署或真实上游请求，不增加整卡完成数。

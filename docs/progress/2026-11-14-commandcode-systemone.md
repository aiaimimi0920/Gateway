# Command Code System One 接入进行中

公开接口与目录探测已接线，完整 Command Code 回归 100 项及独立 feature 检查通过；
**整卡仍为开发中，不是完整功能收口回执**。
保留首批三个服务商的完整收口目标。

用户随后明确同意本批按公开 Provider API 收口：手工 Key 保留，自动签发及未核实的远端状态
生命周期移入后续合同确认，不判定“不支持”。System One 及其他公开 API 的实现与非账号验收仍须完成。

## 本轮合同核实

2026-11-14 联网复核 [Command Code Provider](https://commandcode.ai/docs/provider) 与其链接的
[TypeSafe System One schema](https://docs.typesafe.ai/api/system-one.md)。采用独立决策入口，
不将 answers 转成 Chat 文本。产品公开三类问题 noul/choice/score；模型能力仍以目录声明为准。
TypeSafe 直连的 assets/box 与 beta header 不能据此当作 Command Code 的已获准能力。
ZDR 必须继续保留用户要求，不能为 Jev 请求取消 ZDR。

## 已写入工作树的内容

- `wire_tests/systemone.rs`：实际 Local runtime、公开 `/v1/systemone`、普通预付凭证、结构化答案、
  模型别名、用量审计与固定上游身份的回归用例；失败时先关闭隔离 runtime。
- `systemone_request.rs`：请求结构和三类题目的校验，以及过滤/额度预估用的文本投影。
- `systemone_response.rs`：题目/答案对应、概率/评分/legend、整数用量与溢出的校验；错误不回显内容，禁止重放。
- `systemone.rs` 与 `http/routes/systemone.rs`：normalizer、计划与公开 handler。
  文本投影不作为发送体；不可逆的聊天策略修改不能静默绕过。
- `systemone_tests.rs`：结构化内容、题数、非法题目、坏答案与坏用量，以及实际 plan 的无损发送、
  固定地址、canonical 策略冲突和 request budget 禁止重放测试。
- `wire_tests/systemone_boundaries.rs`：无效输入不发送/不扣费，坏上游答案退款且不重放，
  ZDR 422 保留隐私 header 和退款，以及 Chat-only 配置不能获得决策能力。

已注册独立 `EndpointKind::SystemOne` / `ProtocolFamily::SystemOne`，序列化名称均为 `systemone`。
公开路由带 body limit 并走原有 pipeline，授权、限流和审计使用真实名称与 `POST /v1/systemone`。
目录导入现保留 systemone-only 模型；目录选择只允许该产品明确声明的能力，不落入默认 Chat
或其他 provider。请求共享固定 sender；JSON 原样返回并使用专用 usage 观察，公开模型别名正常投影。

## 验证与具体阻碍

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-first-three-20261113`。

首次独立 feature 的 `cargo test --lib --no-default-features --features line-commandcode-provider-api`
已终态 exit 101：`src/protocol/freebuff/catalog/execution/body_read.rs:75` 引用仅在
`line-freebuff-web-reverse-api` 开启时定义的 `TEST_BODY_STARTED`。这是另一工作线的测试编译条件问题，
不是预期的 System One 404 红测，也不推翻此前不编译测试代码的独立 `cargo check` 回执。
日志 `commandcode-systemone-red.log` 保留原始失败，本线程没有修改 FreeBuff 源码。

随后改用仓库默认 feature 的同一**单项**测试获得功能失败基线，日志
`commandcode-systemone-default-red.log`：**0 passed / 1 failed**，公共请求实际 404，预期 200；
编译 15m07s，执行 0.34s。这才是本功能的有效红测。

实现后 `commandcode-systemone-green.log`：**9 passed / 0 failed**，exit 0；编译 9m22s，
执行 1.19s。覆盖三个题型的完整公开请求、答案/扩展保真、用量 84+3=87、身份、别名和预付扣费，
以及上述校验/退款/隐私边界。默认 feature 回归不代替独立 feature 检查。

同一默认 feature 测试 EXE 的完整 Command Code 串行回归，在既有
`commandcode_catalog_save_restart_preserves_native_routes_and_account` 测试中发生
`has overflowed its stack`，进程 exit -1073741571，日志 `commandcode-systemone-regression.log`。
不能称为完整回归通过。没有因此改生产逻辑或删除测试。

同一 EXE、仅给测试进程设置 `RUST_MIN_STACK=16777216` 的容量对照回归：**95 passed / 1 failed**，
89.35s，exit 101；重启测试通过。唯一断言失败是旧目录测试仍要求排除 decision-only 模型，
与新增 System One 导入能力不一致。日志 `commandcode-systemone-regression-stack.log`。
后续独立 feature 检查因前序失败而**未启动**，不能写成已通过。

同一 EXE 单独运行重启测试（默认栈）仍发生相同溢出，证明并非测试执行顺序造成，日志
`commandcode-systemone-stack-isolated.log`。已仅在组合测试内对管理/探测 future 使用 `Box::pin`，
减少嵌套 future 的栈占用；没有提高永久栈配置、改生产语义或删断言。目录测试已更新为导入
decision-only 并检查其 `/systemone` 元数据，其他路由拒绝与去重断言保留。

默认栈执行 `cargo test --lib commandcode_catalog_` 已终态：**4 passed / 0 failed**，exit 0，
编译 8m50s、运行 0.92s，日志 `commandcode-systemone-catalog-fixed.log`。重启用例与目录断言均通过。
此前 9 项定向通过的生产代码未因这两处测试修正改变；大栈对照不代替默认栈验证或产品包运行验收。

定向 rustfmt、scoped diff 检查通过；checker 及 ratchet 命令 exit 0，日志分别为
`commandcode-systemone-wired-checker.log`、`commandcode-systemone-wired-ratchet.log`。
七个新增 Rust 文件分别为 87/63/81/158/49/87/40 effective lines；canonical 480→484，
stage_send 495→496，均未越过软限，未调整 baseline。

## 控制台决策探测补齐

`systemone_probe.rs` 已接入目录选择和专用 probe：管理提示作为 state，发送固定 noul 问题，
验证 answers 和 usage 后才生成管理预览；公开接口仍返回结构化答案，不转换为 Chat。
沿用既有预览脱敏、大小限制、取消审计与精确账号选择。

新增探测回归覆盖目录持久化重启、两次精确账号探测、预览 Key 脱敏、实际用量、
坏答案不重放，以及取消后释放响应体且不错误恢复模型健康。新增路由矩阵覆盖产品身份、
目录能力、allowed/forced family 和 `systemone` 序列化名称。

默认栈完整 Command Code 回归已终态：**100 passed / 0 failed**，编译 8m49s、运行 93.08s，
日志 `commandcode-systemone-probe-regression.log`。随后独立 feature 的
`cargo check --locked --offline --lib --bins --no-default-features --features line-commandcode-provider-api -j 2`
通过，exit 0、1m22s，日志 `commandcode-systemone-feature.log`。这不表示此前 FreeBuff 阻挡的
独立 feature 测试编译已修复；产品回归使用默认 feature。
复用同一测试 EXE 的共享 `routing::protocol_resolution` 回归 **37 passed / 0 failed**，
日志 `commandcode-systemone-shared-routing.log`。
新增三个文件的定向 rustfmt 检查通过；checker tests **31 passed / 0 failed** 与 ratchet 均 exit 0，分别记录于
`commandcode-systemone-probe-checker.log`、`commandcode-systemone-probe-ratchet.log`。

## 后续必须完成

公共入口边界补测已新增到 `wire_tests/systemone_access.rs`。首轮
`commandcode-systemone-access.log` 为 **15 passed / 2 failed**：鉴权拒绝和公共请求取消退款通过；
过滤与 body limit 用例在设置夹具时 `Arc::get_mut(...).unwrap()` 失败，尚未执行产品断言。
首次修正仅等待强引用释放，复验 `commandcode-systemone-access-fixed.log` 仍为 **15 passed / 2 failed**，
同样停在 `Arc::get_mut`。仅检查强引用数不足以满足独占修改条件；当前改为在首次管理请求前
初始化过滤与 body limit 配置，不修改生产行为。第二次复验
`commandcode-systemone-access-initialized.log` 已终态 **17 passed / 0 failed**，exit 0，
编译 9m03s、运行 2.74s。结构化 state/instructions/criteria 的过滤与退款、请求体 413、
无 Key/无效 Key 401、公共取消释放响应体及退款均通过。复用同一测试 EXE 的默认栈完整
Command Code 回归 **104 passed / 0 failed**，93.31s、exit 0
（`commandcode-systemone-access-regression.log`），无重复 Cargo 构建。
新增用例后的 checker tests 与 ratchet 首轮 exit 0（`commandcode-systemone-access-checker.log`、
`commandcode-systemone-access-ratchet.log`）；最终夹具与 UI 修正后两项再次 exit 0，见
`commandcode-systemone-access-checker-initialized.log`、`commandcode-systemone-access-ratchet-initialized.log`。

桌面账号目录保存测试已扩展为 System One-only 目录：实际渲染表单、合成 Key 发现与保存回调，
再应用目录到配置，确认 `/systemone` 元数据和手工模型保留且不执行生成探测。
`commandcode-systemone-ui.log` **6 passed / 0 failed**，UI typecheck exit 0
（`commandcode-systemone-ui-typecheck.log`）。这不是原生桌面包验收。

1. 目录专用 probe 已通过合成上游回归；继续收齐管理 UI 与可运行产物中的产品流程证据。
2. 未授权、非 Command Code 候选、结构化内容过滤、取消/重启边界已获得对应回归证据；
   继续收齐当前源码整卡回归与交付验收，不将测试 EXE 当产品 EXE。
3. 当前现金预付要求最终发送体具有可强制输出上限；System One 没有已核实的相同字段，
   不能仅往白名单追加 endpoint。此处作为待解决合同/实现项保留，不据此宣称产品收口。
4. 收齐独立构建、相关共享回归和整卡交付门槛。

未使用真实账号，未出包、部署、提交或推送；未修改运行实例、凭据或数据库。

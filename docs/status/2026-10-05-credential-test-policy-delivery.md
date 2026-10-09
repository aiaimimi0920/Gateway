# 凭据池三级测试策略：本地 Windows 交付

## 完成范围

[本次计划](../plan/credential-test-policies.md)的五个实施步骤已完成本地手测交付。
策略按池 → 身份子池 → 账户解析，账户覆盖优先；父策略保存不批量改写子级。
自动与手动复用模型和用例选择、执行器与结果表示；自动策略必须经配置自动提交
成功才生效。临时用例可以只用于本轮，明确保存后才持久化。

联通成功与标准答案正确率分别记录。错误答案不被当成凭据失效；评级只代表
所选有标准答案用例的表现。没有评分证据不虚构能力等级。

## 手测包与来源

- 版本：`gateway-product-20261005-test-policies-r2`。
- 入口：`Neuro/release/Gateway/gateway-product-20261005-test-policies-r2/gateway-ui.exe`。
- 保留整个版本目录及相邻 `gateway.exe` 和资源，不要只复制 UI EXE。
- ZIP：`Neuro/release/Gateway/packages/Gateway-gateway-product-20261005-test-policies-r2-windows-x64.zip`。
- ZIP SHA-256：`a7eea97954bf1fad1276f303e559889f7e8296ec3cf9177ffac8bbdfc672f934`。
- 基础提交：`7431ebbae206fdfe81dd1bf75740c0fc928eb97d` 加 120 个保留的未提交源码文件；
  `sourceTreeDirty=true`，不是新提交或 clean-tree 正式发布。
- 构建指纹：`20f2e5ccce57292bee86c3134c956f72f3ced1dbe433eb8a24fe9494275de327`，
  本轮重新核对冻结的 3,383 个输入与构建 provenance 完全匹配。
- `gateway.exe`：62,069,760 bytes，SHA-256
  `fb963875f9ce4822eaf13430c8c949bf65eb5a29155507ca089f5fe96c265214`。
- `gateway-ui.exe`：12,430,336 bytes，SHA-256
  `cf997a50a8df76659eb85182c7e39209a6a434fd75007658af0a3bd6bb8e02bc`。

续接开始时，120 个 overlay 文件的当前字节、冻结副本与记录哈希一致。
完整工作区与 archive 快照的差异已分类为 archive 排除、CRLF/LF、示例 routes
和保留的生成报告，未将完整工作区指纹伪称等于构建指纹。

最终 formatter 检查发现并修正四处模块声明排序/链式调用换行。每个修正后的文件
都经 Rust 1.98 rustfmt 对原字节的输出等价校验，随后重新通过全目标编译。
这些纯格式修正和本交付文档发生在构建之后；没有重构建或修改不可变 `r2` 包，
不宣称该包包含交付后的源码字节或文档。

## 验证证据与复用边界

证据根：`linshi/gateway-test-policy-20261004-235613`。
续接证据：其下 `closure-20261005-1120`，最终入口 `completion.json`。

| 范围 | 结果 | 证据性质 |
| --- | --- | --- |
| 正式 Windows 构建 | exit 0，2,098.83 秒；两套生产依赖审计 0 vulnerabilities | 复用同一冻结源码的既有记录，不冒充本轮重新编译 release |
| package / integrity / UI / SQLite / native / ZIP 等包门禁 | 7/7 通过 | 复用 `package-results.json`；本轮另执行交付复制后的 integrity |
| 双 EXE 图标 | 两个 EXE 均有 16–256 px 的 10 组尺寸 | 复用 `packaged-icon-result.json`，交付 EXE 哈希再次相符 |
| 新增与邻近前端测试 | 26 + 42 项通过，typecheck 通过 | 复用 `development-verification.json` 与匹配的源码 |
| 新增 Rust 单元与聚焦集成 | 4 个独立单元测试、17 项集成合同通过 | 复用既有记录；未把旧测试数算作本轮新执行 |
| 模式切换 / 内部滚动 / Escape | 宽屏和 390 px 窄屏布局稳定、临时输入保留，焦点回到“测试” | 本轮重新解析同一 r2 的原始浏览器日志并查看截图 |
| 实际 UI 手动与保存后自动执行 | B 只测选定模型 a 和临时 L2 用例，回复 391、100%；父策略保留 | 复用 r2 的 HTTP 响应、真实 UI 保存及自动执行证据 |
| 错误答案与账户覆盖 | B 的回复 43 对标准答案 42：可联通、0%；A 来源为子池，B 来源为账户 | 同一 r2 的实际模拟上游响应，不是真实账户额度调用 |
| 新的隔离运行 | SQLite 自动调度、2 次 loopback 调用、`/ui/` 200、正常退出 0、无 owned 残留 | 本轮新执行；不推定上轮被中断进程的退出结果 |
| checker / ratchet | 31/31 自测；ratchet 通过 | 本轮新执行，不覆盖原有 tracked 报告 |
| formatter / 编译 / diff | `cargo fmt --all -- --check`、`cargo check --locked --offline --all-targets`、`git diff --check` 通过 | 四处格式修正后重新执行 |
| Neuro 通用规范契约 | exit 0 | 本轮新执行；不表示其他子项目通过 |

本轮实改源码有效行：`validation.rs` 404 → 404，`lib.rs` 50 → 50，
`local_runtime.rs` 112 → 114，`provider_runtime.rs` 65 → 65；两行增长仅为 rustfmt
换行，没有新增职责。此次 overlay 中测量的 108 个源码文件最大 495 行，无软上限
例外；既有 14 个不可变 runtime 资产继续按原策略分类，未修改 checker 或基线。

## 未包含与接续边界

- 未提交、推送、部署、替换旧运行版本，也未复制真实 `.ng` 或凭据。
- 生成测试目前覆盖 OpenAI-compatible chat 与官方 Codex 的实现；不将未实现的
  协议或供应商写成通过。未调用真实供应商账户，未验证真实额度或性能。
- 未运行远程 Redis、Linux 或 macOS 的本次功能验收。
- 本轮只读独立代理返回 `503`，没有有效审查结论；不能写成独立 approved。
- 本地手测包不等于公开安全发布。在线读取的 Issue #3 最新回执仍保留既有
  OSV/Gitleaks findings 边界及 open 状态；本轮没有扫描豁免、关闭 Issue 或发布。
- 下一轮如要做源码发布、真实供应商验收或历史安全清理，应分别确认范围，
  不要重做已通过的本地三级策略功能或再次覆盖此不可变版本。

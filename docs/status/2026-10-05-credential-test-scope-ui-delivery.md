# 测试配置交互精简：Windows 本地交付完成

## 本轮接续与完成范围

按 [本轮计划](../plan/credential-test-scope-ui.md)恢复真实停点，未重做已经验证的
产品功能。此前停点的 Tauri 编译以 `1073807364 (0x40010004)` 退出，没有 Rust
源码错误诊断；无法仅凭该退出码确定外部终止原因。失败日志与原快照记录保留在
证据根下 `build-interrupted-20261005`，没有把失败构建当作成品。

补入已验证的最后焦点修复后重新运行正式构建。本轮没有修改生产行为源码，只
完成冻结快照接续、正式构建、最终包内验证、复制交付和本文档更新。

## 可以直接手测

- 版本：`gateway-product-20261005-test-scope-r1`。
- 入口：`Neuro/release/Gateway/gateway-product-20261005-test-scope-r1/gateway-ui.exe`。
- 保留整个版本目录及相邻 `gateway.exe`、资源；不要仅复制 UI EXE。
- ZIP：`Neuro/release/Gateway/packages/Gateway-gateway-product-20261005-test-scope-r1-windows-x64.zip`。
- ZIP SHA-256：`cc06a3d5dcf973f3700f3816a1c374843a69f20f39d8d31d372d69c934848012`。

基础提交 `7431ebbae206fdfe81dd1bf75740c0fc928eb97d` 加 129 个保留的 overlay
文件，`sourceTreeDirty=true`；不是新提交或 clean-tree 正式发布。
冻结的 3,392 个输入与正式构建 provenance 匹配，指纹为
`2e78dee02875f43cb4cc415c325123b0847081ccda79ab74e009d54b95a049b8`。
交付前再次核对 129 个 overlay 与工作区当前字节一致。使用示例 routes，未复制
真实凭据、`.ng`、运行数据或开发 routes。

| EXE | 大小 | SHA-256 |
| --- | --- | --- |
| `gateway.exe` | 62,050,816 bytes | `0dd450cdb32dd7bd38360796c38b6c29bcbb62626cbcb109e7e68dd87a1023c4` |
| `gateway-ui.exe` | 12,430,336 bytes | `5ee824c4cf96b19d762609050d08e4a055c86edd81b300830f52414ca15993eb` |

## 验证证据及复用边界

证据根：`linshi/gateway-test-scope-ui-20261005/closure-20261005-1224`。
最终入口：`completion.json`。

| 检查 | 结果 | 证据性质 |
| --- | --- | --- |
| 聚焦前端测试 | 17 文件、105 项通过 | 复用上轮最终源码记录；冻结 overlay 匹配，不冒充本轮重新执行 |
| 类型与 formatter | 类型、Rust formatter 通过 | 复用上轮匹配源码的记录；正式构建另重新执行 typecheck |
| checker 自测 | 31/31 通过 | 复用此前记录，checker、policy 和 baseline 未修改 |
| ratchet | 工作区与最终冻结源通过，governed source 超过 700 行为 0 | 最终包前重新执行冻结源 ratchet；原报告未覆写 |
| 正式 Windows 构建 | exit 0，1,531.59 秒；两套生产 Node 审计 0 vulnerabilities | 本轮新执行，不跳过正式 builder |
| 包、integrity、UI assets、SQLite、native、ZIP 和包前 ratchet | 7/7 exit 0 | 本轮新执行，`package-results.json` |
| 原生窗口与 backend child | SQLite healthy、无需 Redis、`/ui/` 200、live 256 px 透明图标 | 本轮新执行，`native-result.json`；零模型调用、无 owned 残留 |
| 最终包内浏览器 | 1280×720、390×844 均不水平溢出，内部滚动、固定操作区 | 本轮新执行，直接使用包内 `/ui/`，未使用开发 preview |
| 保存后三层同时生效 | 全池 21、选中两个子池 23、账户 B 29 分钟；未选子池仍 15 | 本轮实际 UI 保存后读取 active revision；两份合成秘密保留 |
| 手动精确账户 | A / 模型 a / 回复 OK / 100%；B 没有额外手动调用 | 本轮实际 loopback 生成；初始自动两次加手动一次，共三次模拟调用 |
| 权限与焦点 | 过期敏感授权先阻止调用；重新授权后运行；Escape 后回到 `更多操作 fixture-A` | 本轮真实浏览器复测，不绕过 Secret Grant |
| 运行清理 | backend 正常退出 0，fixture/browser 结束，无 owned Gateway 残留 | 本轮新执行，`packaged-ui/ui-runtime-final.json` |
| 复制交付后的 integrity / ZIP | 通过，未覆盖或清理旧版本 | 本轮新执行，`delivery-result.json` |
| Neuro 通用规范契约 | exit 0 | 本轮新执行，不表示其他子项目已验证 |

本功能 23 个相关源码/测试文件的上轮最终有效行测量最大为 387，无软上限例外。
本轮没有新增生产源码职责或扩大旧文件；保留既有 14 个不可变 runtime 资产的
分类和三个 tracked cache/report。没有修改行数基线、批准记录或安全门禁。

限定前端 owner 的独立只读交叉核验未发现明确阻断项，见 `independent-review.md`。
它没有运行测试，也不是后端授权、全部业务或正式发布批准。多对象原子性指草稿
clone-then-replace，不能扩大为跨进程事务；300 秒是停止下一项准入，不是已发送
调用的整链硬超时。

## 未包含与下一边界

没有提交、推送、部署、替换正常运行版本或调用真实供应商账户。远程 Redis、
Linux/macOS、真实供应商额度与既有公开安全 findings 不由本轮证明或关闭。
本轮计划完成不等于全部 Gateway 历史开发计划或 Issue #3 完结。

本交付记录、计划完成状态和产品操作说明更新在构建之后，只是文档变化，没有
修改冻结源或不可变包；不宣称包内文档包含这些后置更新。后续开发从该完成回执
和当前 Git status 接续，不重跑本项已匹配的证据，不覆盖已有版本。

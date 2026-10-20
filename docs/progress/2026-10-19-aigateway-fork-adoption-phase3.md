# AIGateway fork 能力采纳：第三批

批次开始日期：2026-10-19；本地验证收尾跨至 2026-10-20（Asia/Shanghai）。用户授权继续推进有价值能力。本批选择 E01 人工维护闭环；采用 Gateway 自身框架重新实现，不复制 CPA 执行器，不新增独立服务或存储版本。

## 已实施

- 渠道卡片手动清理先调用 preview，展示 ID / 永久分类，默认不选中；用户显式选择后才调用 apply。
- apply 重验可信 driver、路由 revision、完整分类 digest 和选择子集。未知 ID、分类冲突、临时 quota 分类、非法或过大选择均失败关闭。
- 新路径强制先归档，再将成功子集通过一次既有 durable route CAS 移除。逐项报告归档失败、预算耗尽、revision 变化和提交未确认，不用 HTTP 200 伪装全部成功。
- 复用 process lock、Local 文件锁 / Server Redis admission 和既有归档 owner。本地阻塞写任务持有 admission 直到真实 I/O 结束，取消请求不会过早放开同池并发。
- UI 防重复提交、脏草稿、关闭后旧 callback，以及 token / API / preview 身份变化后的迟到结果。丢失响应不盲重试，要求刷新核对。

详细 wire、限制、兼容与不确定性边界见 [凭据池人工清理合同](../credential-maintenance-review.md)。旧直接清理 API 和自动清理保持兼容；只新人工路径获得此预览保障。预览仍调用受信任的外部 driver，不是外部代码无副作用保证。

## 工程与保全

开始时保存 84 项 dirty 内容及 tracked Rust / Desktop 有效行数。新责任独立放入 maintenance、review dialog 和 hook 短模块；没有修改 effective-line baseline。相对开始快照，32 个源码 / 测试文件变化，最大 409 有效行；原有 84 项 dirty 内容保持不变。原始 16 项现场及参考 fork 的 219 个源码、HEAD、branch、index 另行核验保全。

管理鉴权和 secret-free DTO、输入大小、可信分类、revision / digest 绑定、archive-before-remove、部分失败、独立实例 durable CAS、阻塞任务取消和 UI 生命周期均经过专项检查。没有读取 live credentials，没有更换 `.ng` owner，没有改 8317、计划任务、部署实例或其他 Neuro 项目。

## 验证记录

最初测试 fixture 的 cleanup 命名错误已修正，保留失败日志；修正后的红测试确认为新 preview 接口缺失的 `404 Not Found`。实现后，第一轮 6 个后端聚焦测试和 7 个前端文件 / 50 个测试通过；随后补入两项归档期间并发修复 / 独立实例 CAS 测试，纳入最终 Rust 全量回归。

生产 Web entry 和测试-only presentation harness 已使用现有 Rsbuild 配置编译，产物仅写入 `linshi`；未执行会清空 checkout `dist/web` 的发布插件。真实 Edge 验证打开不提交、显式选择、归档失败保留、部分成功回执、焦点恢复、长 ID 换行、内部滚动、128 项上限、空结果、加载和错误态。该测试不连接 Gateway，不能代替 native EXE、真实供应商或部署验收。测试页与临时 loopback server 均已关闭。

最终本地验证结果：

| 检查 | 结果 |
| --- | --- |
| Rust 全量，默认线程栈、`--test-threads=1` | 97 suites；4058 passed / 0 failed / 104 ignored |
| `cargo check --locked --offline --all-targets` | exit 0；仍有 unused / dead-code 等既有 warnings |
| 新增后端维护合同 | 8 个测试均在全量回归通过，包括归档期间的同实例修改与独立实例 CAS |
| Desktop TypeScript | exit 0 |
| 本批相关前端测试 | 7 files / 50 passed |
| Desktop 全量 `src`，`--maxWorkers=1` | 690 passed / 12 failed；不是全绿 |
| 不含本批改动的隔离前端副本 | 674 passed / 12 failed；失败名称与最终源码完全相同 |
| Node scripts | 2122 passed / 0 failed / 2 skipped |
| Python | 358 passed / 0 failed / 4 skipped（总 362） |
| line manifests | 44 checked / 0 errors |
| checker tests / ratchet | 31 passed；ratchet exit 0，未运行 strict |
| formatter / scoped diff check | exit 0 |

未为了结果变绿放宽 test timeout、过滤失败用例或修改原有 12 项前端断言。首次不加 `src` 过滤的 Vitest 命令误纳入 Node `node:test` 脚本；最终前端入口明确为 `src`，Node scripts 由独立 Node runner 验证。并行前端运行曾额外出现一个 5 秒超时，最终单 worker 全量运行未再出现。初次 ratchet 输出绝对路径不被 checker 接受，已改用其要求的 repository-relative `target/` 输出并成功；失败日志保留。

不含本批改动的副本位于 `linshi`，由 tracked Desktop 文件构造：本批修改的文件使用 HEAD 内容，原有 4 项 Desktop dirty edits 使用校验过的工作副本，未添加本批新文件。最初副本运行时 cwd 错误造成额外样式失败，已在副本 cwd 重新运行得到上述 12 个相同失败；不是回滚当前仓库后测试。源码 hash 和失败名称对比保存在证据目录。

Rust 验证已结束，未增加 `RUST_MIN_STACK` 或放宽生产限制；ignored / skipped 不算实际通过。最终 32 个源码 / 测试文件的 hash 与冻结快照一致。由于上述既有前端失败仍存在，整个仓库测试门禁不是全绿，本批不宣称可直接发布或已经完成 native / Server Redis live / 真实供应商验收。

## 尚未实现的独立候选

E02 定向健康恢复仍需精确 credential / model / protocol identity 和原子 expected-state CAS，不能仅因模型探测 Passed 就宣称路由冷却已经解除，也不能让较早启动、较晚完成的 success 清除期间的新 failure。本批只完成源码核验，没有调用真实供应商复现，也没有更改现有 model health 行为。

完整 D01、A04、B01/B02 和外部健康 advisory 等候选仍按第二批说明独立评估；本批没有将它们标为完成。原本不采纳的指纹鉴真、无限恢复、请求外 ticket ping、DOM 补丁和周期重启 workaround 不因本轮授权而加入。

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-fork-adoption-phase3-20261019/`。本批未 commit、push、打包 EXE 或部署；Gateway HEAD 保持 `08634912809fea48b45fd888936cd688356fdfac`。

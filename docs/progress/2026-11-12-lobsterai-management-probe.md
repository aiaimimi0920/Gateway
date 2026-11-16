# LobsterAI 管理测试入口与计划结果

## 本批范围

在上一批专属 console probe 上补真实管理 router 到产品 sender 的本地闭环，未修改生产逻辑。复用现有 LocalRuntime、Secret Grant、测试策略执行器、结果存储和 refresh worker；仅使用合成凭据及不转发的 loopback 上游，不读取真实 `.ng`。

新增两项测试，验证：

- 无 Secret Grant 拒绝生成，外来 credential ID 拒绝，均不向上游发送。
- 单次显式提示词、账号范围临时策略、已配置命名计划、账号保存策略四种来源均成功，账号保存策略报告 `policySource=account`。
- 四次调用均使用固定 LobsterAI 产品路径、精确账号 Bearer、配置模型别名及 256 tokens 上限，不更换账号。
- 管理结果接口可读取持久化命名计划结果，无 Grant 的读取被拒绝；结果读取不新增推理调用，响应不泄漏合成 access token。
- 四条完成审计分别记录精确 credential 和真实 5 tokens，普通预付 Key 仍保留 2 次余额，路由 revision 不变。
- 无保存策略的默认测试、显式错误模型均返回 unsupported，零上游发送。

夹具增加可选测试策略参数；原 `with_quota` 仍传入 None，原有调用方式不变。异步测试通过 Box::pin 控制 future 栈占用，不修改线程栈限制。

## 验证

`cargo test --locked --offline --lib -- lobsterai_management_probe --test-threads=2`：**2 passed / 0 failed / 0 ignored**，测试 0.75 秒；完整命令 1409.19 秒，包含共享构建锁等待及编译，不能当成纯编译耗时。

因本批扩展共用夹具，再运行 `cargo test --locked --offline --lib -- lobsterai_console_probe --test-threads=2`：**8 passed / 0 failed / 0 ignored**，测试 3.56 秒；完整命令 2075.59 秒，包含再次等待共享锁及编译。未重跑整个 87 项集合。

有效行 checker、ratchet、scoped rustfmt 通过。首次直接调用 ratchet 脚本时参数形式错误，退出 2；改用仓库正式 `npm run check:effective-lines --prefix scripts` 后通过，没有修改 checker、baseline 或例外。

本批有效行：测试模块注册 200→201，夹具 211→226，新增管理测试 124。全部低于 500 行，文本 UTF-8 无 BOM；本轮 before 快照及独立 diff 留存，不能将共享 dirty worktree 的全部改动归于本批。

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-lobsterai-management-20261112`。

## 未完成边界

这是实际管理 API/router 与本地持久化的合成上游测试，不是浏览器 UI、真实订阅账号或供应商服务验收。自动调度 timer、管理入口的 disabled 组合、真实费用及自有身份头接受性、Server/Redis、原生包仍另行验证。本批不声明通过管理 API 保存策略的完整编辑流程；测试策略由隔离配置装载。

全范围仍有 67 个工作项未完全关闭，不关闭 LobsterAI 整卡。未提交、推送、出包、部署或改动运行实例。

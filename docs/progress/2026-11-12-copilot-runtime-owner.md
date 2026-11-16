# Copilot runtime owner 入库

2026-11-12，状态 `owner_tests_pass / root_feature_check_pass / http_regression_pass / public_inference_not_connected`。
这次已加入 Gateway 仓库代码，不再仅修改临时实验；不将底层 owner 算作 Copilot 整卡完成。

## 实现范围

- 新增 `crates/gateway-copilot-runtime`，固定官方 SDK 1.0.31，仅启用 runtime feature。
- 根工程增加非默认 `copilot-sdk-runtime` feature，通过 `provider_runtime::copilot` 引用该 crate。
  没有增加 provider manifest、路由准入或 UI，也没有默认启用 Copilot。
- RuntimeConfig 要求既有绝对 EXE／目录；不自动发现、不创建 .ng、不读取 profile。
- 正式 SDK Stdio + Empty、禁已登录回退，移除四个 token 环境变量，关闭客户端遥测。
- 明确 token/model 准入、单 owner 单会话；prepare 后先订阅再启动；工具／内置子代理为空、
  权限回调使用官方 DenyAllHandler。账号选择、代次验证和全局并发限制仍待上层接线。
- 正常 stop 与提前 Drop 都复用 SDK 进程所有权；不另写 kill/reap 或 refresh owner。
  SDK 原因保留为内部错误 source，外部错误／日志仍需 Gateway 脱敏，不直接透出。
- 根 `.cargo/config.toml` 强制 `COPILOT_SKIP_CLI_DOWNLOAD=1`。外部目录调用依赖时 Cargo
  不读取依赖仓库配置，因此 crate README 明确要求自行设置；没有伪称该配置自动传播。

## 依赖变化

新增依赖树要求根锁文件中的三个已有包变化：
`rustls 0.23.45 → 0.23.46`、`webpki-root-certs 1.0.9 → 1.0.12`、`zeroize 1.8.2 → 1.9.1`。
尝试用 Cargo 精确保留旧版本时，解析器分别指出 ureq 3.5.1 的版本下限和 zip 9.5.1 的
zeroize 下限冲突。不是全量 cargo update，也未删除依赖／重装；新增包与完整差异保存在
`lock-delta.json`。后续根工程集成编译及现有 HTTP 客户端定向回归均已通过，不能用独立 crate 测试替代。

## 验证与失败记录

1. 先运行配置回归：默认 SDK 配置及相对路径准入使 **2 项按预期失败**，保存 red.log。
2. 实现显式配置与 owner 后，先修正两处编译问题：PreparedSession 的模块路径，以及
   测试专用 bin 所需 serde_json 的 feature 依赖；没有修改测试来隐藏生产错误。
3. 最终仓库 crate 测试 **5 passed / 0 failed / 0 ignored**：配置、相对路径拒绝、
   显式会话身份／重复会话拒绝／正常停止、缺失 EXE 拒绝、提前 Drop。
   使用 Rust 1.98.0、Cargo jobs=2、锁定离线测试；fixture feature 不是 SDK test-support。
4. 正常停止 PID 65908 与 Drop PID 39488 均由 Windows OpenProcess/WaitForSingleObject
   核对退出；不只依据 Rust Drop 返回。没有启动真实 Copilot CLI 或发付费推理。
5. 官方 formatter、checker tests、effective-line ratchet 与 scoped diff 空白检查通过。
   有效行：provider_runtime.rs **112 → 114**；新 config 67、owner 66、集成测试 62、
   fixture 80、lib 4，均不需要例外，不更新 baseline。
6. `cargo metadata --offline --features copilot-sdk-runtime` 成功；根工程
   `cargo check --locked --offline --lib --features copilot-sdk-runtime` 受共享构建锁阻挡，
   锁由另一条 Perplexity 测试持有。未结束他人任务或换 target 绕锁；根编译未计为通过。
   持续等待后核对 PID／父进程／命令行和无编译子进程，只撤销本轮等待的 cargo；
   回执 exitCode 4294967295 表示人工取消等待，不是 Rust 编译失败。本轮未留下后台编译任务。
7. 同日续跑，在共享锁释放后使用根 target、jobs=2 执行相同 feature 检查，退出码 0，
   工具汇总 `cargo check: 0 errors, 112 warnings (107 crates)`。本次确实编译了根
   `neuro_gateway`，不是仅 metadata 或独立 SDK crate 检查；保留 warning，不扩大清理范围。
   新证据目录为 `C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-copilot-root-20261112-181703`，
   `root-check-tool-receipt.json` 保存命令和工具退出回执；其输出经工具压缩／截断，不宣称完整原始日志。
8. 同一根 target 的 HTTP 客户端回归 **28 passed / 0 failed / 0 ignored**（6045 filtered out）：
   `cargo test --locked --offline --lib --no-default-features --features runtime-defaults,copilot-sdk-runtime
   --config profile.test.package.gateway.debug=0 -j 2 http_client:: -- --test-threads=2`。
   覆盖现有 CA 输入、客户端构造、代理／重定向、断线不重放、超时和 cookie 身份隔离测试；
   不是完整 TLS 握手或真实上游验收。编译 17m 09s，测试运行 4.21s，完整命令耗时约 1034.64s；
   不把本次新增依赖组合的编译时间当作热增量速度。`http-tests.log/json` 保存完整输出与退出码 0，
   Cargo.lock 测试前后 SHA256 一致。未改生产源码或补造测试，未重复独立 crate 的 5 项已通过测试。

证据：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-copilot-owner-20261112`。
独立 crate 构建复用此前 SDK 实验缓存；不是绕过根工程锁另编整个 Gateway。
`green-r4.log/json`、`gates.json`、`lines.json`、root-check 日志与本轮 scoped diff 保留。
新源码／配置／文档使用 UTF-8 无 BOM，旧 dirty edits 不撤销。

## 后续仍需完成

根工程 feature 编译与共享 HTTP 客户端定向回归已完成；接下来接管理凭据、代次校验、模型目录、
公共文本请求的准入／事件转换、审计结算和取消。当前 SDK Session 仅是受信任内部接口，
不能直接暴露给 HTTP 用户。无真实 CLI 权限隔离、挂死／后代进程回收、订阅费用、
工具续轮、Server 或原生 EXE 验收。67 项未完全关闭；不提交、推送、部署或出包。

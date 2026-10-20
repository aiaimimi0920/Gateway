# 2026-10-20：Basispoints 与独立站点健康交付记录

## 交付范围

已实现 Codex 凭据的可选 Basispoints 通道，以及独立于账号测试的站点健康自动识别、
advisory 和显式 restrict。用法、五类固定模板、安全策略及限制见
[`../basispoints-and-site-health.md`](../basispoints-and-site-health.md)。
没有复用 CPA 的 Go 插件 ABI，没有生成第二份账号；配置保持可选字段、旧配置默认值，
没有升级数据库或凭据存储版本。

运行 CPA 的只读证据见[计划记录](../plan/2026-10-20-basispoints-site-health.md)：
插件已启用且进程已加载对应 DLL，原生模型与两个显式别名并存。
这不是所有请求实际经过该通道的证明，本轮没有读取请求日志或发起真实模型调用。

## 本地验证

| 检查 | 结果 |
|---|---|
| 新增后端合同测试，五个集成测试文件 | 21 passed，0 failed |
| 前端聚焦测试，五个文件 | 21 passed，0 failed |
| TypeScript typecheck | 通过 |
| Web 生产构建 `build:web` | 通过；不是 Windows EXE/安装包构建 |
| `cargo check --locked --all-targets` | 通过，有 unused/dead-code 警告 |
| `cargo fmt --all -- --check` | 通过 |
| 有效行数检查器测试 | 31 passed，0 failed |
| 有效行数 ratchet | 通过；本轮源码均不超过 500 有效行，无新增例外 |
| `git diff --check` | 通过；三个原有文件存在 CRLF 提示，未为此重写 |
| 新编译 debug EXE 的隔离进程冒烟 | 7 项检查通过；显式 linshi 数据目录、仅 loopback、正常 drain 退出 |
| Rust 完整 library 单元测试（单线程） | 3484 passed，0 failed，36 ignored |

合同测试覆盖真实 loopback HTTP、OAuth/product headers、SSE 完整终态与工具还原、
全部健康模板尝试、无账号密钥探测、DNS/IP 策略、管理鉴权、配置变更发布 fencing、
已知低成功率/未知策略、真正发送模型及默认模型、两种 send mode 和 fallback 索引。
成功刷新后，低成功率模型在 attempt 之前被跳过，健康模型从缓存放行，路由不追加探测，
手动重复检测返回 429；站点刷新不向模型服务发送账号测试请求。
独立进程冒烟还验证了真实启动路径会自动发现并发布站点缓存，而不依赖手动刷新；
使用的是隔离的新 SQLite 数据与伪凭据，没有打开用户的 `.ng`。

### 没有隐藏的失败

首次默认并行的 library 测试为 3477 passed、7 failed、36 ignored；失败集中在
ChatGPT Web stream 测试，报 `chatgpt_web_solver_busy`。同一源码按仓库推荐的
`cargo test --locked --lib -- --test-threads=1` 复核后为 3484 passed、0 failed、
36 ignored，运行耗时 261.13 秒。保留首次失败记录，没有为本轮功能修改共享 solver。

扩大的前端回归为 50 passed、3 failed；三个失败分别是分组空态文案、重复「编辑」按钮
选择器，以及默认分组 ID 断言。关闭本轮新增站点组件和编辑 hook 后，三个断言仍以
相同错误复现。没有保存任务开始时的完整源码副本，因此不把这个对照包装成精确的
before/after 全量证明，也没有为本轮功能修改无关的分组行为或测试断言。

## 证据与交付边界

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-basispoints-site-health-20261020`。
关键记录为 `contracts-complete.log`、`ui-contracts-focused-final.log`、
`ui-contracts-final.log`、`ui-feature-off-comparison.log`、`unit-tests-serial.log`、`check-all-targets.log`、
`effective-ratchet-final.log`、`isolated-runtime-smoke.json` 与 `source-receipt.json`。
收据记录任务开始时已有 dirty 路径的
哈希、本轮接线覆盖范围，以及单独分类的生成测试缓存；没有清空或重置原有修改。

本轮没有真实上游/质量验证、发布构建或安装包运行门禁，也没有升级/重启正在运行的
CPA 或 Gateway，没有提交或推送。Basispoints 图片/文件上传、后台任务和隐式续接不在
首版支持范围；流是完整校验后的事件回放，不是低首字延迟实时流。

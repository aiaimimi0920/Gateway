# R03：Grok Web 正常网页材料保存与精确账号测试

日期：2026-11-12（Asia/Shanghai）。工作区 `C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`；HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`。本批只新增 opt-in 测试宿主和父模块接线，未改生产实现或前端，不是 Grok 整卡或真实平台验收。

## 执行链与结果

真实 Edge 隔离页面 → 正常 `main.tsx` Web 构建 → 同源回环透明代理 → 正常管理 TCP 登录/Grant/CAS 保存与精确 probe → 合成身份 GET/WS peer → SQLite 审计与正常集中测试结果页面。无真实凭据、真实平台请求或在线实例操作。

1. 编辑 `grok-pipeline / exact`，读取合成材料后取消：上游调用 0，revision 未变。
2. 重新导入并保存 SSO token、完整 `sso/sso-rw/cf_clearance` Cookie 和 UA。verify、confirm-secret-access、route-config 返回 200；完整 typed 文档匹配，revision 已变，上游仍为 0。
3. 正常账号测试取消继承、清空默认题集，仅加入一个合成 prompt/标准答案；运行一次，不保存策略或启用自动测试。仅一个管理 probe POST、一次身份 GET 和一次 WS Upgrade。
4. 正常服务商测试入口的 IQ 页显示 `public-alias / exact`、1/1 题正确、合成测试指数 100；额度及比值均为 `—`。100 仅证明合成答案匹配，不是模型智力、质量或权益证据。
5. 宿主断言实际出站 Cookie/UA 为新材料且没有 Authorization；三个客户端事件依次为 session.create、用户 item、response.create，session 模式为 fast，私有 prompt 原文准确。
6. 唯一 completed 审计属于 exact，native model 为 `grok-chat-fast`，consoleModelProbe=true，accessKeyId=null；usage 三项均未知，凭据/UA/私有 prompt 不进入审计。完整配置比较保证其他账号未变。

普通编辑器既有模型元数据写入也纳入精确预期：enabled、model_selection 和 provider 模型并集；Grok session_auth 使用 cookie/sso/sso-rw，不照搬 Qwen bearer。没有放宽生产校验或忽略整个配置来通过测试。

## 检查与证据

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-grok-probe-ui-20261112`。

| 检查 | 结果 |
| --- | --- |
| Rust 编译 | locked/offline、runtime-defaults、4 jobs；cargo-build-v2 退出 0 |
| 公共 pipeline/TCP | 6 passed；新 UI 宿主默认 ignored |
| 精确管理 probe | 2 passed |
| UI 宿主显式执行 | 1 passed；去重合计 9 项 |
| 前端 | 正常 main 隔离构建通过；输入与上一批 Qwen 已通过的 67 项测试/类型检查版本完全相同，未重复运行 |
| checker/ratchet/格式 | checker 31 passed，官方无报告 ratchet、rustfmt、限定 diff whitespace 通过 |
| 规模 | 父模块 249→251，新 browser_host 182 有效行；无 baseline/exception 修改 |

从 Cargo compiler-artifact 获取真实 EXE，复制并核对后运行 `grok-ui-tests.exe`，SHA256：`84ae3b7c73056b8b72c5787740995665577f72c0e6a7abe763299d9ad8ba7220`。这是测试 harness，不是应用 EXE 或 devtest 包。

主要证据：`material-cancel.json`、`material-saved.json`、`probe-observed.json`、`browser-http.jsonl`、`browser-result.txt`、`browser-result.jpg`、`browser-runtime-v1/completed.json`、`pipeline.log`、`probe.log`、`browser-host.log`、`resource-final.json`。

首次编译会话在续接后不可访问，日志没有 build-finished，未计为通过，原日志保留；第二次有完整成功凭据。编译期间两个 Nous 测试文件有并行变动，最终复核另发现 provider_credential_probe_scheduler.rs 已变化，均记录在 rust-input-drift.json。本批结果属于已冻结哈希的测试产物，不宣称当前全工作区输入冻结或这些并行修改已验；Grok 本批直接修改文件及最终前端输入未变。

宿主 graceful shutdown/join、SQLite 与合成 peer 关闭后写 passed 回执；Node 代理正常退出，最终两进程及两监听均无残留。

## 剩余边界

未重建 runtime 验证持久化重启恢复，未验证普通余额 Key 扣额/退款、真实 TLS/WAF、signin/MFA/challenge、refresh、其他模式权益、多轮/工具/模态、Server/真实 PostgreSQL 或独立应用包。未联网重核当前平台合同，不把已有冻结参考当今日官方保证。

新增模板仍 17，无新增整卡 offline_ready；P50/P60/P70 未完成。无提交、推送或出包。后续不再将本批正常网页保存/精确测试链列为空白，但不能外推上述未验边界。

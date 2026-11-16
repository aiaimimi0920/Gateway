# FreeBuff CLI 浏览器批准：独立协议 owner

日期：2026-11-06。状态：本子批定向验证通过；扩大 Python 合同保留一项既存失败，不是全仓绿，也不是 R06/W04、正常管理登录或总计划完成。
仓库：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`，`main`，HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`；保留既有未提交修改。
证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-login-20261106`。

## 1. 实现与依据

按已冻结的 FreeBuff session 产品接入，不创建假 API Key 或 OAuth 池。重新联网获取官方公开 `CodebuffAI/freebuff @ 35c6bb288d2af2efc85616c572774fd8ab47e3db` 的 10 项源码/许可及 tree，11 项 HTTP 均为 200，bytes/hash 在 `public/receipt.json`；没有执行参考项目或调用真实认证 endpoint。

- 新 owner 为 `src/protocol/freebuff_login.rs`，按 challenge、材料与 transport 拆分，使用现有 `line-freebuff-web-reverse-api` feature；没有新依赖或配置项。
- 创建只向固定 `https://freebuff.com/api/auth/cli/code` 发送一次 JSON `fingerprintId`；status 只查询同 origin `/api/auth/cli/status`，不带已有业务 Bearer/Cookie/API Key。
- `expiresAt` 原字符串回传，不按本机墙上时钟解析或改写 HMAC 输入。相对 `expiresInMs` 建立 monotonic deadline；缺省沿固定 CLI 的一小时等待上限，预算自 POST 开始计，不重开网络已耗时。
- 第一次 status 立即执行；401/合法空 user 表示 Pending，后续至少间隔 5 秒。API 只提供单步 future，没有新增账号级无限轮询 task。
- 请求与解码后 body 共用最多 15 秒及 challenge 剩余期限，body 最多 64 KiB；禁止 redirect 和自动 retry。客户端复用 Gateway 统一代理 factory，测试只访问随机 loopback。
- 合法 HTTP 2xx 仍需严格 JSON/材料准入；空响应、坏 user、跨 attempt fingerprint 均不得产出凭据。成功领取后 capability 消费，不重复领取；不推断 token expiry、refresh 或业务 agentId。
- 材料和 challenge 不可 Serialize/Clone，Debug 固定脱敏，错误仅为本地固定码；不把 authToken/hash 放入日志或公开 document。

浏览器链接精确同源限制和返回 fingerprint 严格相等是 Gateway 的本地安全策略，不是上游已经证明的保证。四类边界、许可采用方式、参考差异及在途取消语义详见 [REFERENCE](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/freebuff_login/REFERENCE.md)。

## 2. 失败与修正

1. `red-absent-owner` 实际失败为 Rust `E0583`：模块尚不存在。这是缺 owner 的编译红测，不是已执行的行为红测。
2. 首次 `green-protocol-v1` 编译期间，交叉审查发现仅接受 200 与公开 CLI 的 `response.ok` 不符，并确认需要统一代理入口。核对 rustc PID、Cargo parent 和精确测试命令后，只取消本批 rustc；回执为 `cancelled_before_tests`，后续 Cargo exit 101，不计为通过或行为失败。取消记录和旧日志保留。
3. 接受有效 2xx、保留严格 schema/材料检查，并改用 Gateway factory、仍显式 retry never 后，开始 `green-freebuff-v2`。新增 non-200 正例及空/坏 2xx 负例，不用真实材料。
4. 一次性静态 wrapper 对 rustfmt 成功时的空输出错误传入 `WriteAllLines`，记录为 `Value cannot be null. (Parameter 'contents')`；仅修 wrapper 数组捕获，不改生产代码。
5. 扩大 HTTP factory Python 合同 4 passed / 1 failed：既有 scanner 把 `src/provider_runtime/http_probe.rs` 的 `#[cfg(test)]` 下显式 no-proxy fixture 也当生产原始构造器。该 fixture 非本批修改；没有通过改变生产或删除原断言掩盖。FreeBuff 的独立 factory/retry 回归另行通过。

## 3. 最终验证

所有 Cargo 命令均 `--locked --no-default-features --jobs 4`，测试另加 `-- --test-threads=1`；TEMP/TMP 使用本批 `tmp`，未设置 `RUST_MIN_STACK`，不触发 management-ui 发布/清理 hook。

| 阶段 / 选择 | 结果 | 实测编译 / 测试 |
| --- | --- | --- |
| `green-freebuff-v2`：`--features runtime-defaults --lib protocol::freebuff` | 126 passed / 0 failed / 0 ignored；14 项新增协议测试 | 26m58s / 76.24s |
| `production-check`：`check --features runtime-defaults --lib` | exit 0 | 3m27s |
| `feature-off-tests`：`--features line-gemini-web-reverse --lib protocol::freebuff_login` | 6 passed / 0 failed / 0 ignored；含实际关闭拒绝测试 | 19m51s / 0.07s |
| `feature-off-production-check`：`check --features line-gemini-web-reverse --lib` | exit 0；代表性关闭，不是全 feature 矩阵 | 3m38s |

按完整测试名去重 **127 个不同 Rust 测试通过，其中本批新增 15 项**；关闭模式的五项共同测试不重复累计。四阶段各冻结 3039 项选定 Rust/配置输入，均 `changedInputs=[]`，联合收尾再与当前字节核对。

Rustfmt、检查器 31 项、最终 ratchet（4541 项、14 项 immutable runtime artifacts 分报、governed source 无 >700）、本批 factory/retry 专用 Python 回归和 `git diff --check` 已通过；扩大 Python 的一项失败按上一节保留。没有改 baseline/exception，原有行数报告已按字节恢复。

受影响 10 项源码最大 178 有效行；原 `protocol/mod.rs` 为 104→105，Python factory 合同为 95→101，其他八项 Rust 为本批新增。审查覆盖固定目标/输入、秘密隔离、body/deadline 预算、取消与单次领取、HTTP/任务退出边界；没有新增线程、后台 poll task、依赖、storage version 或数据迁移。

最终联合来源、严格 UTF-8/无 BOM/空白、本地链接和 diff 检查见 [closeout.json](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-login-20261106/closeout.json)，阶段回执和失败现场均保留。所有选定 Cargo 已正常退出；本批没有常驻运行服务。

## 4. 未完成与接力入口

本批只是创建/查询/取消和 typed receipt 的协议边界，尚无正常管理会话 owner 或 UI 登录入口，不把独立 owner 当完整登录流程。

下一批沿既有管理模式接入：真实管理身份与 Secret Grant、精确 provider/preset/原 revision、实例会话限额/TTL/取消与 shutdown；随后服务端独占 receipt 进入正常 SecretPatch/CAS 保存，fingerprint/hash 必须由秘密 owner 管理。已有 token/agentId 导入和业务 session/run 不自动接收这份 receipt，也不能推导 agentId 或复用 Codebuff Key。

完整 agent map、lease/取消、公共推理/精确账号 probe、正常 UI/服务/重启与产品权益仍需后续集成。R06/W04、其他原始渠道、P50 全范围无凭据集成/devtest、P60 集中录制和 P70 真实逐渠道验收均保留；模板累计仍 16，不新增整卡 `offline_ready`。

本批没有读取真实 `.ng`/历史凭据、真实平台或付费调用、容器操作、出包、替换运行实例、提交或推送。P34 单容器许可早已消费清理，不再次复用。

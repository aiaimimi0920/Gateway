# R03/R07：Grok 与 Qwen 会话材料本地持久化恢复

日期：2026-11-12（Asia/Shanghai）。工作区 `C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`，HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`。只新增 `tests/console_web_session_restart_contract.rs`，没有生产修改、存储版本升级或用户数据迁移。

## 本批补齐的边界

前两批正常管理网页已验证材料保存、精确账号调用和结果展示，但其宿主复用 FakeRedisBackend，不能据此证明生产本地 authority 重启恢复。本批使用生产 `RouteConfigRuntime::new_local`、真实文件 archive/YAML/local-active/journal 和恢复 coordinator；既有 RuntimeHarness 只提供隔离目录与中断阶段夹具，其 fake runtime/backend 不参与被测提交或恢复。

每项同时覆盖 Grok Web 与 Qwen Web。材料全部为 synthetic，包含稳定 exact/other ID、token、完整 Cookie、UA、alias/native model 和产品专属 session_auth；Grok 是 cookie/sso/sso-rw，Qwen 是 bearer。

1. **旧格式读取**：缺 session_auth 的已有材料可读，bootstrap 不重写原 YAML；销毁 runtime 后重新加载，账号和材料不丢失。
2. **轮换保存与重复启动**：用正常 commit_document/CAS 保存新 token/Cookie/auth；释放旧 runtime/store 后连续构建两次新 owner，revision、完整归档与 YAML 文档一致。实际 route candidates 的账号、native model 和 payload 与预期一致，不只检查文件存在。
3. **防旧写入与明确移除**：旧 revision 写回旧材料返回 console_revision_conflict；正常移除 exact 后反复启动不从旧归档复活，other 账号保持不变。这里是移除账号，不冒称 token 空字符串语义已验。
4. **发布前中断**：已有事务夹具执行真实 YAML 原子替换后停止；本地 authority 尚未发布新 revision，恢复后回到完整旧材料并将事务标为 Aborted。
5. **发布后中断**：夹具模拟 local-active 已发布、journal 尚未完成；恢复后保留完整新材料并补为 Committed。两种中断都再启动一次确认幂等，旧/新归档均保留。

中断是磁盘阶段注入，不是杀进程或物理断电；重启是同一测试进程中销毁并新建 runtime/store，不是应用 EXE 重启。无浏览器重复操作、上游网络或真实平台生成。

## 验证结果与来源

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-web-session-restart-20261112`。

| 检查 | 结果 |
| --- | --- |
| 新增合同 | 3 passed，每项覆盖两渠道；0 failed/ignored |
| 相邻 console_local_runtime_contract | 7 passed；0 failed/ignored |
| 构建 | locked/offline、runtime-defaults、4 jobs，两 integration target；build-v2 退出 0 |
| checker / ratchet / 格式 | checker 31 passed，最终 ratchet 与 rustfmt 通过 |
| 规模 | 新文件 238 有效行，无既有文件增长，无 baseline/exception 修改 |
| 来源与资源 | inputs-v2 最终无漂移，独立复制的测试程序执行后无残留进程 |

从 Cargo compiler-artifact 获取实际 EXE 并复制，执行的是证据目录内固定副本：

- `console_web_session_restart_contract.exe`：SHA256 `f6babedb80c29922079cc6d433ac9f5eac4b53991eee90b5e1c103f370553c24`。
- `console_local_runtime_contract.exe`：SHA256 `6002dd7006877745777946952088e19c554f61a856789f6386f1be9e183bd608`。

首次编译暴露 4 个 E0624：集成测试误调用 crate-private snapshot.document。仅调整测试，通过公开 route candidates、完整 archive 和 YAML 比较验证，不扩大生产可见性；原失败保留在 build.jsonl。最终无源码漂移，未将初次失败计为通过。

TEMP/TMP 显式指向本批 linshi/runtime，既有 fixture 的 Drop 只清理这些隔离测试目录。未访问真实 .ng、credentials 或在线实例。新文件 diff whitespace 检查无输出（no-index 的 1 表示存在新增文件差异，不是空白错误）。

## 仍未完成

本批收拢两渠道的本地配置 authority 持久化/恢复合同，不把此前 FakeRedis 网页链和本批测试拼称为同一个完整 UI→真实 local→应用重启验收。仍缺独立应用进程/包、重启后真实上游调用、Server/真实 PostgreSQL、普通 Key 扣额/退款、真实会话/权益/当前平台协议与其他渠道能力。

新增模板仍 17，无新增整卡 offline_ready；P50/P60/P70 未完成。无真实凭据、供应商调用、提交、推送或出包。

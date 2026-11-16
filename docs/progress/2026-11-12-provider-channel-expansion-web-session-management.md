# R03/R07：管理 TCP 保存与真实本地 authority 恢复联合

日期：2026-11-12（Asia/Shanghai）。工作区 `C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`；HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`。仅测试新增，无生产实现、数据库版本或前端修改。

## 本批执行链

真实随机回环 TCP → 正常 build_router/ConnectInfo/管理认证 → 正常 secret Grant 与 secretPatches → PUT route-config/CAS → 生产 RouteConfigRuntime::new_local 文件 authority → graceful shutdown/join 并释放旧 AppState/runtime/store → 新监听和新管理状态从磁盘恢复 → 正常脱敏 GET 与实际路由候选核对。

沿上一批 `console_web_session_restart_contract` 的精确合成 Grok/Qwen 文档复用夹具，只新增 management.rs。RuntimeHarness 提供独立磁盘目录；其 fake runtime/backend 不参与被测提交和恢复。管理 AppState/auth 构造复用既有测试 support，Redis 是未连接的惰性池，没有启用后台 worker，也没有建立完整 SQLite 业务 runtime。因此本批是配置 owner 联合，不是完整 build_app_state 或应用启动验收。

每个渠道执行以下流程：

1. 无管理 token 的 PUT 返回 401；有 token、无 Grant 返回 403/console_secret_access_required。原 YAML bytes 和活动 revision/路由不变。
2. 通过正常 confirm-secret-access 获得 Grant；新 token、Cookie 和产品专属 session_auth 经 secretPatches 替换，其他账号用 keep。正常 PUT 返回 200、新 revision；GET 与预期脱敏文档完全一致，不包含完整合成秘密。
3. 关闭并 join 旧 HTTP 服务，确认只剩最后一个 AppState 引用后释放；新服务重新调用生产 new_local/recover_startup，不继承旧 runtime/store。
4. 新服务读取保持同一新 revision、精确账号、native model 和凭据 payload。上一管理 auth owner 的旧 Grant 被拒绝；使用新 Grant 但旧 revision 返回 409，两种失败均不改变 YAML 或已轮换材料。
5. 新服务再正常关闭，沿上一批完整 archive/YAML/路由比较连续重建两次，最终状态一致。

两渠道均只使用 synthetic 材料，没有上游请求、真实凭据、真实 .ng、在线实例操作或浏览器重复验收。旧 Grant 检查基于本批新建的管理 auth owner，不宣称已覆盖生产 auth 文件恢复或完整登录会话生命周期。

## 验证与来源

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-web-session-management-20261112`。

- `cargo test --locked --offline --no-default-features --features runtime-defaults -j 4 --test console_web_session_restart_contract --no-run --message-format=json`：退出 0。
- 冻结测试副本执行 4 项全部通过：本轮管理 TCP 联合 1 项（覆盖两渠道），以及原持久化/发布前后中断恢复 3 项。没有重复计入上一批独立相邻 7 项。
- 从 Cargo compiler-artifact 获取实际 EXE，原件/副本哈希一致；副本 `tests.exe` SHA256：`96e89bf2b73a6dfe4c245946e2d77c45b5ed26d116e6b5d8687fb74330dce82c`。
- checker 31 passed，官方无报告 ratchet、rustfmt、限定 whitespace 检查通过。父文件 238→240，新 management.rs 211 有效行，未改 baseline/exception。
- `inputs.json` 对最终源码与直接测试支持输入无漂移；`process-final.json` 无本批残留测试进程。HTTP 服务均通过 shutdown/join 收尾。
- TEMP/TMP 指向本批 linshi/runtime，fixture 清理仅发生在该隔离目录；文本 UTF-8 无 BOM。

## 不外推的边界

本批将正常管理 TCP 保存与真实 local authority 重建连接起来，但没有把先前网页宿主、当前管理测试和上游测试拼称为同一次完整端到端运行。仍缺实际网页→真实 local 保存→应用 EXE 重启→上游调用的完整链、独立进程/包、真实 Server/PG、余额/扣额、当前平台会话与权益验收。

新增模板仍 17，无新增整卡 offline_ready；P50/P60/P70 继续。无提交、推送或出包。

# FreeBuff：本地安装设备秘密、显式注册与签名目录恢复

日期：2026-11-06。工作区 `C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`，`main` / HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`。大量 inherited dirty edits 保留，HEAD 不是本批源码或交付凭据。本文只承接上一批正常管理只读目录，不表示 R06/W04、整卡 offline_ready、P50/P60/P70 或总目标完成。

本批证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-device-runtime-20261106`。本地持久 owner/管理注册/签名目录这一限定后端子批已通过最终定向验证；不是完整注册 UI 或渠道生命周期完成，来源与范围以本批 `closeout.json` 为准。

## 1. 新增的完整后端路径

正常管理新增 `POST /v1/internal/gateway/console/freebuff-device-registration`。请求与只读目录相同，另有可选的单次未知结果重投确认：

```json
{
  "providerId": "freebuff-pool",
  "credentialId": "stored-account-id",
  "expectedRevision": "current-saved-revision",
  "businessOrigin": "https://codebuff.com",
  "client": "cli"
}
```

复用正常管理 token、Secret Grant/Origin、精确 provider/credential 与 compiled payload、原 typed 登录受众校验和 revision fence。business origin 仍限两个已证 HTTPS origin，client 仅 CLI/Desktop，无默认目标、调用者 token 或任意 URL；未知字段拒绝。无 LocalRuntime 的 Server 明确 `409/freebuff_device_storage_unsupported`，不创建任意本地文件补位。FreeBuff 关闭时先按既有 compiled-out 合同拒绝。

首次显式注册：有效 Bearer bootstrap catalog GET → 再核权限/revision → 安装级 OS try-lock → SQLite writer transaction 内读取/生成唯一 Ed25519 key、提交待确认 marker → **释放 SQLite transaction 后**注册 public key → 再核权限/revision → 验证并密封保存 receipt。网络两步各沿既有最多十秒 deadline、无 redirect/retry、body cap 和安全错误码，不 admission、不购买会话。

后续正常 `freebuff-model-catalog` 读取会按精确 provider/credential、origin/client/完整 token scope 加载已保存 registration，并实际发送 signed GET。没有 ready record 时仍可 unsigned bootstrap；有 ready 但损坏、密钥不匹配或上游拒绝设备时明确失败，不自动降级、注销、换 key 或重新注册。显示 DTO 与路由/模型配置不变。

## 2. 持久秘密与兼容边界

- LocalRuntime 内提取共享 `LocalSecretSeal`，复用原 `access-key-seal.json`；保留 JSON/Base64 AES-256 key、nonce12+ciphertext/tag 格式及旧 access-key AAD、读取上限、digest 与错误 code。access-key 导入/鉴权边界既有限制 token ≤512，原正常 Key 行为不扩大或改变。
- 新表为 additive schema-1 初始化，不 bump storage version、不改 credential YAML 或既有秘密/ID。设备表以 `CHECK(id=1)` 约束一个安装 key；registration 独立表、public FK、最多4096条绑定。
- PKCS#8 与 registration receipt 分别以不同 AAD 密封。receipt 严格版本/字段、protocol scope、公钥和 key ID 校验后才恢复内存 capability，不对 `DeviceRegistration` 开放 Deserialize。
- 本地绑定带 domain 和 length framing，涵盖 provider ID、credential ID、原 protocol 的 origin/client/token SHA-256 scope。安装 key 不使用登录 fingerprint；同 root 账号共用安装 key，但 registration 不按邮箱、模型或相同 Key 文案合并。
- 启动 has-sealed 合计 access-key、设备和全部 registration 记录，包含 nullable marker。仅 FreeBuff 数据时丢 seal 也不重新生成；坏 key/坏密文不当作 absence，恢复原文件后才恢复原身份。
- 这是同数据目录 AES seal，不是 DPAPI、OS keyring、HSM 或内存 zeroization；读取完整数据目录的人仍可能取得秘密。未创建私钥临时文件或另一个明文 key store。

## 3. 结果未知与并发

OS 锁位于 selected data root 的既有锁目录，文件名固定，不按账号无限新增文件，也不删除锁文件释放锁。RAII guard 覆盖 prepare、网络与保存；请求取消/失败或进程退出释放 kernel lock，但已提交 marker 保留。

有未确认 marker 时，普通重复注册不再 POST；响应 `registration:"unconfirmed"` 和非秘密 `unconfirmedAttemptId`。只有管理员**再次显式决定注册**并提交当前精确 `retryUnconfirmedAttemptId` 才可重投同一安装 key。发送前先原子换成新 attempt ID，旧确认不能重复使用；并发请求仍受 OS 锁与 SQLite 条件更新约束。这不是“重投安全/服务端幂等已证”。

已保存 ready 时普通注册只核本地 receipt 并回 `registration:"stored"`，不重新 GET/POST；它表示本地已保存，不证明当前平台权益。POST 后、保存前的 fence 失败留 unconfirmed；**保存成功后最后响应 fence 失败会返回错误，但保留已验证的 durable receipt**，后续精确读取可 reconcile，不能为了返回错误而擦掉已知结果、制造自动重 POST。

## 4. 公开来源

本轮重新联网下载固定 `CodebuffAI/freebuff@6f567ec21542afc07dbe447bce0dcaf7a5a73a8d` 的 [device signing](https://github.com/CodebuffAI/freebuff/blob/6f567ec21542afc07dbe447bce0dcaf7a5a73a8d/common/src/util/freebuff-device-signing.ts)（blob `32d37a38d403f4ff4dce6fdcb27ffff1ed3ced0b`）、[官方客户端测试](https://github.com/CodebuffAI/freebuff/blob/6f567ec21542afc07dbe447bce0dcaf7a5a73a8d/common/src/util/__tests__/freebuff-device-signing.test.ts)（blob `0e74d3647dce589d42725b88a128ef657da21def`），并重新核对 [catalog types](https://github.com/CodebuffAI/freebuff/blob/6f567ec21542afc07dbe447bce0dcaf7a5a73a8d/common/src/types/freebuff-model-catalog.ts)（blob `9ad6d8bb6cb8e05eb334524e43306779e5564119`）。只读、不运行 SDK；沿已核 Apache-2.0 许可与来源清单。

实现明确要求宿主持久安装 key，并按 account/host 缓存 registration；客户端测试同 account/key 重新注册可接受不同 key ID。公开参考未找到注册服务端 handler/uniqueness 合同，不能将客户端重试行为当作服务端幂等证明。GitHub connector 本轮不可用后使用固定 raw URL 下载，保留引用而非猜测当前平台实现。

## 5. 验证、失败与剩余范围

最终阶段：

| 阶段 | 实际结果 |
| --- | --- |
| `related-freebuff-v2` | 70 passed / 1 ignored，包含6项新增管理设备测试及既有只读/登录/TCP本地闭环 |
| `local-v2` | 全部 LocalRuntime 44/44，包含旧 access-key、独立旧密文向量及4项新增设备持久/锁/密封测试 |
| `catalog-v2` | 相邻协议24/24 |
| `feature-off-v2` | 仅移除 FreeBuff、保留 runtime-defaults 其他 features，30/30，含新注册入口 compiled-out 测试 |
| `production-v2` / `format-v2` | 生产 lib check 与官方 `cargo fmt --all -- --check` 通过 |

六阶段各 **3241项选定输入无漂移**，最终阶段输入表相同。Rust 按完整名去重 **139 passed / 1 ignored**，其中本批12项新增，不把重复执行或原 access-key 19项改前基线累加。忽略项是需独立浏览器的旧 UI host，不是真实平台或原生 UI 成功。正常管理使用 production router oneshot、真实 LocalRuntime/SQLite/原 seal 与随机回环上游；不是浏览器→真实管理入站 TCP、Server 或独立进程重启。

Rustfmt、checker31/31、ratchet4606（14 immutable runtime单列，governed source >700=0）通过；18源码（9新增/9修改）最大417有效行、新增最大284，无 baseline/exception 修改。原 root 有效行报告在每次 gate 前直接备份、归档实际输出再恢复，前后 SHA-256 均为 `dc7e58afefbfaa02f4d257ef099af01e91b8fb61a444b517863b3f816bdb3550`。UTF8无BOM、来源/diff与本地链接的最终核对见 `audit.json`。

失败全部保留：两个404行为红测；首轮三处 E0308（实际 AppState 是 `Option<LocalRuntime>`）；v2 的9/1为测试夹具在请求后尝试 `Arc::get_mut`，改为重建管理状态/新 Grant；随后10/10但 `src/.../REFERENCE.md` 文档变化导致冻结输入漂移，不作为最终绿回执；关闭组合的 E0658（proc macro 内文件模块）/错误 Default，移到稳定文件模块并用真实 YAML 构造；审查把 fetch header 断言校正为实际 `x-freebuff-catalog-fetch` 后做最终六阶段。临时 PowerShell 引号、Python统计 SyntaxError 与格式 probe 也单列保留，不影响最终源码/测试语义。不宣称清除旧最小 ChatGPT-only 的17个 Gemini media 关线错误或前批 Python scanner 失败，不报全矩阵/全仓绿。

本批不修改前端；已有只读 UI 在后端已保存 registration 时沿同接口得到 signed read，但**显式设备注册/未知重投的正常 UI 控件仍未接**，不把管理 API 通过当作 UI 验收。当前 unconfirmed 状态读取仍先 bootstrap 目录，后续应拆出不依赖上游 GET 的本地状态查询；ready 失效后的显式撤销/重新注册也未接，不能要求用户手工编辑数据库。持久默认业务 origin/client、catalog 发布、session/completion 最终签名、逐次 admission/reuse/费用确认、typed agent/lease/probe/公共生成仍缺。Server 秘密 owner、真实独立进程/OS 重启与跨进程锁竞争、原生包和平台真实权益分别待验。

不读真实 `.ng` 或历史凭据、不发平台认证/生成/收费请求、不动在线实例、容器或现有卷；不出包、不提交/推送。P34 的 Redis 授权已消费并清理，本批不需要 Redis，也不复用历史授权。

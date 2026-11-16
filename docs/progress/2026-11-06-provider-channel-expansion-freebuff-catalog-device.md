# FreeBuff 当前业务目录与设备注册/签名协议子链

日期：2026-11-06。仓库 `C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`，`main`，HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`。工作树包含大量既有改动，HEAD 不是本批源码来源凭据。

本批承接已验的登录 UI/真实本地管理及 legacy ownership，不重复实现登录或旧 run/session。**当前实现为独立协议子链，不是正常管理/公共业务接通，也不是 R06/W04、P50 或总计划完成。**

## 实现与公开依据

固定官方公开参考为 `CodebuffAI/freebuff@6f567ec21542afc07dbe447bce0dcaf7a5a73a8d`。联网读取协议/类型，不下载运行 SDK、不访问平台认证或生成。

- 浏览器登录 origin 和业务 origin 分开。公开 `freebuff/e2e/README.md` smoke 配置使用 `https://www.codebuff.com` 业务地址及 `https://freebuff.com` 登录地址，CLI session API fallback 为 `https://codebuff.com`。这证明部署地址可分离，**不是正式发布 binary 的内嵌配置凭据**。
- 新 `protocol::freebuff::catalog::CatalogSession` 要求显式选择这两个固定业务地址之一、token 和 CLI/Desktop client。没有默认目标、任意 URL 或从 Gateway GUI edition 推断身份；现有 typed 登录 token 不会自动跨 host 发送。
- bootstrap GET 有 Bearer、catalog protocol/client headers，无已有 registration 时只省略设备签名。有效 catalog 后才允许显式注册，POST 仅 publicKey/client。返回 keyId 与请求时不可变的 token/origin/client/key 绑定；外来 capability 在 I/O 前拒绝。
- `DeviceKey` 显式导入 PKCS#8 DER，支持编码 v1/v2，派生公钥必须与另外提供的 raw32 公钥一致。不生成、替换或落盘安装 key，也不把登录 fingerprint 当签名 key。官方 key record schema 仍为 version:1。
- 生产签名发送目前仅接 catalog GET。内部 primitive 覆盖 method/path/timestamp/精确 body hash/fetchId；session/completion 的字节反例是 primitive 测试，**不是这些业务请求已经接线**。
- catalog 仅内存：key/handle/fetchId 分开，短效材料不可自动序列化进 route。无 fetchId/空串均兼容；未来 row 用服务端 issuedAt 隐藏，locked 可列出不可选择，refreshAt 不是硬失效。未知扩展字段兼容，不虚构默认 agent 或回退旧静态模型。
- 固定路径，禁 redirect/retry/default secret headers；复用 Gateway proxy factory。一次 send/read 共用最多 10 秒预算，decoded catalog 1 MiB、registration 64 KiB；取消丢弃本操作 future，不留后台 task。临时错误保留上一完整 snapshot，永久/非法响应不报成功。

本地额外准入包括：可见 ASCII token/header，token 最多 16 KiB；catalog 最多 512 row、64 badge/row；毫秒整数不超过 JavaScript safe integer；重复 key/handle、控制字符及非 `fbm1.` handle 拒绝。它们是本实现更窄的有界支持策略，不宣称都是官方 schema 的限制。未解释 agent compaction、effort/placement/section 等附加字段，不把目录解析等同完整 agent 构造。

## 验证与证据

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-catalog-device-20261106`。

- `protocol-red`：三项契约测试实际 0 passed / 3 failed，失败为尚未实现的有效 catalog 与 key 导入；3221 项选定输入无漂移。
- `catalog-focused`：23 passed / 1 failed。失败为测试夹具 hex OddLength，发生在请求发送前；重新用 Python gzip.compress(mtime=0) 生成，并独立解压核对 65,537 个 x 字节后修正，不改生产大小限制。
- `catalog-final`：runtime-defaults 下 24 passed / 0 failed / 0 ignored，3229 项选定输入无漂移；含 12 个使用随机回环 TCP 的测试函数。实际二进制与测试名字在最终 closeout 单列，不与 feature-off 重复计数。
- `production-check`：同配置生产库 cargo check exit 0，3229 项选定输入无漂移。
- `feature-off`：最小 ChatGPT-only 配置编译失败，17 个现有 Gemini compiled-out media import/symbol 错误，未进入测试，输入无漂移；本批不修改 Gemini/S06 lane。仅移除 FreeBuff、保留其他 runtime feature 的 `feature-off-v2` 为 **13 passed / 0 failed / 0 ignored**，含实际 feature-disabled 拒绝 bootstrap/registration；3229 输入无漂移。**不宣称原最小组合或全 feature 矩阵通过。**
- Rustfmt、checker 31/31、ratchet 4588 项通过；14 个 immutable runtime 单列，governed source >700=0。本批 12 项源码最大272有效行，11项新增最大231；原 freebuff.rs 为271→272，仅增加模块接线，不改 baseline/exception。root ratchet report 已直接备份、归档实际输出并精确恢复。
- 最终两种配置共 37 次成功执行、按完整 Rust 测试名去重为 **25 个不同测试**；不是37项不同合同。三个成功阶段各3229输入冻结；最终审计再核对与当前源码一致，最小 feature 失败也使用相同源码。正常 peer graceful shutdown/join 和 caller Drop/body cleanup 已实际执行。
- 三项红测、23/1 fixture 失败、最小 feature 编译失败、rustfmt 排序 probe 和临时 PowerShell verifier 引号失败原记录均保留。来源/UTF-8无BOM/本地链接/diff、二进制哈希及本批进程收尾见证据目录中的 `audit.json`、`test-binaries.json`、`process-closeout.json` 与 `closeout.json`；不报全仓绿。前序扩大 Python scanner 失败未重跑或宣称清除。

## 下一步与未验证边界

1. 将明确业务 origin/client 选择、安装级设备私钥/registration 纳入既有管理鉴权、SecretPatch、revision 与持久秘密 owner；不要新增独立临时 JSON store，也不要默默更改已验登录目标。
2. 从正常管理账号接 catalog 发布/精确选择，再做显式 admission/reuse/消费确认。普通推理不得自动购买会话；未知 POST 不自动重放。
3. 完成 typed agent/lease、session/completion 最终字节签名、正常 Console 精确 probe、公共 JSON/SSE/取消及完整生命周期。Server/跨页面/进程/原生 EXE/整包仍另验。

累计模板仍 16，不增整卡 offline_ready，不复用历史付费、凭据或 Redis 许可。R01–R16 其他未完项和 P50/P60/P70 保留；未提交、推送、部署或出包，未操作在线实例/容器/真实 `.ng`。

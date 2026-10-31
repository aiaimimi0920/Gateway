# Gemini 浏览器认证 checkpoint 与 CAS（r14）

## 本轮完成的闭环

r8 已解决导入 JSON freshness，但浏览器旋转后的认证状态只留在进程内。r14 为浏览器池
`storage_state_file` 增加独立、显式 opt-in 的加密 checkpoint，接入 context 准入和 dispatcher
调用收尾；不覆盖 seed，不增加 npm 依赖，不升级或改写 Gateway 现有数据库。

- AES-256-GCM 加密 Cookie、localStorage、IndexedDB；AAD 绑定 seed identity、内容 hash、
  checkpoint namespace 和 key 标识。不同账号、seed 版本、存储命名空间不混用。
- 本机独立 SQLite 使用事务与 revision CAS。跨进程旧版本不能覆盖新版本，不依赖会在
  崩溃后遗留的应用层 lock 文件。限制记录大小、绑定数量、数据库页数与忙锁等待。
- S3 使用独立 `.checkpoint` 对象，创建 `If-None-Match: *`、更新 `If-Match`，不写原 JSON，
  不覆盖 legacy mirror。请求级 client、单次 SDK attempt、5 秒 deadline、16 MiB 流上限；
  缺 ETag、条件冲突、损坏、远端错误不无条件重写或退回旧状态。
- 空闲准入检查 checkpoint revision，变化才重建 context；同版本仅更新存储配置快照，
  不把磁盘 Cookie 覆盖回 live context。正在执行和保存的 entry 保持 busy。
- 成功操作且仍有认证 Cookie 时捕获状态。401/403 按当前 revision 写失效标记，旧 worker
  不能撤销新 worker 的状态；普通失败不当作认证成功保存。
- 保存失败、冲突、超时不替换已成功的业务回执，不重发生成；回收本次 context，下一次
  准入重读权威状态。捕获超时后的迟到结果不能再保存，finally 仍释放 lease。
- CDP 与 persistent profile 不自动导出。默认 `disabled`，没有为在线真实账号启用。

配置、权限、存储合同和资源边界见 [认证 checkpoint 文档](../gemini-auth-checkpoints.md)。
本功能不是生成任务 checkpoint、业务 exactly-once 或跨 worker 生成互斥；也没有让纯 HTTP
路径自动写回认证状态。S3 CAS 仍依赖服务端真正实现原子条件请求。

## 实际验证

新增准入红测最初 2/3 按预期失败，旧实现没有识别非法 opt-in 配置。最终新增 **37/37**
通过，包含在相邻 Node **122/122** 中，无 skip。所有账号状态都是合成数据，没有真实
Google 会话导出、浏览器启动或上游调用。

覆盖独立子进程读回、两个独立进程争用同一版本只有一个成功、事务内强制退出后的恢复、
重导入和账号隔离、外部更新导致空闲 context 失效、旧 worker 不能覆盖或失效新版本、
容量拒绝、损坏/篡改、加密和敏感正文遮蔽、捕获期间 busy、超时后零迟到写入，以及真实
S3 SDK 对接 loopback 的加密 GET/PUT、条件冲突、缺 ETag、错误/重定向和流大小限制。

首轮 S3 fixture 错将 SDK `x-id` query 当作对象身份，造成 4 项失败；修正 fixture 按
pathname 匹配后通过。失败日志保留，不把该尝试算作成功。超时和流超限另用合成 SDK body
验证 AbortSignal 与销毁。没有用 loopback 合同声称生产 S3 已验收。

- 嵌套 worker 包合同 1/1：四个新模块纳入官方包枚举与内容 hash 校验。
- effective-line checker 31/31、ratchet、16 个 Node 语法检查、Rust formatter、
  `git diff --check`、新增/改动文本 UTF-8 无 BOM 通过。
- `node:sqlite` 和 MockTimers 在 Node 22 下仍有 experimental warning，不声称零告警。
  未运行全量 Rust/Node/桌面回归、真实账号测试或新 EXE 构建。

生产模块有效行：checkpoint coordinator 74、codec 57、本机存储 59、S3 存储 43。
既有 context 313→321、dispatcher 238→252、主 worker 492→494、Python 包合同
253→257；dispatcher fixture 增加一个 no-op 依赖后为 60。新测试与夹具均不超过 149。
没有修改 policy、baseline 或 exception。

证据根：
`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-gemini-dual-mode-20261030-r1/r14/`。
最终以 `validation.json`、`node-adjacent-final.log`、`package-contract-final.log`、
`ratchet-final.log` 与前后有效行记录为准，包含源码 hash，不包含真实 Cookie/token。

## 在线与未完成边界

收尾只读检查：4200 `/healthz`、`/readyz` 均为 200。在线仍是 r10，Gateway PID 46468、
UI PID 36304；没有重启、替换或关闭，没有修改在线账号、配置、凭据和数据库。
在线 r10 不含 r11–r14。没有提交、推送或发布新包。

跨重启认证 checkpoint、本机多进程 CAS 与 S3 条件写回已经完成源码接入和本地合同验证，
尚未完成真实 Google 认证旋转和生产 S3 验收。Playwright 导出会先在内存中组装快照；
持久化的 16 MiB 限制不是浏览器总内存配额，SQLite 同步磁盘操作也不承诺总 deadline。

仍未关闭的业务大项：

1. Web 图片只读恢复、媒体输入/输出、原文朗读、专用转写/MP3 等真实账号公共 API 验收。
2. Canvas 音乐/视频的可信可用宿主合同，不能以普通 Web/API Key 回退代替。
3. 新来源 devtest EXE、隔离运行和人工手测；不能拿 r10 readiness 代替新源码业务成功。

没有重试此前被工具拒绝的真实 storageState 导出/隔离 worker 组合，也没有换入口绕过或
重放已消费的媒体生成 guard。其他任务的恢复等待文件保留不动。

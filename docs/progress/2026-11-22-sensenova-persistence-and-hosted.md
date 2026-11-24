# SenseNova Token Plan：Local 持久化与 Server Hosted 公共推理

## 已验证

- **Local**：从空路由开始，经管理授权写入合成账号和 Key。目录读取只读，不改变 revision、不生成；显式 CAS 保存目录并 `keep` 原 Key。重建 AppState 后，revision、账号标识、Key 和目录保持一致。管理探测命中该账号且不扣用户余额；公共 Chat、Messages 和 Responses 各走原生端点、各结算一条公共审计，三次各扣一条预付消息。包括新用例在内，SenseNova 常规回归 **32 passed、5 ignored**（五项隔离 Server 测试单列执行）。
- **Server Hosted**：通过管理 API 将账号、凭据和唯一模型别名存入 PostgreSQL。随后用不同的、路由文件为空的 AppState 执行 Chat、Messages、Responses 的 JSON／SSE，共六次。逐次核对原生路径、实际 Key、返回终态和精确模型；公共审计绑定调用者、Provider 和凭据，每笔用量 2/3/5 tokens，内部探测审计为零。两套隔离容器均为 loopback 非默认端口；第一遍 PowerShell 5.1 包装的数据库登录失败，沿用原 PowerShell 7 夹具入口通过，没有改密码或测试业务断言。
- **桌面定向回归**：SenseNova 与共享 Command Code 目录用例在内 **20/20**、TypeScript typecheck 通过。旧 SenseNova 测试把非空 grant 换值一律视为权限丢失；现固定相同管理会话续期仍接收一次合法结果，而权限撤销、管理令牌或编辑 revision 变更丢弃迟到结果。此轮未更改桌面生产行为。

## 复跑

在 Gateway 根目录用以下命令。Server 使用与[调度回执](2026-11-22-sensenova-server-probe-audit.md#复跑)相同的独立 PostgreSQL／Redis 和显式环境变量；本文件不依赖临时脚本或真实账号。

```powershell
cargo test --locked --offline --lib --no-default-features --features runtime-defaults --config profile.test.package.gateway.debug=0 -j 2 sensenova -- --test-threads=1
cargo test --locked --offline --lib --no-default-features --features runtime-defaults --config profile.test.package.gateway.debug=0 -j 2 sensenova_server_ -- --ignored --test-threads=1
```

## 桌面手测包与原生运行

- `desktop / devtest`（非正式 Release）：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/release/Gateway/gateway-devtest-20261122-sensenova-persistence-v1`。清单和双 EXE 的大小及 SHA-256 匹配。构建来源指纹：`01af322fb9f4851452e48ee4b70552a497005c205b063d7e1631c6dce8808112`，记录的是构建时的工作树。
- `gateway.exe` SHA-256：`40b8fb674244d26e8630f9737b25355da70d64d2be8bed80eb9ee9eeb46c0299`；`gateway-ui.exe` SHA-256：`3407f683a17ff2ab4a04274f5b19af4338a0a28ee19e703355a7fa851ce22df3`。包内通用 Local SQLite 启动、重启、余额持久化、Key 轮换和正常 drain 通过。
- 独立端口及新数据目录中，原生桌面点击选择 `sensenova-token-plan-api`，确认锁定 Base URL，添加并实际写入空池，再打开账号表单核对字段。桌面保存新 Key 会触发官方目录请求；为保持零上游调用，此处未点击账号保存，改用同一隔离包的本地管理 API 以 CAS 写入合成账号。正常退出并重开原生桌面与其托管后端后，桌面显示保存的账号，API 读回同一 revision、账号标识与 `configured=true`；两轮后端 PID 不同。结束后两个新进程均退出，4200 上既有实例不受影响。
- 同一源码测试二进制的隔离 Server 回归 **5/5**，含实际调度两个周期及 Hosted 三端点 JSON／SSE；测试容器已正常停止，保留卷。有效行 checker **31/31**、ratchet、定向格式及 `git diff --check` 通过。

## 未验证边界

2026-11-22 后续：使用包含 Chat 推理历史改动的新测试 EXE，在隔离 PostgreSQL／Redis 复跑上述 Server 套件 **5/5（63.52 秒）**，覆盖 Hosted 原生三端点 JSON／SSE、两个调度周期和探测失败／取消／持久化边界。容器已正常停止，未删除数据。工具推理历史两轮是同一 EXE 的另一个 Local 用例，不能算 Hosted 多轮。

原生桌面验证不等于两个 Server EXE 的跨进程推理或真实 SenseNova 目录、生成和结算。现有源码合成上游使用仅在 `cfg(test)` 编译的传输夹具，不能直接移到已发布的手测 EXE。现有默认客户端锁定 `wreq` 的 `webpki-roots`，`SSL_CERT_FILE` 不会替换该默认根；仅配置本机拒绝转发的代理也只能验证拒绝边界，不能制造成功回复。未更改固定域名、主机证书信任或 TLS 校验。Hosted 也不证明 Redis 凭据缓存失效；Redis 是本次 Server 运行依赖，不是上述 Hosted 凭据的真源。控制台 OAuth／积分池、实际订阅资格与用量仍独立列项。没有导入真实账号、调用付费服务或部署。

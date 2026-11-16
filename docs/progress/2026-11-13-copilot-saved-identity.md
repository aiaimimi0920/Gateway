# Copilot 已保存身份与目录代次校验

后续同日更新：显式 SDK 构建已增加受限公共路由资格及公共链验证，见
[公共单轮文本接线](2026-11-13-copilot-public-routing.md)。下文“无条件禁用”描述的是本次
历史保存切片，不再代表后续 feature-on 源码；普通构建仍保持禁用。

本块复用现有 route-config 凭据存储、SecretPatch、管理鉴权和 CAS；没有新增数据库、
存储版本、登录流程或公共推理 adapter。生产验证结果在下方记录，不等于整张 Copilot 卡完成。

## 保存合同

专用容器使用 `auth_mode: copilot_sdk`、`explicit_credentials_only: true` 和空 `base_url`。
不配置 preset 或 adapter，不填写任何 HTTP endpoint、通用认证、session、refresh 或发现配置。
凭据新增可选秘密字段 `copilot_github_token`，缺省不存在；旧配置读取不需要迁移。

```json
{
  "id": "copilot-local",
  "auth_mode": "copilot_sdk",
  "base_url": "",
  "explicit_credentials_only": true,
  "credentials": [{"id": "copilot-account", "enabled": false}]
}
```

使用现有 `PUT /v1/internal/gateway/console/route-config` 保存整份脱敏 document，并用
SecretPatch 替换 `/providers/<index>/credentials/<index>/copilot_github_token`。
仍需有效管理身份、Secret Grant 和 expectedRevision；不允许把秘密原文塞入 document。
替换、保留、清除和稳定凭据 ID 的规则复用现有实现；响应采用现有 descriptor／fingerprint／
preview 策略，不返回完整 token，也没有新增原文复制 API。保存不验证账号订阅或推理权限。

新字段不进入 ProviderAccountPayload，不能被通用 HTTP 认证、探测或 refresh 发送。
公共推理尚未接通，因此编译器无条件把专用容器内所有凭据设为不可路由，即使 document
写了 `enabled: true` 也不会开放。空 endpoint 和禁止其他认证材料使手动通用探测没有 HTTP
目标；未知 adapter 本身存在通用 HTTP fallback，故本实现不依赖未知 adapter 保护秘密。
这些存储与资格约束不受可选 SDK feature 是否开启影响。

## 从已保存账号读取目录

在启用 `copilot-sdk-runtime` 的构建中，同一目录接口还支持：

```json
{
  "providerId": "copilot-local",
  "credentialId": "copilot-account",
  "expectedRevision": "current-route-revision"
}
```

该身份来源与原有 `githubToken` 临时来源互斥。只从同一 snapshot 的专用字段取 token；
配置 revision 作为保守的账号代次栅栏，任何配置变化都会要求客户端重新读取 revision。
runtime 启动前、启动后发送目录 RPC 前及结果发布前校验 revision，旧身份结果不能发布。
这不是跨网络请求的事务锁，也不宣称撤销已经发到上游的 RPC。目录仍不触发生成，
沿用上一块的并发、断连、shutdown 和目录清理 owner。

## 验证与证据

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-copilot-credentials-20261113`。
定向测试覆盖秘密保存／保留／重排／替换／清除、旧配置读取、禁用资格与无通用 payload、
Secret Grant／CAS，以及合成账号 A → B 轮换后的旧 revision 拒绝和新目录读取。
最终执行结果：

- 根工程锁定离线 feature 定向编译成功，Copilot **9 passed / 0 failed / 0 ignored**；
  opt-in 合成 EXE 测试已显式执行，没有计作跳过成功。
- 同一刚编译测试 EXE 的账号组相关回归 **17 passed**，包含因新 schema 字段而调整的夹具。
- 本轮 7 个合成 child 通过 Windows OpenProcess／WaitForSingleObject 核对全部退出；
  `focused-processes.json` 固定该次核对，不用后来可能复用的 PID 重判已退出进程。
- checker 测试 **31 passed**；ratchet 通过，5496 文件、governed source >700=0。
- formatter、scoped diff、UTF-8 无 BOM 和 Cargo.lock 与本轮开始快照一致检查通过。
- 根编译记录 90 条警告；随后仅移除新增测试的一条未使用 import，未为该无行为清理重复
  11 分钟的根测试编译。其余警告未扩大清理。

新增配置 owner 72 有效行，配置／秘密测试 117 行，保存与目录集成测试 129 行；
已有 catalog owner 113 → 127、入口 97 → 121。账号组夹具改用既有 serde 默认值构造，
避免每次 schema 增字段重复手填几十个无关空值，文件 500 → 463 行；17 项回归通过。
没有 baseline 变更、新依赖或存储版本升级。

只读独立审查核对了活跃配置、历史 revision、旧凭据列表、复制密钥、refresh 和归档路径；
专用 token 只随原有私密配置／归档持久化，不进入通用 payload。没有将归档误称为内存态。
仍未进行真实 CLI、真实账号、EXE 出包或 Server 部署验收；没有提交、推送或运行实例变更。

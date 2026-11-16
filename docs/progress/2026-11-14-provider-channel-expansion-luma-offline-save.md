# Luma 显式离线授权与秘密保存

日期：2026-11-14。范围仅为 MCP 管理授权与保存，不开放自动刷新、桌面离线开关或生成业务。

## 本轮改动

- 管理请求显式 `offlineAccess`，默认 false；授权、会话绑定、material 和 context 全程保留该选择。
- 离线模式请求 `openid offline_access` 与 `prompt=consent`，必须收到合法 refresh；普通登录仍丢弃额外 refresh。
- 现有 Gateway 动态注册只有 authorization_code；持久回执中相同 client ID 拒绝离线请求，不能通过换 owner 绕过。存储不可读时不继续授权。
- 复用原 SecretPatch 保存 access／refresh；普通重新授权清除旧 refresh，不修改账号 ID 或其他元数据；清空秘密后允许保留可读 context。
- 默认 false 字段不改变旧 context 的 canonical 表达；Keep 绑定覆盖新字段。
- RefreshGrant 要求 offline context，保留轮换结果的 offline 元数据；尚无正常账号刷新调用点。
- disabled、禁止可变刷新地址／client／lifetime、禁止自动 probe 和推理等约束保持不变。

## 验证记录

- 测试先失败：新增正常管理入口请求得到 422，测试期望 200；证明旧入口拒绝 offlineAccess。
- 源码只读审查发现测试缺少另一秘密字段的操作；已补 Clear，保留分别验证 access／refresh Keep 绑定的意图，未放宽生产代码。
- 定向新增 6 项测试全部通过。相关 Luma 回归 157 passed、0 failed、2 ignored（未执行的显式 opt-in 场景不计通过）。
- 回归复用本次刚构建的 `target/debug/deps/neuro_gateway-391168db0cd875e0.exe`，没有复用旧包；先取消了本轮仅等待锁的重复 Cargo 启动器，没有终止其他任务。执行命令为该测试程序加 `luma_ --test-threads=2`，详见 `regression-direct.log`。
- 非测试 `cargo check --locked --offline --lib --no-default-features --features runtime-defaults -j 4` 通过。编译仍有仓库既有 warning，本轮未扩大为无关清理。
- 有效行检查器测试 31 项通过；ratchet、scoped rustfmt、scoped git diff --check、UTF-8 无 BOM 校验通过。18 项源文件哈希在回归后保持一致，详见 `source-hashes.json`；各文件均不超过 250 有效行。
- 日志目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-luma-offline-save-20261114`。

## 未完成边界

后续需要持久旧 grant 消费占用、一次性刷新结果保存／恢复／发布，以及显式桌面入口和 refresh-capable client 注册方案。未进行真实授权、刷新、工具调用、下载或费用消费；未修改运行实例、真实凭据或数据库；未出 EXE、未提交或推送。

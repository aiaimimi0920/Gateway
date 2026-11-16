# Luma 刷新响应回执与解释前保存

日期：2026-11-14。完成范围是协议接收能力和 Sending 回执持久保存；正常账号执行器和 Result／Published 尚未开放。

## 实现

- `RefreshGrant::receive(self)` 单次读取后返回有界原始响应，不在保存前执行 token admission。保留原固定目的地、15 秒总预算、64 KiB decoded limit、无默认凭据、不跳转、不重试的约束。
- `ReceivedRefresh` 保存 canonical base64 正文、发送时间与完整 context／旧 refresh 绑定。保留重复字段、畸形 expiry、非 JSON／非 UTF-8 等响应以供隔离；读取时验证边界，解释时才进行原有 token 校验。
- expiry 始终锚定原发送时间；旧 refresh 省略保留、显式非法值拒绝的规则不变。原 refresh 不额外明文复制到回执；Debug 不输出正文、binding 或秘密。
- 新增 Luma receipt variant，复用原文件所有者、writer lock、精确原记录和不可变回执写入。authority 改变后仍可保存收到的原响应，不代表可以发布到新 authority。
- Result／Published 仍明确拒绝，包括已附合法 receipt 的合法 successor。通用 worker 的穷尽匹配分支返回固定 terminal 错误，不增加 Luma worker 构造入口或自动刷新。
- base64 不是加密，绑定 hash 不是 MAC，也不是对拥有存储写权限者的防篡改证明；存储权限及完整性沿用原 owner 合同。

## 验证

- 测试先失败：原 GrantReceipt 拒绝 `lumaOauth` variant；随后实现新 variant 与完整保存／解释合同。
- 只读审查完成。测试 endpoint 注入仅存在于 cfg(test)，公开 receive 的网络测试使用本地合成服务，不访问官方端点。
- 新增定向测试 9 项通过。本轮刚构建的测试程序执行 Luma 回归：171 passed、0 failed、2 ignored（含新增 9 项，跳过项不计为通过）。
- 原有 `console_refresh_grant_contract` 集成测试：14 passed、0 failed、6 ignored；6 项 Redis 场景需要显式可丢弃 Redis 地址，本轮未执行。该集成测试已构建非测试库。
- 非测试库编译已由上述集成测试构建验证通过。为避免重复编译，主动取消额外的 `cargo check`；该命令未取得完整成功结果，不计通过。只停止本轮检查进程，并复核没有其遗留子进程，未停止其他任务的构建。
- 格式／scoped diff、有效行检查器 31 项、ratchet 通过；11 项源文件已冻结并检查 UTF-8 无 BOM，最大文件 374 有效行，无超 700 行文件增长。
- 证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-luma-received-20261114`；测试 TEMP/TMP 指向该目录下 temp。源文件哈希见 `source-hashes.json`。

## 未验证和后续

尚需接通正常账号执行器的 reserve→receive→receipt 持久确认→解释，以及 Result／revision 轮换发布和恢复。当前协议 mock 与本地持久重建测试不等于真实账号、进程崩溃、多节点、Redis／PG 或原生包验收。

本轮不登录、不真实刷新、不生成或下载、不消费额度；不修改运行实例和真实凭据，不出包、不提交或推送。

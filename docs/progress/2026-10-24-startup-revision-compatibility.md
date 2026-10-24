# 2026-10-24 桌面后端启动兼容性修复

## 故障与责任层

会话 `01a169c8-64fd-7c70-9e75-0a819558e149` 指出的
`gateway-devtest-20261024-004824` 并非缺少桌面自动连接操作：后端在恢复旧版
站点健康计划的不可变配置 revision 时退出。新增的 `enabled: bool` 缺省为 true，
但序列化时会把原文中没有的字段写回，导致 documentDigest/yamlDigest 与原档案不符。
本地与 Redis 配置共用 revision 校验类型，因此本地错误也带有 Redis 字样。

## 最小修复

- `ProbePolicy.enabled` 保留缺省/显式 true/显式 false 三态，缺省不输出字段。
- 运行时统一调用 `is_enabled()`，缺省仍开启；暂停、取消、过滤和遥测仍使用布尔语义。
- 显式 null、字符串依然非法；没有放松摘要校验、重写历史档案或变更存储版本。
- 前端已有 `z.boolean().default(true)`，无需修改桌面启动或连接逻辑。

本次范围为 site_health 的 policy、五处运行使用位置及相关回归测试，不接管工作区
其他未提交功能。没有访问真实凭证内容、停止旧实例、修改数据库、提交或推送。

## 已完成验证

- 修复前：兼容性测试准确复现两个摘要拒绝；篡改凭证的反例仍被拒绝。
- 修复后：4 个定向 Rust 套件共 15 项通过，覆盖三态精确往返、连续两次恢复、
  档案/路由/测试秘密不改写，以及计划暂停取消、默认开启与限制行为。
- 前端站点健康解析与计划测试：2 个文件、4 项通过。
- 原问题包使用合成历史档案启动：复现原错误并退出。
- 修复后的本地 devtest EXE 使用相同合成档案：连续两次 SQLite ready，
  `/ui/` HTTP 200，正常 drain 退出；档案与 routes 哈希不变。
- effective-line checker 31 项测试通过，ratchet 通过；本次文件 rustfmt 检查通过。

临时证据目录：
`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-startup-compat-20261024/`
其中 `red.log`、`green.log`、`old-package/result.json`、`fixed-binary/result.json`
分别记录失败复现、通过结果和真实进程启动结果。测试只使用虚构凭证与隔离目录。

## 交付与验收边界

使用日常 `tools/package-gateway-devtest.ps1` 生成新的 desktop 手测包，不覆盖旧目录。
最终包的构建/清单与启动证据另存同一临时证据目录，不能把本地 EXE 的检查冒充包检查。
本记录不宣称正式发布、全量回归或付费上游验证完成。

旧包 `gateway-devtest-20261023-231034` 仍占用本机 4200，保留运行现场。
桌面本地启动拒绝抢占其他实例端口；用户手测新包前应正常关闭旧桌面实例。
本轮没有在真实数据上打开新桌面 WebView，也没有自动切换现有运行版本。
# Command Code 原生桌面人工验收入口

用户已同意在不提供真实账号的隔离环境中完成原生点击与表单验收。

入口：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-first-three-20261113/commandcode-manual/Start-CommandCode.cmd`

同目录 `手测步骤.md` 提供假 Key 保存、隐藏、编辑及关闭重开保持的简短步骤。管理密钥为仅用于本机合成环境的 `commandcode-manual-local-only`。

复用 `gateway-devtest-commandcode-20261114-closure1`，未重建或改写包。启动器校验包完整性，独立 Local SQLite 后端使用随机 loopback 端口；桌面通过 existing 模式连接该后端，另用独立 profile。所有数据、日志保留于入口目录。不使用用户原有 `.ng`、真实凭据或 PostgreSQL/Redis。脚本关闭时仅 drain 自己创建的后端。

已实际运行 `start.ps1 -VerifyOnly`，退出码 0：readiness HTTP 200、SQLite 非降级且不要求 Redis、控制台 HTML HTTP 200、drain HTTP 202、后端正常退出。复核仅有既有 gateway PID 34624，未修改或停止它。自动回执保存在入口目录 `runs/*/result.json`。

原生点击/表单验收仍待用户反馈，预检不替代人工通过。此环境是数据隔离，不是断网沙箱；仅使用假 Key，保持自动测试关闭，不执行上游导入、测试或生成。

## 同批 Server 回执补充

`commandcode-packaged-server-v6/result.json` 已读取确认 passed：两个真实 EXE 共享专用 DB/Redis，第一进程禁用账号后第二进程拒绝路由，重新启用后恢复现金边界拒绝，现金不变，两个进程正常 drain。此前 v5 的 PUT 更新得到 405，核实管理路由后改为 POST，v6 退出码 0。

该证据属于 DB/access-catalog 跨进程变更生效，不冒称 Redis CredentialEntry 缓存失效专项验收。自动 scheduler 测试仍在共享 Cargo target 队列中，不记录为通过。

# SenseNova Token Plan Server 自动探测与内部审计

## 缺口和修复

- 在隔离 PostgreSQL／Redis 中先用修复前的二进制复现：关闭该 AppState 的专用池后，探测仍发送 **1** 次；两个实际周期取得 **2** 次合成上游回复且 Redis 已发布结果，但 PostgreSQL 内部审计为 **0** 条。
- SenseNova 的显式与定时文本探测现共用 Command Code 已有的取消安全生命周期。在 Server 下先提交 `running` 才发固定 Chat 端点；不能创建审计时不发送。终态写入失败不回传成功或答案、不改变模型健康，也不重放请求；保留的 `running` 仅表示终态未知。Local SQLite 行为不变。
- 沿用已有 `007` 表，使用代码绑定的产品 `sensenova_token_plan`；创建和更新都按产品及部署作用域约束。只保存精确 Provider／账号标识、模型、状态、耗时与观察到的用量，不保存 Key、正文或原始上游错误。不使用公共调用者身份，不进入公共请求审计或用户计费。

## 验证回执

- SenseNova Server **4/4** 通过：两个独立 AppState 运行实际 60 秒调度循环，合计只发两次，各形成一条 `completed` 审计且用量为 2/3/5 tokens；第二实例及重建后的 AppState 读到同一结果，重建后活跃 Redis 槽仍阻止重复领取。上游拒绝／请求取消各一条相应终态；关闭连接池后发送前拦截；收到合成回复后终态写入失败，不误报、不重发。实库独立读回为 `completed` 2、`failed` 1、`cancelled` 1、负对照 `running` 1、公共审计 0。
- Command Code Server 回归 **7/7** 通过，含同库两周期、内部审计边界、需公共调用身份、Hosted 原生 Messages 对照及仅含 `001` 至 `006` 的独立负对照库。
- SenseNova 常规回归 **31 passed**、Command Code 常规回归 **104 passed**，共享探测审计 **7 passed**、调度作用域 **1 passed**；SenseNova 独立 feature 的 lib／bins 检查、定向 Rust 格式、有效行 checker **31/31**、ratchet 和 `git diff --check` 通过。

本批只启动既有隔离夹具，没有变更生产库、账号、额度或部署。两套测试库保留数据。双 AppState 不等同双 EXE；未验证 SenseNova Hosted 公共推理、Local 管理保存后的真实重启使用、桌面手测包及原生业务点击，也未向真实 SenseNova 发送请求。

## 复跑

使用隔离的 loopback PostgreSQL 数据库 `gateway_fixture`（已应用 `001` 至 `007`）与独立 loopback Redis，不能使用默认 `5432`／`6379` 端口。设置 `SENSENOVA_TEST_DATABASE_URL`、`SENSENOVA_TEST_REDIS_URL` 和 `SENSENOVA_TEST_EVIDENCE_ROOT`（`linshi` 下已创建的证据目录），在 Gateway 根目录运行：

```powershell
cargo test --locked --offline --lib --no-default-features --features runtime-defaults --config profile.test.package.gateway.debug=0 -j 2 sensenova_server_ -- --ignored --test-threads=1 --nocapture
cargo check --locked --offline --lib --bins --no-default-features --features line-sensenova-token-plan -j 2
```

本轮短期日志位于 `C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-sensenova-server-audit-20261122`。测试与复跑入口均在仓库内，不依赖该目录的临时脚本。

# Command Code ZDR 请求与账号设置

本切片属于首批三个服务商中的 Command Code，不代表整卡完成；SenseNova Token Plan、
SAP AI Core 仍按顺序排队。Gemini 仅保留用户授权的 feature 关闭编译门禁修复，不扩展功能。

## 合同与实现

2026-11-14 联网核实官方 `https://commandcode.ai/docs/provider`：`x-cmd-zdr: 1` 强制
零数据保留／不训练；无合格上游时返回 422 `cmd_zdr_no_providers`，不能降级普通上游。
Responses 在 ZDR 下仅允许客户端执行的 function/custom/local_shell 工具；Messages 接受
该 header，但其 Anthropic 路径本已有账号级 ZDR。真实路由、资格与价格仍需真实账号确认。

- 复用现有 provider/credential headers 持久化，不新增数据库字段、不迁移数据。
  只接受逻辑 `x-cmd-zdr=1`，拒绝其他 header 或任何冲突值；配置 merge 出现同值大小写变体
  可以共存，sender 最终只发一个规范 header。公共 HTTP 重复值仍严格拒绝。
- 账号表单增加“零数据保留（ZDR）”选择，默认继承，可强制开启；继承不能关闭 provider
  已要求的 ZDR。保存、重开、复制和 Key keep/待保存 secret patch 保持原契约。
- 三个公共协议入口严格接收 caller header；空值、非 1、逗号合并值、重复值不能静默变成关闭。
- 真正进入候选前先完成 provider policy/fallback 授权筛选，再提升请求级隐私要求；
  信号保存在跨 retry/re-route 的 context，而不是单 attempt 或可伪造的 raw body extra。
- eligibility/capacity 跳过后，后续 Command Code 候选仍强制 ZDR；若遇到其他产品，明确失败，
  不假定其他产品的同名 header 具有等价保证。这里选择 fail-closed，不扩建跨产品隐私路由器。
- refresh 后重新应用要求；最终原生 Responses wire tools 再检查白名单，Messages 不误套此限制。
- 422 只识别固定公开 code，不反射上游正文／秘密；保留 Abort、不重放和退款流程。

## 验证

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-first-three-20261113`。

- `commandcode-zdr-ui-red.log`：新增两个行为测试均按预期失败（缺控件／未保存 header）。
- `commandcode-zdr-rust-red.log`：账号 ZDR 配置红测 **0 passed / 1 failed**，exit 101；
  失败点是现有配置拒绝合法 ZDR，不是编译失败。
- `commandcode-zdr-ui-final.log`：三个相关测试文件 **11 passed**。
- `commandcode-zdr-ui-typecheck-final.log`：typecheck exit 0。
- `commandcode-zdr-checker.log`：31 项通过；最终 `commandcode-zdr-ratchet-closure.log`
  exit 0，未改 baseline；定向 rustfmt 和 scoped `git diff --check` 通过。
- `commandcode-zdr-rust-green.log`：首轮 **84 passed / 1 failed**。失败属于测试夹具：
  直接取 probe target 后没有像生产路由一样选择原生 Responses/Messages path，误走 Chat 路径。
  已按已有原生请求测试的方式补齐协议选择，未改生产语义或放宽工具断言。
- 真实 send-loop early-skip／policy 测试已通过：关闭测试 Redis pool，在 route 后禁用
  credential，验证零发送尝试及 ZDR latch；未授权候选不能污染请求状态。
- `commandcode-zdr-rust-final.log`：**86 passed / 1 failed**，唯一失败是既有真实 60 秒
  调度测试的重启断言，见下节；新增 ZDR 与 send-loop 测试均通过。
- 同一测试 EXE、不改生产源码，`commandcode-zdr-serial.log`：**87 passed / 0 failed**，
  90.25 秒，exit 0。不隐瞒此前并发回归失败，也不将一次重跑当作根因修复。
- `commandcode-zdr-feature.log`：`--no-default-features --features line-commandcode-provider-api`
  的 `cargo check --locked --offline --lib --bins -j 2` 已通过，exit 0。
- `commandcode-zdr-production-hashes.json` 记录 7 个本次生产 Rust 文件指纹；调度测试修正后
  复核一致，生产逻辑未随测试调整改变。
- `commandcode-zdr-closure-source-hashes.json` 记录 25 个本次源码／测试文件；UTF-8 无 BOM
  校验通过。

## 调度测试时间边界修正

只读查询失败用例的隔离 SQLite，保存 `commandcode-zdr-scheduler-timing.json`：
首轮四次请求跨越 `23:53:08`／`23:53:09`，后续只有两次请求在 `23:54:08` 执行，下一次
timer tick 到 `23:55:08` 才执行更多请求。累计 8 次并不保证四个计划均刚完成第二轮，
重启时仍可能有已正常到期的计划，所以 `due_count == 0` 不能单独证明是否重置槽。
这里是测试断言的时间假设有误，不能将该失败归因为 ZDR 引入了调度重放。

保留真实 worker、60 秒定时器、首次未到期 sweep、请求计数／用量和结果持久化检查；
仅将重启部分改为比较重启前后完整的 4 个 `(id, next)` 持久化槽，验证时间戳未重置。
没有暂停时钟、写入伪造 next、放宽生产间隔或改变生产调度逻辑。
`commandcode-zdr-scheduler-fixed.log`：修正后的定向回归 **1 passed / 0 failed**，exit 0，
实际运行 60.33 秒；此次只改变测试夹具断言，未无故重跑此前已通过的其他 86 项。

新增 ZDR 策略模块 109 effective lines，HTTP 捕获 15；stage_send 499→495，
原 Bedrock preflight 收入既有 admission owner，保持原校验语义，避免触及 501 行例外。

未进行真实账号请求、PostgreSQL/Redis 部署验收、原生桌面视觉手测或当前源码整包交付。
远端状态／background、System One/Jev、Studio 资格、Server／多实例等仍按总清单保持开放。

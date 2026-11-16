# Command Code 原生目录、专用探测与本地重启

## 本轮范围

目录读取现在保留声明 Chat、Messages 或 Responses 的模型，不再遗漏原生专用模型。
System One-only 尚未接入对话执行池，仍是未完成的独立功能，不以过滤它来宣称整项完成。

专用模型探测在发送前按 Chat → Responses → Messages 的声明顺序确定一个端点，
使用对应的请求与回复解析器；失败不切换端点重放。继续使用固定目标、精确账号 Key、
不跟随跳转的产品发送器。回复限制、唯一 JSON、完成状态、无工具输出、脱敏和未知用量语义保持。
Responses 的 input/output 求和溢出和 total 不一致在生产与 probe 共用解析处拒绝。

对目录明确声明的模型严格遵守声明，不给 Messages-only／Responses-only 增加 Chat 回退。
目录外的手工模型则保留历史 Chat 路径，不因为刷新目录而失效，也不会凭模型名获得原生协议资格。

## 验证记录

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-first-three-20261113`。

- 红测 `catalog-native-probe-red.log`：43 passed / 2 failed，复现原生专用目录被遗漏、probe 失败。
- 首轮实现 `catalog-native-probe-r1.log`：48 passed / 0 failed；包含两种原生 probe 成功、
  脱敏／未知用量，以及坏 JSON、超限正文、工具输出、未完成状态、非法用量不重放。
- 本地持久化测试经真实管理目录接口、SecretPatch keep 与 revision CAS 保存，销毁 runtime 后
  从同一 state/config 重建，再验证精确 Key、revision、原生 probe、公共推理、审计和普通 Key 扣费。
  这是进程内 runtime 重建，不是打包 EXE 进程启动验收。
- 前端 native-only 目录 apply／序列化测试 5 passed，typecheck 通过；未改变前端产品 adapter 身份。
- 复用首轮测试 EXE 的共享目录回归 35 passed、Responses 回归 60 passed；相关共享源码未再改变。
- 追加手工模型兼容保护后的最终 Rust 回归 `catalog-native-probe-r2.log`：49 passed / 0 failed，
  测试 74.67 秒；包含目录外手工 Chat 与两类原生专用声明互不扩权的保护测试。
- 最终独立 feature 编译通过（exit 0），见 `catalog-native-commandcode-feature.log`：
  `cargo check --locked --offline --lib --bins --no-default-features --features line-commandcode-provider-api -j 2`。
  总耗时 12m24s 含等待其他工作线的共享 Cargo target 锁，不作为纯构建性能数据。
- checker／ratchet 追加修改后通过；未改有效行 baseline 或例外。
- 16 个本轮源码／测试文件 UTF-8 无 BOM 检查与 `git diff --check` 通过。
  源码和最终测试 EXE 的 SHA-256 分别保存在 `catalog-native-source-sha256.txt`、
  `catalog-native-test-exe-sha256.txt`，测试 EXE 不作为产品交付包。

## 剩余边界

Command Code 仍处于开发中。Server DB raw payload 尚缺目录到 compiler metadata 的投影及
与显式 allowed/blocked family 策略的组合验证；不能将 Local 证据外推到 Server。
完整保真、System One/Jev、reasoning／媒体、ZDR、Studio 身份边界和可运行包相关门槛仍见
[首批三个收口清单](../providers/first-three-closure-2026-11-13.md)。本轮未调用真实付费上游，
未修改真实凭据、数据库或运行实例，未提交或推送。

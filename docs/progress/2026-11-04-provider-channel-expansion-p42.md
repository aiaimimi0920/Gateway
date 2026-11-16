# P42：Accio 最终业务 body 的凭据所有权

日期：2026-11-04（Asia/Shanghai）。状态：本批业务 planner 修补与定向离线验证通过；R01/W01 和总目标仍在执行，不报告完整 OAuth 或全仓验收完成。

## 1. 范围与兼容合同

接续 P41 的实际业务风险：刷新 runtime override 改变 `payload.api_key`，但旧 Accio planner 不把该值固定到 body；caller `req.extra` 与随后合并的 `payload.extra_body` 可以留下旧 token。专用 planner 的 `utdid` 也使用 `or_insert`，不能覆盖 caller 已填值。

`routes.example.yaml` 明确支持 `extra_body.token` + credential header `utdid` 的 legacy 配置，必须保留；已有 importer 同时保存 `token/accessToken` 是冗余材料事实，不证明业务 wire 必须双发。`session_chat.rs` 与架构合同指定业务认证字段为 `token`，设备来自 headers；没有依据新增 extra-body-only 设备来源。

本批已实现的最窄责任为：

- 当前非空 `payload.api_key` 为唯一优先认证来源，忽略旧副本；空白 primary 不冒充可用 token。
- 缺省时仅兼容可信配置中的 `extra_body.token/accessToken`，两个值不一致或已提供的值类型/内容无效时明确拒绝，不任意选择身份。
- 最终 wire 规范为 `token`；caller 和配置中的认证别名不能绕过专用绑定或被 generic merge 重新加入。
- body `utdid` 来自可信 header `utdid/x-utdid`，必须有效且与现有 header owner 相同；caller 和旧 body 副本不能替换或补足它。不生成随机设备。
- 不把 refresh token 或 opaque `sgcookie` 作为业务 body 参数发送；不把 `sgcookie` 推定成 Cookie。

不改变其他平台的 merge 语义，不修改持久文档/存储版本，不开放正常 Accio 登录或未接完的专用 refresh，不扩大自定义 endpoint 的已有行为。

## 2. 验证与证据

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-provider-expansion-20261104-p42`。

四个既有源码和三份既有文档已有逐字节 before。`source-before.json` 记录四源码有效行为 request_plan 149、module 4、disabled 20、generic planner 157；不使用全 dirty 工作树 diff 冒充本批增量。

生产未改时，`red-body-auth.log` 实际编译并执行 12 项新测试：**1 passed / 11 failed，exit 101**。失败实际复现 caller token/utdid 越过可信材料、旧别名残留及缺失/冲突材料仍能形成计划，不是编译报错冒充行为红测。

实现后另补一个真实 `RouteConfigStore` / refresh generation / candidate override → 公共 planner 的测试：直接 apply 合成轮换材料，旧持久文档与 extra body 均不改写，候选的当前 access 应成为最终 token；没有调用 refresh transport、durable grant 或真实上游，不把它说成完整 scheduler 验收。

首轮实现后的 `green-accio.log` 为 **80 passed / 1 failed，exit 101**。唯一失败在新增 routing fixture 的反序列化：`missing field base_url`，尚未走到行为断言。v2 只补齐夹具必填地址，不改生产逻辑、不削弱断言；同一四项生产源码 hash 与 v1 完全相同。既有 custom-path 正向夹具也补了可信 header utdid，以满足现有材料合同；其 URL 断言未改变，缺失设备另有负向用例。

最终成功检查如下，完整命令与退出结果由 `verify.ps1`、`regression-v2-exits.json`、`verify-disabled.ps1` 记录：

| 日志 | 实际结果 |
| --- | --- |
| `green-accio-v2.log` | 81 passed，其中 13 个本批新增测试 |
| `regression-planners.log` | 33 passed |
| `regression-call-flow.log` | 3 passed |
| `regression-routing.log` | 119 passed |
| `regression-headers.log` | 39 passed |
| `feature-disabled-check.log` | 仅关闭 Accio leaf、保留其余 runtime-defaults leaf，`cargo check --lib` exit 0 |
| `checker.log` | 31/31 passed；checker 未改，不因单个 fixture 字符串修补重复执行 |
| `rustfmt-final.log` / `ratchet-final.log` | 官方 formatter 检查与无报告写入 ratchet 均 exit 0 |

五个成功 Rust run 共 275 次通过，`test-summary.json` 按完整测试名去重为 **270 个不同 Rust 测试，13 个本批新增**；不累计红测或首轮失败 run 的通过项。新增矩阵是生产 planner 的 canonical-family 标签及 buffered/stream 参数验证，不冒充各 HTTP ingress 的真实解码或实际发送验收。runtime override 用例实际经过现有 generation 与候选投影，但不证明尚未接通的 Accio refresh transport/持久 grant。

五项源码最终与测试前 `source-frozen-v2.json` 完全一致；最终九项范围（五源码、四文档）的 hash、UTF-8 无 BOM、尾空白和逐文件 scoped diff 检查记录于 `source-final.json` 与 `scoped-diff.patch`。有效行：request_plan 149→208、module 4→8、disabled 20→23、generic planner 157→162；新增单一职责 planner 测试为 276，处于 251–500 的允许范围，没有 501+ 新增或增长的历史超限文件。不改 baseline/exception，不以全 dirty 工作树 diff 冒充本批增量。

只读独立复审在指定修改范围内未发现可复现的正确性或认证边界缺陷；disabled planner 仍在 generic merge 前返回 compiled-out，新增 predicate stub 不产生发送计划。本批 feature-disabled 只做依赖/类型检查，没有重跑 P41 的整个关闭模式测试；P41 明列的四项既有用例失败未修复，也不报告全 feature 矩阵绿。

所有材料为合成值；未读取 `.ng`/历史凭据，未发真实平台认证或生成请求，未新建 Redis/Docker 对象，未启用 management-ui/工作区发布 hook，未出包、替换运行实例、提交或推送。

## 3. 后续责任

共享刷新仍须让原绝对 expiry 和 sgcookie 同时贯穿秘密存储、generation、durable Result、有限配置投影、replay、publication 和 cache repair，之后才能替换 P41 的临时 gate；不能另起 scheduler 或仅接 JSON sender。

正常管理登录/Secret Grant/原 revision/稳定 credential ID/utdid/SecretPatch-CAS、模板/UI、精确业务/额度测试和真实客户端使用边界仍待完成。前端新增模板仍为 11 个，本批不增加完整 offline_ready 卡；其他原定渠道与 P50/P60/P70 均继续。

# Command Code 原生请求保真与策略同步

本切片继续首批三个服务商中的 Command Code；SenseNova Token Plan 与 SAP AI Core 不扩写。
这不是服务商整卡完成回执。

## 已复现问题

原生 Messages／Responses 的目录选择与回复透传已有实现，但 runtime 在原生分派前执行
共享 text/function validator，使 reasoning、媒体、custom／内置工具等原生请求被拒绝。
直接绕过 validator 后调用旧 pack 也不能解决问题：原始签名、工具结果状态及扩展字段可能丢失。

`commandcode-native-request-red.log`：编译完成，**0 passed / 3 failed**，exit 101。
三项分别覆盖原生 rich 输入、canonical 修改优先权、公共 JSON／SSE 实际出站 body。

## 实现

- 同协议原生 plan 使用产品内 `native_request`；跨协议 Chat 转换仍执行原有共享 validator，
  不放宽其他产品或通用转换能力。
- 原生 history 未变时保留原始结构，包括 reasoning、签名、图片、custom／上游工具、
  `is_error`、cache_control 与未知 provider 扩展；model／stream 由已选路由固定覆盖。
- tools、tool_choice、reasoning、metadata、continuation 与 extra 同步 canonical 修改，
  工具子字段删除也不从旧 tool.raw 恢复。内部 extra 不能覆盖显式 canonical owner。
- 顶层 system／instructions 修改与 history 分开处理，保留未改动的 rich history。
  Messages 多条顶层策略不会只保留第一条；必需 max_tokens 缺省沿用既有 4096。
- Command Code 原生请求顶层保留 `gateway_`／`neuro_` 作为本地控制命名空间，不出站。
  不递归清理嵌套 metadata、工具参数或业务扩展；跨产品签名 history 明确拒绝。
- 固定 provider URL、本账号 Bearer、禁止 redirect、不重放的既有 sender 不变。

## 开发缺口与保护性边界

- `store=true`、background、previous_response_id／conversation 的远端状态生命周期仍未实现，
  继续显式拒绝，而不是删除字段后假装成功；不据此把整卡范围改为“不支持”。
- 官方合同明确拒绝的 Responses remote MCP 继续拒绝；这与上面的开发缺口不同。
- 当前共享 canonical 对 opaque Responses item 可能归一为相同空消息，Messages 也会拆分
  单个原生 item 并丢失部分 wrapper 属性。因此 rich history 本体被修改时，不能安全地按
  canonical 相等自动重配原始片段；当前明确拒绝不可逆编辑，不猜测、不恢复旧值、不丢字段。
  未改历史及只改顶层策略已接入保真路径。这不是将原生 rich 输入改写成“不支持”；
  若后续增加会重写 rich history 的策略消费者，必须同时补可追踪的 wire 身份，不能直接复用有损 pack。
- ZDR、System One/Jev、Studio 签发资格、适用 Server／多实例和当前源码手测包仍按总清单推进。

## 验证状态

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-first-three-20261113`。

- `commandcode-native-request-green.log`：首轮完整 Command Code 回归 **77 passed / 0 failed**。
- 随后加强工具子字段删除的夹具，明确包含旧 description/schema，并检查额外属性保留；
  `commandcode-native-request-final.log` 最终 **77 passed / 0 failed，exit 0**；
  Cargo 阶段 19m16s（包括共享构建锁等待），测试运行 74.46s。
- 最终测试 EXE 的共享 stream／Responses／Anthropic 回归分别 **53／60／60 passed**，
  见 `commandcode-native-request-shared-*.log`。
- `commandcode-native-request-feature.log`：独立 feature 检查已取得终态，exit 0；命令为
  `cargo check --locked --offline --lib --bins --no-default-features --features line-commandcode-provider-api -j 2`。
- `commandcode-native-request-checker.log`：checker 通过。
- `commandcode-native-request-ratchet-final.log`：最终 ratchet 通过，未改 baseline。
- 定向 rustfmt 与 scoped `git diff --check` 已通过。
- `commandcode-native-request-source-hashes.json` 保存本次源码指纹，最终回归后复核一致；
  本次源码 UTF-8 无 BOM 检查通过。

有效行：新增 native_request 186、native_request_history 94、请求测试 245、公共 wire 测试 36。
runtime 111→111；responses 145→143；messages 134→116；commandcode 249→251（模块接线）。
未调用真实上游、未改凭据／数据库／运行实例、未出包、未提交或推送。

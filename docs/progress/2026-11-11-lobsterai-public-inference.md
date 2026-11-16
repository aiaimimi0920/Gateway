# LobsterAI 本地公共推理切片（2026-11-11）

本批接续已验证的管理授权、目录保存与本地持久刷新，接通个人会话的公共 Chat Completions、Messages、Responses 文本／客户端函数工具请求。三入口都使用固定 Lobster Chat 上游。本地合成验证与真实账号验收分开；全部 67 项的完整验收分母不变。

## 本批行为

- 发送目标固定为 `https://lobsterai-server.youdao.com/api/proxy/v1/chat/completions`。专属连接池禁止自动重试；每次发送禁用跳转和继承头，使用自己的 Gateway 版本，不附带调用方 Cookie、Codex 身份、企业上下文或 refresh token。
- 使用已保存目录的 exact `modelId`；仅准入明确 `apiFormat: openai`、无特殊 runtime profile 的普通模型。未知模型、虚拟 `yd-fast`／`yd-ultra`、Kimi K3、非文本、reasoning、托管工具等本批不支持的语义明确拒绝。函数工具需要目录明确 `supportsToolCalling: true`。通用探测／目录与任意 passthrough 仍拒绝使用 Lobster 秘密。
- 根据实际响应 `Content-Type` 处理 JSON／SSE。非流式收到 SSE 时有界聚合；流式收到 JSON 时在成功准入后转换为标准 SSE。缓冲响应最多 16 MiB；这不是整个进程的峰值内存上限。
- Lobster 的 SSE 接受合法 `finish_reason` 或 `[DONE]`，仍等待严格 EOF；只收到文本、usage 或截断 EOF 均不能补成功。其他使用共享 Chat 守卫的产品继续要求 finish 与 DONE。only-DONE 时补出的标准 finish 是 Gateway 的协议转换规则，不是上游声明的具体停止原因。
- 文本增量返回。工具 id/name/arguments 可能分片，先按 index 有界收齐，待原始流完成校验后再交给公共转换器；不让 Messages 转换器补默认工具名。工具字段合计上限 16 MiB，完整工具、finish、usage、DONE 按序输出。错误或取消释放 HTTP、缓存和公共额度预扣。
- 内部目录／授权 context／generation 投影由配置编译器生成，不发送给上游。access/context/generation 在同一 material 锁下读取，发送票据绑定当前 owner。只有实际 sender 的 exact HTTP 401 才标记对应代次并通知唯一持久 worker；本请求不隐藏重发。worker 保留 Sending→receipt→Result→Published，后续请求使用 successor；403、429、5xx 不触发 token 刷新。
- Server 当前刷新缓存仍要求 expiry，公共推理在本产品 Server 路径明确拒绝；不伪造过期时间或 Redis TTL。

## 合同证据

本轮匿名联网重新核对官方固定 SHA `5d6621a0aed22adb53439e290234cc2d64318caf` 的 `openclawTokenProxy.ts`、`modelPresets.ts`、`constants.ts`、`modelRuntimeProfiles.ts`，URL、获取时间与 SHA-256 保存在本批 `contracts/sources.json`。

官方客户端按响应类型处理 JSON／SSE，不能据此声称 `stream:false` 必定返回 SSE。`virtual-models-v1` 还涉及独立 resolve 接口，本批未实现，因此不发送该能力声明。自有版本头及未声明能力时服务端是否接受，仍缺真实账号回执。官方源码只提供协议依据，不代表第三方账号或订阅使用资格已验证。

## 验证结果

最终 Rust 定向联合测试：**106 passed / 0 failed / 0 ignored**，其中测试名含 `lobsterai` 的 57 项通过。命令为：

```text
cargo test --locked --lib -- lobsterai protocol::commandcode:: protocol::openai::chat_stream token_refresh::tests::worker_ --test-threads=2
```

- 覆盖三公共入口、JSON/SSE 实际响应类型、finish-only／DONE-only、分片函数工具与两回合调用、用量未知保持未知、失败与取消退款。工具尾包顺序、错误时立即释放源流和缓存也有定向断言。
- 公共 401 在三入口与两种 stream 设置下都经过真实 HTTP sender，验证仅唤醒现有 worker、持久记录达到 Published 且保留 receipt、下一请求使用新 token、迟到旧代次 401 无效。403／429／500 不刷新、不重放。
- 相邻 CommandCode、共享 Chat 流守卫及 token worker 回归包含在同一轮中。代码行检查器 31 项通过，最终 ratchet、定向 rustfmt 与 diff 检查通过；本批源码与最终测试启动时 `source-r4.json` 的 SHA-256 一致。
- 保留失败历史：r1 是 `Vec<u8>`／`Bytes` 编译类型错误；r2 仅为已停止的 Cargo 排队进程，不算测试结果；r3 为 101 passed / 3 failed。r3 两项源于合成服务重复 Content-Type，另一项为旧目录头断言；修正夹具／合同断言后由 r4 重新验证，没有放宽生产响应类型判断。

本轮未重跑前批 UI／桌面 launcher，也未执行真实上游、原生 EXE 或独立 feature 构建。

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-lobsterai-inference-20261111`。基准 HEAD 为 `7969f12c85efceacf2a5e384e143b5c39a550981`，工作区有大量继承改动；本批用修改前备份、源码 SHA-256 和 scoped diff 单独定界，`wholeWorkingTreeProvenance=false`。

## 未完成边界

真实浏览器／账号的上游推理与续期、订阅资格／费用、自有版本标识的接受性、企业账号、虚拟模型 resolve、特殊 runtime profile、Kimi／其他专用工具协议、完整 reasoning／媒体、Server／Redis、独立 feature 构建与原生 EXE 仍未完成本产品验收。本批没有付费上游调用、出包、提交、推送、部署或重启运行实例。前批管理／UI／桌面 launcher 的结果保留，不冒称本轮重新验收。

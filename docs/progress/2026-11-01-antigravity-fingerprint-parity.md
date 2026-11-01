# Antigravity 指纹实际 A/B 与修复（2026-11-01）

## 结论范围

已使用本机原生 language server 与 Gateway 实际发送 owner 做 A/B，
不是仅对照参考项目 User-Agent。修复前实际不一致；修复后首个 ClientHello
归一化结构、已采样 HTTP 头和生成 framing 一致。整条应用行为并未逐字节复制。
本轮没有向 Google 提交生成、没有真实 OAuth/refresh 验收、没有模板标蓝，
没有出包、提交、推送或替换运行实例。

方法与边界见 [指纹对照](../antigravity-fingerprint-parity.md)。

## 已证实的原生请求

- 版本：Windows hub 2.15.0 / CL 983516863。
- 官方 SNI 下八个新连接 ClientHello 样本结构稳定。
- 正常 LS RPC 已实际产生 checkpoint 及 agent 的
  `/v1internal:streamGenerateContent?alt=sse`；替身目录使用测试 model/project。
- UA、Bearer scheme、头大小写及顺序、自动 gzip、单个 JSON chunk 均已采集。
- 初次 `loadCodeAssist` 和 `fetchAvailableModels` 的请求体已与 Gateway 做 JSON 语义比较。
- agent 外层字段集、固定字段和 requestId/sessionId 格式已实际比较。
- 不把原生额外的 checkpoint/listExperiments/fetchUserInfo 工作流复制到 Gateway；
  Gateway 仍是调用转发器，不是整个 IDE agent 的替代实现。

## 修复

1. 独立 `AntigravityHub215` TLS profile，不使用 wreq 默认 TLS。
2. 更新实测 UA，移除原生没有的 Accept，不混入浏览器/CPA 身份。
3. 生成始终使用 SSE 上游，HTTP/1.1 为单 buffered JSON chunk；自动 gzip 头后置。
4. 独立单例及连接池，不改变默认 CPA profile；代理来源复用既有 Gateway 策略。
5. 系统根与域名验证保留；专属 startup 私有 CA 设置，不引入证书验证 bypass。
6. native transport 不可用时失败，不静默换成另一种指纹，不自动重放付费生成。
7. 修正 standalone transport lock 中原有陈旧 source 记录：
   `rama-boring-sys` 仍为 0.6.20，只从 registry source 对齐到 manifest 已声明的本地 patch，
   增加既有 build driver path dependency；没有升级依赖版本。

## 验证记录

| 检查 | 实际结果 |
| --- | --- |
| 修复前产品 ClientHello 对照 | 失败，符合预期，保留原始二进制与日志 |
| 修复后产品 ClientHello 对照 | 通过；同一测试内 3 次独立采样 |
| HTTP framing 定向测试修复前 | 失败，固定长度正文无法满足原生 chunked 接收断言 |
| native transport crate | 21/21 通过（新增 framing、既有 body/proxy/cancellation/connection 生命周期） |
| 常规产品集成合同 | 13/13 通过（含 Antigravity、专用 OAuth 权限、CPA profile/wire、TLS 首航） |
| 本地 HTTPS 正确证书 | 通过；4 次请求复用 1 条 TLS 连接；公共 `/v1/chat/completions` 的非流式及流式均返回 200 和 OK |
| 本地 HTTPS 不受信任证书 | 安全拒绝通过，0 次应用层请求 |
| 本地 HTTPS 域名不符证书 | 安全拒绝通过，0 次应用层请求 |
| 独立 Python 归一化对照 | ClientHello 全结构及全部列出的 HTTP 比较字段通过 |
| 根库 Antigravity 定向单测 | 14/14 通过 |
| 代理来源/明确空 bypass 回归 | 7/7 通过，含新增 native bridge |
| checker tests | 31/31 通过 |
| effective-line ratchet | 通过；本轮生产文件均不超过 250 有效行，HTTPS 公共入口测试为 284 行 |
| root/transport rustfmt、diff whitespace | 全部通过 |
| UTF-8 无 BOM | 22 个本轮源码、测试、夹具和文档检查通过 |

本地 opt-in HTTPS 测试在本轮确实以显式 `--ignored` 执行，不把默认跳过算通过。
既有其他模块编译 warnings 保留，不在本轮顺手清理。

HTTPS 验证先在直接 owner 上通过，随后升级为公共路由入口再实际重跑三种证书场景。
升级后的第一轮沿用 10 秒替身预算而超时；公共入口含离线 Redis fallback 等待，
测试预算改为有界 45 秒，最终 trusted 用时 10.33 秒并收到四个预期请求。
没有放宽产品上游 timeout、TLS 验证或响应断言。

## 对照数值

```text
native JA3 MD5:         9b7dcdf3f997f1fb7b4409c94cb7ef36
gateway before:        6aac88aefeb1c3f8b576782dbe085b6e
gateway after:         9b7dcdf3f997f1fb7b4409c94cb7ef36
normalized shape SHA:  8fe535705736dbae0ddaa33613d697788c73c2b1bf660f4e4c7a13dd24553273
record length:         1537
ALPN:                  absent
```

归一化只排除真正随机的 client random、session ID 值和临时公钥内容；
这些值的长度、分组与排列仍参与检查。不是仅比较 JA3。

## 有效行与源码审查

权威 lexer 测量：native transport lib 120→152，dial 100→106，pool 219→225，
wire 189→227，Antigravity fingerprint 28→10，transport 97→92，tests 169→186。
新文件：TLS profile 45、profile dispatcher 23、native owner 57、wire test 83、
proxy bridge 46、产品 TLS test 114、HTTPS 公共入口 test 284 有效行。
proxy policy 179→187，factory 18→20；Antigravity mod 相对加入 native 接线后的快照 126→122。
没有修改超限基线、没有新增例外记录。

审查边界：固定 origin、凭据头脱敏、不接收请求控制 TLS/CA hook、
16/32 MiB 上限和 deadline、独立 immutable profile/pool、取消清理、无自动生成重试。
本轮启动的原生子进程已退出；没有修改已安装客户端、设备 ID、系统代理或系统根证书。

## 证据位置（本机隔离目录）

`C:/Users/Public/nas_home/AI/GameEditor/linshi/antigravity-fingerprint-20261101/`

- `native-smoke-1793542984175878000/`：官方 SNI 的原生 TLS。
- `native-smoke-1793545487525250100/receipt.safe.json`：原生 HTTP/生成请求；认证头已去值。
- `gateway-before.bin`、`gateway-before.log`：产品修复前。
- `gateway-after.bin`、`gateway-after.log`：首轮修复后。
- `product-regressions.log`：最终产品集成及 HTTPS trusted。
- `http-framing-before.log`、`native-transport-regressions-final.log`：framing red/green 及最终 21 项回归。
- `https-untrusted.log`、`https-hostname.log`：证书负向验证。
- `public-https-trusted.log`、`public-https-untrusted.log`、`public-https-hostname.log`：升级公共入口后的最终回执。
- `antigravity-unit-tests.log`、`proxy-policy-tests.log`：根库 14 项及代理 7 项回归。
- `https-fixtures/gateway-http-*.json`：安全 HTTP 回执；不含原始 token。
- `comparison.safe.json`：逐字段 A/B 汇总。
- `lines-before.json`、`lines-proxy-before.json`、`lines-after.json`：有效行测量。

测试 CA/key 仅用于隔离替身，不安装、不发布为生产信任材料。
尚未验证：真实 Google 生成、全部模型与工具、多轮 agent 行为、HRR/恢复、
ML-DSA-only 服务端、其他 OS/客户端版本、无 native feature 构建、正式发布和 Docker runtime。

# FreeBuff：业务状态隔离、请求字段所有权与当前协议复核

日期：2026-11-06。本批承接[正常 UI 与本地管理闭环](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-06-provider-channel-expansion-freebuff-local-ui.md)，没有重复登录 UI、SQLite 保存或 runtime 重建。

**结论：修复两类已复现的业务 ownership 缺陷；不是当前 FreeBuff admission、typed 登录公共推理或整渠道完成。** 总目标仍 active，模板仍 16，不新增整卡 `offline_ready`，R06/W04、P50/P60/P70 继续。

## 1. 先读实际代码，再核实当前参考

既有公共 pipeline 的 FreeBuff 分支调用旧 `execute/execute_stream`，配置仍依赖手工 agent map，session 使用旧自动 POST/轮询。typed fingerprint receipt 在配置层限制产品与秘密保存，并未自动实现当前业务协议。正常 Console connectivity probe 仍明确 unsupported；旧 account probe 的 START/FINISH 不能冒充该 UI probe。

本次通过 GitHub connector 联网只读核实 `CodebuffAI/freebuff`，固定 commit `6f567ec21542afc07dbe447bce0dcaf7a5a73a8d`。同版本[根 LICENSE](https://github.com/CodebuffAI/freebuff/blob/6f567ec21542afc07dbe447bce0dcaf7a5a73a8d/LICENSE) 前 20 行及 blob SHA 已取得，确认为 Apache-2.0。采用方式仅为协议事实参考，新增 Rust 独立编写，不复制/执行外部 TypeScript、SDK 或远程 agent，不由许可推定厂商客户端身份授权。

关键差异和固定来源汇总在[业务参考](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/protocol/freebuff/REFERENCE.md)：

- 当前 CLI POST 是独立 `/session/admission`，GET/DELETE 另走 session；reuse 是“只复用指定 live instance、不购买/抢占”的另一条路径。未知 POST 结果不可盲目重投。
- admission 可能消耗 Freebucks，wallet header 不是余额上限，消费确认在客户端完成。`cost_mode=free` 或 FINISH 的零 credits 不是钱包零扣额证明。
- catalog 的持久 key、账号短效 handle、fetchId 不可混用；catalog 模式还涉及设备 Ed25519 签名，覆盖实际发送 bytes。公开客户端也有 fallback，但这不是所有旧模型/旧 session wire 当前可用的证明。
- 业务 URL 有部署配置来源；新 typed FreeBuff origin 与公开 SDK/CLI 默认 Codebuff 地址的授权关系仍未完整核实，不能自动跨 host 携密。
- 参考源码引用的 `docs/freebuff-model-catalog.md` 本次 fetch 为 404；只采用实际读到的类型/代码，不冒称读到了该文档或未公开服务端。

公开返回、blob SHA、失败 fetch 与文件摘要留在[来源清单](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-business-ownership-20261106/source-references.json)。这不是平台在线可用性或真实权益验证；旧登录固定参考未被本次业务来源替换。

## 2. 实际修改

### 2.1 同一账号换授权，不借用旧 session/run

旧 `credential_subject_id` 一旦存在稳定 credential ID，就只返回该 ID：同账号换 token/target 仍命中旧 session/run。现在将规范化 base URL、当前 trim 后 token、trim 后 credential ID 一起做长度定界的 SHA-256 namespace，原材料不进入 key。保留 Codebuff URL 既有规范化及等价空白规则。

这是进程内缓存身份，不改变持久 credential ID、token、数据库 schema 或既有 receipt。旧 namespace 的在途 owner/缓存继续由原有有界 registry/lifetime 持有；本批不实现换 token 立即撤回、注销远程 session 或跨进程协调。

### 2.2 run/session 声明由实际执行器产生

同一 pack owner 同时用于 planner 与实际 chat body，清除公共 caller 的 runtime 配置字段。普通 custom metadata 和普通生成参数保留；`run_id/cost_mode/client_id/surface` 与精确 `freebuff_` 前缀 metadata 是保留字段，先移除，再由实际执行器补真实 run、cost/client 和可选 instance。无真实 instance 时不能残留 caller 声明。

非 object `codebuff_metadata` 归一化为空 object，不能借 string/null/array 等形状压掉必需 envelope。旧测试中“opaque 原样保留”的预期随这一明确边界修正，不再把它作为兼容承诺。未改变通用 OpenAI packer 或其他服务商。

## 3. 真实回环证据与限制

新增 6 项测试：4 项配置/packing 回归、2 项随机 127.0.0.1 TCP 测试，直接调用生产 FreeBuff executor/run owner。

1. 同一 credential ID、同一 runtime owner 连续使用两个合成 token，各自取得 session/START/chat/FINISH；检查实际 chat 中 run/instance 与该次授权一致，shutdown 等待各自 FINISH。
2. 合成上游返回 disabled session，实际 chat 不转发 caller 伪造的 instance/multi-session。

peer 只模拟旧 generic wire，不删除 typed receipt 来冒充新产品。32 KiB 请求上限、最多 16 条记录、client/shutdown/stop 各有 3 秒预算；正常路径 await run shutdown，再 graceful stop/join peer，失败路径 Drop abort。所有进程内状态随测试进程退出，不增加常驻服务或未跟踪后台任务。

未建立“已保存 typed 登录账号→公共 router→当前 admission/catalog→JSON/SSE/取消/本地结算”的整链，也没有本轮新的浏览器 UI/Server/跨进程/原生 EXE 证据。正常 Console probe 的 unsupported 语义未改。

## 4. 检查结果与保留失败

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-business-ownership-20261106`。

- `ownership-red`：新 fixture 漏传 shutdown budget，`E0061` 两处，尚未进入测试；仅补正确调用参数，保留原日志。
- `ownership-red-v2`：实际执行 6 项新回归，**0 passed / 6 failed**，冻结 3216 项选定输入无漂移；确认 token namespace、伪造 instance、非 object metadata 和 runtime extra 逃逸，不把编译失败当行为红测。
- `affected-final`：**163 passed / 0 failed / 1 ignored**，包含上述全部 6 项；冻结 **3217** 项选定输入无漂移。ignored 是前批显式 browser host，本批未再次启用。按完整测试名去重，不叠加历史 157 项或重复计算红测。
- `production-check`：`--locked --no-default-features --features runtime-defaults --jobs 4 --lib` 生产库检查 exit 0，另冻结 3217 项输入无漂移；最终审计再次逐项核对上述两个最终阶段的输入。
- 最终审计首轮把新增文件 `git diff --no-index --check` 的空输出 exit 1 误判为失败；仅修临时审计对 no-index 差异退出码的分类，仍要求无任何 whitespace/error 输出。原脚本与失败摘要保留，不改源码/生产语义或测试来凑绿。
- Rustfmt、checker **31/31**、ratchet **4577** 项通过；14 个 immutable runtime assets 单列，`governed source >700=0`。7 项源码最大 271 有效行，两个新文件分别 191/126；不改 baseline/exception/policy。两项生产逻辑 owner 为 config/request，root 仅 cfg(test) 接线。
- 正确仓库根 `artifacts/effective-code-lines.json` 已直接备份、归档新扫描并精确恢复原 bytes/hash，不借历史副本冒充本批直接备份。最终 UTF-8 无 BOM、scoped diff/链接及来源复核见 [closeout.json](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-business-ownership-20261106/closeout.json)；独立只读复核无阻塞 findings，限制原样保留。

前序扩大 Python scanner 的既有失败未重跑或宣布清除；不报全仓绿。没有依赖升级、出包、提交、推送、部署或在线实例操作，没有读取真实 `.ng`/历史凭据、真实认证/生成/付费请求或容器操作。P34 授权此前已消费清理，本批未复用。

## 5. 下一闭环

先在现有框架内明确业务 origin、当前 catalog/设备 key owner、显式 admission/reuse/消费确认和模糊结果处理，再接精确 model/agent 与 typed receipt 的公共 JSON/SSE/取消/退出。不能为了跑通合成 peer 发明默认 agent、静态额度、零费用或额外认证类型。

Server、跨页面/进程、独立进程重启/原生包与真实集中录制验收另有退出条件；其他原始渠道和 P50/P60/P70 均未被本批抵消。

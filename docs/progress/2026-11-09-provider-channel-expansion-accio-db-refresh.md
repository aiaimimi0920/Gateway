# Accio DB 刷新存储与发布：存储合同已验证

日期：2026-11-09。继续 R01 的数据库自动刷新缺口；总目标和 P50/P60/P70 不缩小。工作区仍为 C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway，未提交、推送、出包或修改现有实例。

## 已实施的代码

新增真实 PostgreSQL authority snapshot、永久 grant/head journal、先保存 Result 后发布、原 authority CAS 和 object 新键发布。复用共享 `GrantRecord` 状态机与协议材料 parser，不拼临时 route YAML，不把 xmin 或 TTL lock 当永久消费证据。普通 hydrated reader 的旧 Accio 投影已提为同一函数供 snapshot 使用，原算法不复制。

交叉核验发现并修复两个并发/误写问题：普通管理更新先读旧 payload 后盲 upsert 可能覆盖 successor，现改为同一读取的短期 xmin CAS；publisher 误把未认定为 access alias 的 root `token` 当认证副本删除，现保持它，只清理 parser 实际认识的层级。

实际健康 cooling 与禁用分别处理。Active/cooling 属于同一 enabled 权限桶，冷却期间不发布，正常恢复可以应用原已确认 Result；disable→restore、归档/转移/重建或 payload UPDATE 仍形成新权限边界。显式清空 DB refresh endpoint 不重新从保留 client/token 推导并启用。

Schema 扩展位于 `deploy/postgres/initdb/005-gateway-refresh-authority.sql`，正常 schema 仍由 Platform owner 审核，不手改 001、不自动 DDL、不应用到现有数据库。部署/回滚/秘密和旧 object writer/deletion 边界见 [合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/provider-accio-db-refresh.md)。

## 最终源码对应的验证证据

证据目录：C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-accio-db-refresh-20261109。

- 实际 PostgreSQL schema probe 通过：同脚本两次执行、诊断更新、同键 payload UPDATE、disable→restore、新 incarnation、删除保留 grant，事务结束回滚隔离 fixture。
- 真实新增 cooling 红测报 `temporary cooling revoked refresh authority`；修复后的同样 schema probe 通过。不以编译错误冒充行为红测。
- 初始四项 snapshot 红测最终运行：**2 passed / 2 failed**；此前一次 E0425、一次 LNK1104 只记构建失败。E0425 为并行 Nous 调用了不存在的 `ensure_payload_compiled_in`，仅改为真实接口 `ensure_payload_compiled`，不改其协议/测试。LNK1104 的目标 EXE 被现存其他测试进程占用，未终止或删除该进程/文件；转正常默认特性构建后得到真正的四项红测。
- 最终 r2 测试构建 exit 0，722.312 秒（包含共享 build lock 等待，不是纯编译时长），构建前后记录的输入无变化。冻结测试程序 `r2.exe` 的 SHA256 为 `979933fff2554bf0ec8e2794cfb83473d64f194f7147799bbad9b80e84024a00`；这是 lib test harness，不是 Gateway 手测包。
- `db-r2`：**12 passed / 0 failed / 0 ignored**，包含六项 snapshot、五项真实 PostgreSQL 与一项 object rollback 合同；显式 endpoint clear、正常 cooling 恢复、迟到管理 snapshot 和 root 非认证 `token` 保留均在最终程序中运行。新增用例不宣称每一项分别有独立红绿。
- 同一冻结程序的相邻检查：Accio **134 passed / 6 ignored**、普通 payload **20 passed**、共享 receipt **2 passed**、token-refresh **163 passed / 4 ignored**。按完整测试名去重后为 **300 passed / 0 failed / 4 ignored**；普通 Accio suite 中跳过的六项已由 `db-r2` 实际执行，不保留为未验。其余四项是既有 real-Redis opt-in，本批未执行。
- API/runtime-defaults 生产 lib 检查 exit 0，101.531 秒。只移除 Accio leaf、保留其余 runtime-defaults 的生产 lib 检查也 exit 0，786.594 秒（包含等待其他 owner 的共享 build lock）；沿原 handle 完成，没有另起重复构建。关线检查前后记录的输入无变化；不以等待时长承诺构建性能。
- 最终 **18 个 Rust 文件的定向 Rustfmt** 和 `git diff --check` 通过；checker **31 passed / 0 failed** 复用本批先前、未改 checker 输入的证据。endpoint-clear 修改后的最终 ratchet exit 0，扫描 5184 个文件，14 个 immutable runtime artifacts，governed source >700=0。没有修改 baseline/例外。

首轮 `green-build` exit 0，其冻结程序的 12 项也曾通过；最终 endpoint-clear 补充发生在该轮实际 rustc 启动之后，因此首轮不作为最终源码证据。最终判断使用上面的独立 r2 和 `db-r2` receipt。`green-build`、`queued-final(-r2)` 保留历史输入及进程快照，不冒称全共享工作区被冻结。

开启 Accio 的生产检查期间，其他 owner 修改了 `src/protocol/nous_portal/wire_tests/fixture.rs`；该文件是测试夹具，不属于此生产 lib 的消费输入。`src/protocol/nous_portal/runtime.rs` 的 preimage diff 还含该 owner 的原生请求校验扩展，本批仅归因接口名的一处编译修复，不接管这些行为或将其余差异计成本批实现。所有并行改动保留；不宣称最新全仓联合回归。

官方 lexer 的本批范围测量：credential 入口 105→107、payload 193→201、snapshot updater 154→172、HTTP mutations 239→243、import parser 131→139、grant 入口 201→202、record 274→300。新模块最大为 database tests 257 行，snapshot 201、publication 136、journal 123，SQL 41；全部本批文件 ≤500，无新增超限、无 >700 旧债扩张。Nous runtime 76→132 的合计差异主要是其他 owner 的扩展，不计作本批接口名修复的行数增长。

最终范围审计采用官方 lexer、UTF-8/BOM 检查、完整测试名与 r2 输入 hash，清单为 `final-scope.json`，含 24 个本批相关文件（18 Rust、1 SQL、5 文档）的 hash/行数/所有权说明。Accio-owned 源码与最终 r2 输入一致；共享记录中当前仅上述 Nous 测试夹具有后续变化。临时 Node 审计入口首个 preview 因 Windows ESM 不支持 `C:` import 返回 `ERR_UNSUPPORTED_ESM_URL_SCHEME`；仅将临时脚本 import 改为合法 file URL 后 preview exit 0，不修改生产源码或放宽检查。

## 资源与未完成

本批收到新的明确同意后，创建 run `ef5ed892d7524147acc68dcff73f1e34` 的一对 PostgreSQL/Redis 容器，缓存镜像、仅 loopback、tmpfs、无现有卷，Redis 无持久化。最终 PG/object 测试后已按原 ID/name/label/mount 校验，仅停止并删除这一对，`containers.json.status=cleaned`。45 个既有容器的 ID 集合及卷集合不变，无本批容器残留；不执行 FLUSHDB、不操作旧库、旧容器、卷或服务。本次授权已消费，P34 和上一批读取兼容授权均不复用。

Redis 本批只作 fixture guard，不能报 Redis cache/Lua CAS 已验证；测试前后也未调用任何真实供应商。对象 fixture 仅操作证据目录下本次创建的键，生产 predecessor/staged 对象的保留语义不等于自动垃圾回收。

后台 Accio steward、一次专用发送的 shutdown/unknown/restart 生命周期、Redis 缓存 CAS/修复和正常服务联合尚未接完；新 journal 或能读旧 payload 不等于实际 DB scheduler。旧 account object writer/deletion 的全部联合对象语义未在本批重写。真实 client/wire/模型/额度、完整取消 audit、全部运行模式/进程/包及其他原渠道继续。

模板仍 17，无新增整卡 offline_ready；总目标 active。未读取真实凭据、调用供应商或改动现有数据库/容器/卷/服务。

## 下一接力闭环

1. 由独立 DB Accio owner 接入正常 `provider_credential_refresh` 启动、批量查询、期限判断和 shutdown/join；不得给现有仅 ChatGPT 的 adapter 列表加 Accio 后继续调用通用表单刷新或 TTL lock。
2. 原 DB reserve commit 明确确认后，复用 `protocol::accio_oauth` 的专用单次 HTTP；Sending 未知或 acknowledgement 丢失禁止重发。原 Result 先保存，再发布；存储等待、撤权、删除和 shutdown 不丢已确认轮换材料，不派生新的 attempt。
3. 接实际 DB authority 与 Redis cache CAS/修复、正常请求读回和 restart。先证明持久 Sending/Result/Published 在新 runtime 中的实际消费，再推进完整 server/跨进程/包；route-config 的既有 cache-repair 测试不能代这条 DB 链。
4. 旧 account object writer/deletion 的锁前 side effects 仍需按真实调用路径审核，不以本批局部保护承诺全流程对象原子性。005 经 Platform schema owner 审核和生产迁移仍未执行。

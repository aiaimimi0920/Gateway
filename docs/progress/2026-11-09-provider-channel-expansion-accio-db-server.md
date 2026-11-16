# Accio DB server／管理／public 子闭环已验证

日期：2026-11-09。接续已验证的 DB worker 子链，完整分渠道目标及 P50/P60/P70 保持 active。工作区 C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway，main / 7969f12c85efceacf2a5e384e143b5c39a550981；保留其他 dirty edits，不提交、推送、出包或修改现有实例。

## 本批实际入口和限定范围

- 新 opt-in 合同沿真实 `runtime::build_app_state` → `spawn_background_tasks` → `run_gateway_server`，不是直接调用 Worker.tick 或 Router.oneshot。正常管理 TCP 创建账号和凭证，再由后台 worker 读取真实 DB 行、执行专用单次 HTTP、保存并发布 Result。
- 原始数据库仍绑定固定 `https://phoenix-gw.alibaba.com` 业务受众、原权限代际、显式 client／refresh／精确绝对 expiry；不将账号 base URL 改成 loopback 来跳过 refresh authority，不注入临时 route-config 凭证。
- 库测试编译专用、run-scoped loopback peer 覆盖最终推理 transport、后台刷新以及正常管理缓存 miss 的额度请求。保持原 RequestPlan／body／headers／query，只有最终目标被替换；scope 不存在时不影响既有测试，生产编译没有此设置、路由或 origin override。完整 server 的测试 handler 只加 transport scope，不改变原鉴权和路由。
- 同一新数据库内每测试使用独立 UUID schema。使用仓库原 001／002／004／005 SQL，仅转换 schema qualification 和 search_path，原 migration ledger 的 inline COPY 用 SQLx COPY 协议加载并核对全部行数；保留真实列、约束、默认值及 account audit FK，不采用 standalone route-document 的003放宽，不自动迁移已有数据库。
- 已验证 Chat／Responses／Messages 的 buffered／stream、调用方材料不覆盖可信 DB token/device、普通管理秘密脱敏、DB publication 后普通 public 请求使用 successor、同名 hosted `gw:cred` 不参与 DB 消费、不被本轮发布误写、真实 DB audit 和新 AppState／server 恢复不重发。
- 该组 public 使用正常共享 `GATEWAY_API_KEY` 和 bootstrap project，**不是** unified access-key/bundle/prepaid 的联合证据；那些身份和结算路径仍需独立推进。新 AppState／server 重建不是 OS 进程强杀／restart 或原生包验收。

## 验证进度与来源

证据目录：C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-accio-db-server-20261109。

- 最终官方 lexer：入口14→24、lifecycle46→55、router45→48、profiled sender81→86、quota HTTP169→170；五个新 server 模块为160/135/192/82/146。10个 Rust 文件均≤500、UTF-8无BOM，未改 baseline/例外。
- 三项 guarded 完整 server 合同在默认线程栈下 **3 passed / 0 failed / 0 ignored**。覆盖正常管理创建 DB account/credential、未授权 public 401、真实 quota miss 私有回环、轮换前后及第三次 server rebuild；drain 中原500ms HTTP收尾并保存Published；HTTP500未知结果保留永久Sending，重建后的正常管理不重发、DB access保持原值且无推理调用。不是仅新增源码或编译通过。
- 相同冻结程序的 Accio／DB refresh／旧reader／材料／管理／专用transport／额度相邻合同 **168 passed / 0 failed / 0 ignored**。按完整名称去重连同三个 server 合同共 **171 passed**；清单为 `r5-adjacent-selected.json`、`final-passed-tests.json`，不是全仓或所有渠道回归。
- 首轮构建于247.625秒主动取消，exit101；已确认普通管理 quota cache miss 会使用独立 HTTP client。测试尚未运行，为补齐无真实上游的隔离而仅停止已核验 ID／parent／Gateway target 的本轮 rustc，不因观察超时重开。保留 `first-build-cancellation.json` 与原日志，不把取消当行为红测。
- quota 私有回环接线完成后，r2 使用 locked/offline、4 jobs 构建，exit 0、580.828秒；冻结 EXE 为 `9ea671a0f89654e7c067c6155f4c28c67125799145eac41f1ae1df49126fce91`。三个默认栈用例0 passed/3 failed，暴露 inline COPY 不能整段交给 SQLx raw_sql（42601）；只修测试 adapter，以 SQLx COPY 保留全部原记录和约束，不改 SQL，不把 fixture 失败当生产行为红测。
- r3构建 exit0、563.485秒；三个用例初始化真实schema后，被 fixture namespace 的冒号/长度触发原 `console_invalid_redis_namespace`，0 passed/3 failed。只改为合法、逐测试独立的schema名。r4构建 exit0、541.437秒；三个用例进入实际管理TCP后被原校验返回400 `payload.adapter 不能为空`，0 passed/3 failed。核对真实DB validator、credential normalizer、typed payload和nested flow的deny_unknown_fields后，只补齐fixture payload.adapter和原snake_case flow，不放宽生产reader/validator。原日志和冻结程序均保留。
- 最终r5构建 exit0、552.235秒，沿原handle完成；构建/运行源码未变，包含直接消费的两个外部Rust fixture。冻结 `r5.exe` SHA256为 `1446d54edf8aba5030673f036d5806b47a50f236d8df79a16c0e5ec971b66551`。这是lib test harness，不是Gateway手测包或全前端构建凭据。
- 不带management-ui的生产lib：全部runtime leaf开启 exit0、80.844秒；只移除Accio、保留其他runtime leaf exit0、73.906秒；两轮源码未变，避免触发Web publication/snapshot cleanup hook，不冒称全部edition/feature-off矩阵通过。
- 最终10文件Rustfmt和ratchet exit0（5202文件、14个immutable runtime artifacts、governed source >700=0）；checker31项复用本批未变checker输入的通过记录。全仓fmt仍仅有范围外 `src/provider_runtime.rs` 的 `mod dim_model_probe` 排序失败，保留原日志、未接管该owner，不报告全仓格式绿。最终diff、编码、源码/资源终态见 `final-receipt.json`。

## 新授权与资源

本批收到新的明确“同意”后，仅新建 run `15488d2b0f2c41809b66d9a831f081ef` 的 PostgreSQL／Redis 容器对；缓存镜像、只发布127.0.0.1、tmpfs、无既有卷，Redis关闭持久化，两服务验证同一run guard。全部验证后已按原ID／name／label／mount校验，仅停止并删除这两只；`containers.json.status=cleaned`。45个既有容器ID集合和全部既有卷集合不变。没有FLUSHDB、不改现有数据库/容器/卷/服务，未读真实凭据或调用供应商。本次授权已消费，不复用旧授权。

## 继续保留的缺口

本共享Key server子闭环已取得上述运行证据；完整 unified key／bundle／prepaid／额度与各运行模式、OS独立进程强杀/restart、包、真实client/wire/模型额度和所有取消边界仍未完成，tmpfs/测试PG设置不证明生产durability。旧通用 credential-cache poller／provider probe scheduler 等任务未观察全局shutdown，不能把Accio handle已join扩张为全部后台资源已join。下一接力推进实际identity/结算与lifecycle owner，再补独立进程/devtest；不重做已验证DB worker或本共享Key子链。

旧 account object writer／deletion 的锁前副作用、005经Platform schema owner审核和已有服务器正常迁移仍未完成。模板17，无新增整卡offline_ready；P50/P60/P70及完整目标未完成。

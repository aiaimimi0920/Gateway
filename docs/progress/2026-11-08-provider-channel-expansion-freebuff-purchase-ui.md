# FreeBuff：正常账号的逐次购买确认 UI

日期：2026-11-08。工作区：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`；`main` / HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`。保留继承的未提交修改；HEAD 不是本批 dirty 源码的交付凭据。没有提交、推送、出包、部署或操作现有实例。

## 1. 本批完成与未完成

承接[请求拥有的一次性批准](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-07-provider-channel-expansion-freebuff-request-consent.md)，把已有管理 list/decision 接到正常已保存 FreeBuff 账号编辑页。手动读取当前待确认请求，再查看完整报价，显式批准或取消；前端不生成报价、不持有购买许可、不直接购买、不启动模型，也不保存账号配置。

**公共 producer 仍未接，目录模式仍返回 `409/freebuff_business_admission_required`。** 本批退役的是正常 UI 代码及其组件/浏览器合成验证缺口，不是公共购买、公共推理或 UI→真实管理 TCP→活请求消费的联合验收。不存在让管理员在空闲时创建购买的入口。

## 2. 改了哪些内容

- [API](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/apps/desktop/src/api/console/freebuff-purchases.ts)：严格解析最多16项 pending、host-issued v4 pending/quote ID、request ID、两个 SHA-256 和完整 typed meter；拒绝重复身份、坏数值、额外秘密字段及 foreign provider/credential/revision。复用正常 `GatewayApiClient` 的管理 token / SecretGrant、same-origin、no-store、拒绝 redirect 通道。
- [请求状态](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/apps/desktop/src/features/console/useFreebuffPurchases.ts)：不轮询、不自动请求或重试；同步 ref 防双提交。账号、API client、revision、管理 token、Grant、禁用状态或业务目标变化时立即隔离旧视图，并取消原请求、忽略迟到回包。决定只可引用当前实际显示的原结果；发出决定即消费旧视图，成功、失败、未知或停止等待后均需重新显式读取。
- [确认视图](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/apps/desktop/src/features/console/FreebuffPurchasePanel.tsx)：显示稳定模型、会话价格、总/每日/钱包余额、预计钱包支出、赠额/折扣/flags、原请求 ID 和 caller/body 摘要。可选事实缺失显示未知，不造0；上游 `spendWithoutAsking` 也不免除显式批准。报价短时有效，不锁价、不承诺最终扣额或精确倒计时。
- [正常接线](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/apps/desktop/src/features/console/FreebuffCatalogPanel.tsx)：仅已保存业务目标的账号展示；复用 `ConsoleGlobalDialogs` 原有 saved-account tools，不增加另一套全局弹窗 owner。未保存账号字段、dirty/autosave/锁定或 revision 不一致时禁止操作；购买确认网络等待期间禁用冲突的目录/设备/claim操作，不把自身 busy 回送为自身 disabled。
- [样式](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/apps/desktop/src/features/console/FreebuffPurchasePanel.css)：复用 Neuro 组件和现有主题，没有新主色或替代弹窗。列表/完整报价独立有界滚动，长摘要可换行；窄屏改为单列。

`decision` body 只有 `providerId / credentialId / expectedRevision / pendingId / quoteId / approve|cancel`，不把显示的价格、caller/hash、prompt 或 token 当授权输入。`delivered:true` 只显示“决定已交付”，不显示“购买成功”；停止等待明确不撤销可能已交付的决定，不自动重放。报价/动态管理权限/原请求 binding 与 durable admission 仍由既有 Rust owner 负责，本批未修改它们。

## 3. 实际验证

证据根：[gateway-freebuff-purchase-ui-20261108](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-freebuff-purchase-ui-20261108)。包含直接 preimage、命令/log/JSON测试身份、各阶段选定输入 hash、范围 diff、有效行、截图及最终 audit。

| 检查 | 结果与边界 |
| --- | --- |
| 首条行为红测 | 正常 `ConsoleGlobalDialogs` 找不到“读取待确认购买”；1 failed / 19 skipped，证明原入口确实缺失，不是缺符号编译红测 |
| 最终 focused-v2 | 3文件70 passed / 0 failed，含51项新增（API25、panel18、正常host8）；不是70项全为新增 |
| 最终 adjacent-v1 | FreeBuff前端及共用API client共13文件262 passed / 0 failed，包含上方70项，不相加 |
| 测试身份修正 | 终审发现null/Infinity/NaN经`%j`打印为相同测试名，实际为3个不同输入；只在名称加入case index，不改断言/输入/生产语义。api-identities-v3重跑25/25；其他237项按未变源码复用，最终262个不同file/fullName身份见test-identities.json，不叠加重跑数 |
| typecheck-v2 | 真实 `npm --prefix apps/desktop run typecheck` 通过 |
| 隔离生产网页编译 | `build-v2.log` 第一段真实配置/main入口编译通过；web目标输出到本批 linshi 的 `production-web-v2`，关闭清理、不启用现有网页发布/剪裁插件。不是正式 build:web 发布、EXE或交付包 |
| 合成浏览器编译 | build-v4通过真实生产组件 harness；临时构建脚本问题见第4节 |
| 真实 Edge 浏览器 | 1280×900及390×844深色/减弱动效下检查完整报价；窄屏 quote clientHeight384 / scrollHeight698，实际滚轮 scrollTop314，无水平溢出（308/308）。Escape关闭并恢复“查看并批准”焦点，仍仅1次只读；Enter重新打开、Tab/Enter显式批准后恰为1次read+1次decision，展示“决定已交付”。控制台0 errors/0 warnings |
| checker / ratchet | 31/31；4921文件通过，14项immutable runtime资产单列，governed source >700=0。baseline/exception不改，默认根报告按直接preimage恢复 |
| 范围审计 | 9份源码/测试/样式，最大219有效行；新增最大92，既有catalog67→71、正常host测试173→219。12份范围文本UTF-8无BOM、范围diff及Git空白检查见最终audit |

浏览器通过本批安装缓存中的 Playwright CLI 使用隔离 Edge session，未使用用户浏览器 profile；仅加载回环静态页。harness 使用真实组件和 API client，但响应是合成 `fetchImplementation`，**不是 Rust管理HTTP、真实pending producer或FreeBuff上游**。临时浏览器已关闭，自建静态监听已结束并核无监听；现场保留。各阶段记录935项所选前端源码/配置，阶段内稳定；最终只有上表已精确核对并重跑的测试名一行变化，生产消费输入保持原hash，类型/构建/浏览器及有效行按不变语义复用。这不是整个仓库、node_modules或发布包的完整来源声明。

本批没有修改 Rust，未重复运行其既有编译/单测或完整 feature matrix；不把旧结果说成本轮重跑。项目没有配置 TS/CSS formatter 命令，本批不安装新格式化依赖，使用既有代码风格、类型/编译及空白门禁。没有重新执行整仓前端回归、正式安全审计、真实上游或付费测试。

## 4. 失败记录与修正

首条预期行为红测保留于 `red.log`；随后 focused-v1 的63项通过，扩充正常host覆盖后最终70项通过，不与中间数相加。首次有效行调用误用 `typescript` 语言名，lexer明确拒绝；改用项目 `languageForExtension` 后完成真实计数，没有修改checker或baseline。

临时浏览器构建三次失败均在本批 linshi 脚本：第一次从不导出的 `@neuro/ui/package.json` 查路径；第二次目录alias无法解析导出的样式子路径；第三次CommonJS `require.resolve`不匹配该包的import-only root export。读取实际已安装package的exports后，将临时harness alias明确指向其现存dist/index.js和dist/styles.css，build-v4通过。没有为测试改变生产模块解析、升级依赖或修改正式发布配置。build-v1/v2各自的生产main编译已通过，后续仅重做失败的harness部分；整体脚本失败不冒称全阶段通过。

## 5. 接力顺序与总体状态

1. 公共 `stage_send` 的真实candidate/model准入之后、并发permit之前，增加catalog-aware请求组合owner：当前quote-time snapshot、原精确账号、真实caller、同一冻结harness意图→quote→原request-owned pending。不能由管理API创建闲置报价，不能采用早期cash snapshot或重新调度换账号。
2. 明确区分恢复既有claim与新购买；批准后重新capture同一binding，让既有 `ApprovedRequest::admit` 消费原许可。等待留在原请求future、原deadline与取消owner中，不detach、不包装或取消预扣事务、不增加outbound attempt。
3. 原active claim→CatalogRun/root/decoder/显式FINISH→公共JSON/SSE及TrackedStream/closeout/cancel，保持原费用和身份；仅在接线与相应测试完整后替代具体guard路径。工具/child是否启用按实际能力实现，不能把默认CLI全套工具当作已证上游强制前置，也不能用prompt/schema冒充已执行工具。
4. 联合验正常UI→真实管理TCP→活请求→合成上游、撤权/取消/变价/未知义务、下一次调用及适用probe，再继续其他渠道和P50。

模板仍17；无新增整卡 `offline_ready`。FreeBuff公共业务、完整harness/实际工具/child与费用、持久run/history/远端lease、Server/跨控制台/进程/原生包，及其他平台剩余功能均保留。**P50无凭据全范围集成与devtest、P60集中录制、P70真实逐渠道验收与总目标未完成。** 本批没有使用真实凭据、真实服务商、Redis/Docker或已消费的P34一次性授权。

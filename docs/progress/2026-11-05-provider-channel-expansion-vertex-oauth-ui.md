# 服务商扩展：Full Vertex 正常授权 UI 与模板

日期：2026-11-05。状态：本前端子批定向离线通过，不是整卡或总计划完成。对应 R11/W07，接续 [正常授权管理后端](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-05-provider-channel-expansion-vertex-oauth-management.md)。

## 范围与未完成边界

本批开放独立 `vertex-oauth` 空池模板，接入正常 Console 的专用 API client、显式 client/target、添加账号、稳定 ID 重授权、metadata Keep 与同管理身份下未知保存关闭重开。
复用三适配器、managed-auth、Secret Grant、SecretPatch、原 revision/CAS 与后端持久 owner，不新增秘密存储、刷新 scheduler 或并行配置保存链。

**浏览器使用明确标识的合成 fixture，管理响应和最终保存均模拟；没有连接真实 Gateway 管理 TCP/SQLite、Google OAuth 或模型服务。**
累计新增模板 **14→15**，不新增整卡 `offline_ready`，不标 R11/W07、P50、P60/P70 或总目标完成。
未读取 `.ng`、ADC、历史凭据或真实 OAuth client，不借 Antigravity client/token；未创建 Docker/Redis。P34 容器授权已消费并清理，未复用。

## 实现合同

### 1. 产品与模板

- [专用模板](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/apps/desktop/src/features/console/vertexOAuthCatalog.ts)使用 preset `google-agent-platform-official-api`、typed `auth_mode=vertex_user_oauth`，显式 Project/Location 生成只读 base。
  无内置 client、secret、模型、quota header 或首个凭据；沿正常空池创建，不自动探测、调用或标蓝。
- 新 UI 仅识别 typed auth mode 与四个已有 Vertex preset aliases，不按可修改 Provider ID 猜身份，不把旧 generic Vertex 配置自动转换。
  Express 与 Full Vertex 分池，额度关系继续 `unknown`。
- project/quota lexical 与后端一致：首字符 alnum，后续 alnum/`_.:-`，最多256 ASCII字符；location 为最多52字符的小写 DNS label。
  global、普通 regional、`us/eu` 专用 `.rep.googleapis.com` base 分别构造；不允许任意 URL。
- 新授权从已保存父级 base 取得 project/location，quota project 由本次显式输入捕获，不从父级缺省 context 推断。
  重授权从稳定 credential 的 `vertex_oauth.target` 读取完整 target，核实际 base，quota 固定，不暗改受众。

### 2. API 与正常账号入口

- [vertex-auth API](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/apps/desktop/src/api/console/vertex-auth.ts)复用 `createManagedAuthApi`，对应 `/v1/internal/gateway/console/vertex-auth-sessions`；target wire 保持 snake_case `quota_project`，外层 camelCase。
  session 校验额外的实际 loopback `redirectUri`，只接受 `http://127.0.0.1:<合法端口>/`。
- [VertexOAuthPanel](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/apps/desktop/src/features/console/VertexOAuthPanel.tsx)要求显式 Client ID、可选 password Client Secret，显示实际 redirect、固定 Google origin 的授权链接及手动 callback fallback。
  本批浏览器验证不点击真实授权链接或执行真实授权，不复制其他产品的 client；callback/secret 提交后从输入 state 清除，operation receipt 不保留这些私密输入。清除普通 JS 引用不等于物理内存 zeroization。
- Ready 不自动保存，必须点击“保存到凭证池”。绑定管理身份、原 revision、draft/lock 与 Secret Grant；换版本不会自动 rebase。
  新增与稳定 ID 重授权复用正常 GlobalDialogs；typed Full Vertex 不显示 generic API Key 或借用其他产品目录。
- metadata/手工模型编辑沿原 credential editor 和 nested SecretPatch Keep；拒绝 generic add、replace/clear、改受众、目录/会话/其他产品材料。
  手工模型复用已有严格单资源段/2048-byte策略，不推断可用模型或权益。

### 3. 未知保存与回收会话

- Console 为 Full Vertex 使用独立 operation Map，按 API client/management token 隔离，与 Accio/Grok/Kiro 不混用。
  accepted/unknown 保存关闭重开只 GET 同一 receipt，不第二次 create/import、不取消保存、不换 revision。
- 专属 policy 只精确识别 `GatewayApiError(code="vertex_login_rejected", message="vertex_login_unavailable")`。
  保存前的会话确定不可用时停止轮询、释放旧 session，允许用户明确重新授权；其他错误不被当作回收。
- 曾尝试保存的会话不可用时保留 uncertain receipt 与 duplicate-save fence，停止自动查询，允许手动继续 GET；不把回执消失推断为提交失败。
  通用 hook 的 create 同样受 `saveAttempted` 阻挡，不能绕过 UI 重建账号。
- 实际浏览器发现未知保存重开会显示新建空 quota 草稿，容易误认为原捕获 target 已变；新增红测后修正为活动/未知 receipt 隐藏创建草稿，不伪造无法恢复的私密输入。

## 运行证据与失败历史

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-vertex-oauth-ui-20261105/`。所有合成材料、一次性脚本、日志、截图及构建产物均在该目录。

1. `red.log`：5项行为红测失败，覆盖缺模板、Google Vertex 链接和不可用 session 处理；另发现 cleanup mock 未返回 Promise 的夹具错误，随后修正。`green-initial.log` 的5项通过不重复计数。
2. `typecheck-initial.log`：过宽 type predicate 令已为 Record 的 false 分支变成 `never`，产生 TS2339；改为 boolean 加边界 `isRecord`，后续类型检查通过。
3. `green-focused.log`：5文件59项通过；正常 GlobalDialogs 与 metadata SecretPatch 另有定向通过证据。
4. `affected-final.json`：76文件 **552 passed / 4 failed / 1 skipped**。四失败为 Vertex 新模板未提供目标、两项 Qwen 既有空地址夹具和一项 Canvas 命名假设。
   在两个通用测试夹具补显式 Vertex 目标、合成 Qwen workspace endpoint，并保留既有 `gemini-canvas` 精确身份；未放宽生产校验或删业务断言。`corrected-affected.json` 的4文件104项通过。
5. `red-receipt-display.json` 保留未知保存重开草稿显示的失败；`final-panel.json` 的20项通过，浏览器同步复核修正后的流程。
6. 调查时用 CommonJS `require.resolve('@neuro/ui')` 触发 `ERR_PACKAGE_PATH_NOT_EXPORTED`，该包只提供 import export；这是调查方式错误，不是构建失败，未因此改生产代码或依赖。

### 最终检查与去重口径

| 检查 | 实际结果 |
| --- | --- |
| 前端受影响组合 | 按绝对测试文件 + `fullName` 合并最后实际执行结果，**76文件、556 passed / 0 failed / 1 skipped**；包含7个新测试文件的 **74项新增测试**，不累计绿色重跑 |
| v2 最终定向复验 | 在外部依赖声明漂移分类后，仅重跑7个新增文件：**74 passed / 0 failed / 0 skipped**；`focused-v2-receipt.json` exit0 |
| TypeScript | `typecheck-closeout` 与 v2 `tsc --noEmit -p tsconfig.json` exit0；本轮没有 Rust 改动或 Cargo 重跑 |
| 隔离 Web 构建 | 使用实际 `src/main.tsx` 与合成 fixture；`build-v2-receipt.json` exit0，22个产物有 SHA256；不使用 live publisher，`cleanDistPath=false`，只写 linshi |
| 浏览器与构建对应 | `safe-web/final2` 实际浏览器产物与 v2 最终构建的22个文件逐个哈希相同；`build-browser-comparison.json` 为22/22相同 |
| 静态 | checker **31 passed / 0 failed**，ratchet exit0（4471文件、14 immutable runtime、governed source >700=0）；`static-receipt.json` 记录当时 `git diff --check` exit0 |
| 规模与源码 | 本批26个前端源码/测试文件有写前副本与 scoped diff，13个新文件；严格 UTF-8 无 BOM、独立 whitespace 无诊断，全部低于500有效行，最大481；不增例外、不改 baseline |

机器权威回执：`tests-deduplicated.json`、`focused-v2-receipt.json`、`typecheck-v2-receipt.json`、`build-v2-receipt.json`、`source-audit-v2.json`、`browser-proof.json` 与 `helper-cleanup.json`。
唯一 skipped 为需显式 `GATEWAY_WEB_MATERIAL_INPUT_PATH` 的私有 worker 样本测试，本批未提供或读取私有样本，不计通过；先前后端子批457个不同 Rust 测试是独立历史证据，不与本批前端数相加。最终文档差异/引用/编码检查另见 `documentation-closeout.json`。
前端未配置 formatter 命令，本批保持现有样式，没有安装格式器或伪报 formatter 执行。

### 外部依赖声明漂移的分类

首次冻结877个前端源码/config输入后，`package.json` 和随后 `package-lock.json` 出现本批未写入的变动：UI sync 追加 `--package @neuro/ui`，本地 UI 包迁到 `AwesomeDesignMD/packages/react/neuro-ui`。
当前 manifest 与 lock 的该包地址一致、目标存在；installed `@neuro/ui` 为普通目录（非 symlink），version `0.1.1`。未撤销、重装、升级或改写该外部工作。
原 `source-freeze.json`、漂移回执保持不变；v2 明确另建902项输入快照，包含原877项、已安装 UI package/dist 的22项与3个隔离 fixture/build/server脚本，最终核对无漂移。
v2 不覆盖整个 `node_modules`，不宣称 clean install 或全部依赖可重装已验；构建、74项新增测试和类型检查证明当前已安装依赖下的结果。锁文件重新安装及新本地包全量兼容未验证。

### 实际浏览器与资源清理

Edge 专用回环页仅使用合成材料，操作真正的模板/GlobalDialogs/CredentialDialog/VertexPanel/API schema，所有管理保存明确模拟。
正常选择模板、显式 project/location 创建空池、添加 OAuth 账号、显式 client/可选 secret/quota、手动 callback→Ready→明确 import；模拟 accepted 后响应丢失，关闭重开不第二次 create/import。
确认原 receipt 后对话框关闭且仅 refresh 一次：最终 `create=1, complete=1, import=1, cancel=0, refresh=1`，target 仍为 `project-a/global/quota-a`。
metadata 手工模型保存的 access/refresh/client-secret 三项 patch 全为 `keep`、`stagedSecrets=0`；重新授权显示稳定 `synthetic-account`，project/location/quota 均 readOnly、client 空、secret 为 password，无 API Key。
Escape 关闭后焦点回“编辑 OAuth 账号”。实际 CSS1365×900及390×780均无页面横溢出；旧窄屏6px溢出来自一次性 fixture 长 JSON，修 fixture 布局而非生产 dialog，旧证据保留。
最终截图为 `ui-wide-final.png`、`ui-narrow-final.png`、`ui-reauth-final.png`，不得冒充正式实例或真实授权成功。
专用 tab 已关闭，临时 viewport override 已 reset；核 Node PID45300 命令身份后只停止该 helper，52023 listener 与原 PID 均为0。没有删除文件、停止浏览器或现有服务。

## 下一接力点

以下为本UI子批当时停点；正常UI→真实TCP/SQLite/刷新/同进程runtime重建和Chat文本已由[后续本地子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-05-provider-channel-expansion-vertex-oauth-local-ui.md)推进，其余完整响应/三协议/分布式/真实验收边界仍保留。

1. Full Vertex 正常 UI→真实管理 TCP/SQLite→调用/刷新与同 owner runtime 重建；本批保存 mock 不冒充真实本地业务闭环。
2. 公共 Chat/Messages/Responses JSON/聚合 SSE、普通 Key/usage/结算、取消退款；独立完整 Full Vertex decoder、工具、签名及模态，不能借 Express 成绩替代。
3. Vertex-specific Server/Worker 真实 Redis publication/restart/竞争、PG/Hosted、跨进程/断电/failover receipt 恢复和旧数据/部署兼容；需要新容器时重新申请一次性授权。
4. 完整 feature matrix：此前仅官方线缺共享 Gemini Web 的15项缺符号未由本批修复或重跑，不报全矩阵绿。
5. Express 剩余 decoder、其他原定渠道、Luma Agents/Web/媒体及 P50 全范围无凭据集成/devtest继续保留。真实 OAuth client、IAM/project/region/model、refresh 和额度留 P60/P70 集中录制验收，不要求现在提供凭据。

未出包、部署、重启现有实例、迁移数据库、提交或推送；总目标继续执行中。

# P49 后续：Kiro CLI Key folder 产品隔离与材料转换

日期：2026-11-04 开始，2026-11-05 收尾。工作区：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`。
主配置与代表性 feature-off 定向离线验证已通过；不是总计划 P50 或 R02/W02 整卡完成。

## 1. 改了哪些内容

- folder family 和 surface 在共享 `kiro_compatible` executor/`kiro` profile、可改 label 的旧规则之前，
  按 runtime 产品识别为 `kiro-api-key`。服务仍为 `kiro-platform`，默认文件路径为
  `kiro-platform/kiro-api-key/api-key/<credential-id>.json`，不把它折叠为 `kiro-compatible/bearer-token`。
- 专用 normalizer 要求独立的 Key account、DirectHttp、相同 service/family/profile 和固定 US East runtime/mode/path；
  导入不能把 OAuth pool 转换成 Key pool。复用既有 Key validator、真实 DB merge 和 call-flow prepare，最终投影重新核验。
- `apiKey/api_key/KIRO_API_KEY` 等按既有字段归一规则折叠，同值可合并、冲突/错误类型拒绝；Key 必须是有界原子材料。
  只保留单份有效 Key、精确模型/账号 metadata 与已验证配置，不构造 OAuth 注册、machine、session、refresh 或目录。
  根 `rawSource` 仍只作不参与认证的归档并丢弃；嵌套混合材料、未知字段和不允许的 headers/body/target/flow 拒绝。
- 产品 guard 同时检查 root 和单层 `extraBody/extra_body`（沿既有大小写/分隔符归一规则），识别明确 `KIRO_API_KEY`
  以及 Key mode/runtime/surface/preset。非 Key surface 明确拒绝，不能绕到 generic passthrough 或旧 OAuth normalizer。
  不泛递归归档，也不通过 opaque `apiKey` 的 `ksk_` 形状猜测认证产品。
- export materializer 现在可返回错误；Key 材料进入相同严格转换后，才允许调用文件 writer。
  account 与 credential 跨产品 override 明确拒绝，不以重新贴 marker 的方式“修复”混合账号；无 Key claim 的既有 export 保持原语义。
  这是逐项写前边界，不承诺整个扫描事务回滚，也不改用户既有 source path、文件、数据库或未变 source hash。

生产 owner：
[分类](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/provider_credential_folder_sync/classification.rs)、
[专用转换与声明识别](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/provider_credential_folder_sync/normalization/kiro_key.rs)、
[分派](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/provider_credential_folder_sync/normalization.rs)、
[export metadata](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/provider_credential_folder_sync/payload_metadata.rs)、
[写前调用点](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/provider_credential_folder_sync/export.rs)。

## 2. 验证与过程保留

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-kiro-key-folder-20261104`。
Cargo 主配置为 `--locked --jobs 4 --no-default-features --features runtime-defaults`；测试栈16MiB，TEMP/TMP在本批目录。
不启用默认 management-ui 发布/清理 hook，不升级或清理依赖。

| 检查 | 实际结果 |
| --- | --- |
| 行为红测 | `red-folder.log`：0 passed / 5 failed，复现共享身份误归、错误材料类型、未建立静态身份与错误池接收 |
| 第一版 folder 全组 | `green-folder-v1.log`：169 passed / 14 ignored；该版本尚有后续只读审查发现的声明漏识别，不能当最终来源 |
| 修正后的 folder 全组 | `green-folder-v2.log`：171 passed / 14 ignored；包含13项新增显式测试 |
| Kiro 相邻回归 | `related-kiro.log`：162 passed / 0 ignored；与 folder 组重叠30项，主配置去重303 passed / 14 ignored |
| 代表性关闭配置 | `feature-off.log`：12 passed / 0 failed / 0 ignored，包含关闭 transport 时拒绝 import/export；完整列表见 `feature-off-list.txt` |
| 格式与有效行 | Rustfmt、checker31项通过；最终无报告写入的 ratchet 通过，不修改既有 dirty report/baseline/exception |
| 来源与最终 diff | 8项源码匹配 `source-green-input-v2.json` 且UTF-8无BOM；前批13项UI/材料与33项后端来源保持。最终diff/文档结果见 `closeout-exits.json` |

14项 ignored 是既有专用 PostgreSQL/Redis 导入删除/状态合同，未启用也不计通过；本批没有使用真实数据库。
主配置303项按完整名称去重，不叠加旧批250/124项、第一版绿测或关闭配置；关闭配置仅1个名称主配置没有。

独立只读审查确认，轻量 map 只有 account inline base，而真实 DB credential 可以覆盖 base/auth mode；因此本批选择
独立 Key account，并在 export 拒绝跨产品覆盖，而不是假设混合账号不可达。第一版绿测后还发现 explicit `KIRO_API_KEY`
及单层 extra 声明可被 generic/旧 OAuth 保留；v2补 bounded claims 和两项回归，审查按同一SHA确认该缺口已封闭。
这个新审查缺口没有另做单独的红测运行；原5项红测、v1绿测及v2最终来源分别保留，不混成同一输入。

首次 `npm run check:effective-lines --prefix scripts -- --json <absolute-linshi-path>` 因 checker 只允许仓库内相对报告路径
返回exit2；失败保留在 `ratchet.log` / `static-exits-v1.json`。改用同一官方 checker 的无报告写入入口后通过，
未为转绿放宽 policy 或覆盖工作区原有报告。8项源码最大423有效行，新文件最大226，符合新增/增量限制。

## 3. 边界与下一步

1. 本子批收尾时 map 只按 account 的 inline `baseUrl/base_url` 区分产品，object-storage account 的 Key 产品发现尚缺。
   后续已由[对象发现子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-05-provider-channel-expansion-p49-key-object-discovery.md)
   接入实际 import 的懒发现、原缓存/排序、错误与资源预算，并补 legacy alias；本回执的原测试数字不冒充该后续实现的证据。
2. 专用 folder 链只支持独立 Key account，不支持 OAuth account 下的 Key credential 或反向覆盖。
   合成测试覆盖实际 DB merge、metadata/path选择、导入/export纯转换和不借父级秘密；不是实际 PG upsert、cache同步或OS重启证据。
3. Key normalizer 复用已有启用状态校验；关闭 Kiro transport 时明确拒绝导入/export，不因解析材料启用网络，
   不将它冒称为与既有 OAuth pure parser 相同的关闭模式支持。
4. 普通前端与管理保存已由[Console子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-04-provider-channel-expansion-p49-key-console.md)承接；
   完整公共 Chat/Messages/Responses JSON/SSE pipeline、正常 UI→真实管理服务→保存→调用及重启仍须验证。
5. Kiro旧route/未变source hash DB自动升级、当前最小scope/profile/订阅/ARN/业务Region、跨应用receipt，以及
   Vertex OAuth/Express、Luma Agents、Grok余项、其他Web/媒体与Qwen OAuth受限待决继续保留。

累计新增模板仍13，不增加整卡offline_ready。总目标、P50/P60/P70继续；不要求现在录制凭据。
没有读取 `.ng`/历史凭据、登录、真实推理、创建Redis容器、出包、改在线实例、提交或推送；未删除linshi外内容。

# P49 后续：Kiro Key object-storage account 产品发现

日期：2026-11-05。工作区：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`。
状态：源码 v2 的定向离线验证与最终来源检查通过；不是总计划 P50 或 R02/W02 整卡完成。

## 1. 改了哪些内容

- 实际 folder import 在 source-hash skip、选中后的 hydrate/normalize 和 DB upsert 之前，调用同一个 scan-local account selection owner。
  第一次遇到 Kiro Key、OAuth 或 legacy family surface 时，先解析缺少 inline base 的稳定 Kiro 候选，再重建原索引、按原排序选择。
- 候选由 adapter/profile/family/service identity 识别，不用可改 label；没有相关材料时不预读全部对象，非 Kiro 路径不触发发现。
  同时支持 `baseUrl/base_url`。已有 inline base 的账号保持原轻量选择，不因此增加读取。
- 完整材料沿既有 `get_provider_account_for_folder_sync()` 和对象 owner 读取，复用原 128 项 / 64 MiB retained-estimate LRU cache。
  没有另建对象读取器、payload vector、数据库字段、迁移或全表 hydrate；缓存被淘汰或大于留存额度时仍按原 owner 语义处理。
- 单次产品发现共用 30 秒 async I/O / 检查预算，逐个读取而不是每个候选重新给 30 秒；新复制的 base 字符串累计最多 32 MiB。
  I/O 超时、错误、超限、无效 JSON 不吞成“没有 Key”，不回退旧 OAuth map；读完和重建 map 后拒绝已经超预算的结果。
  同步 JSON parse 不可抢占，这不是整个 scan 的硬墙钟截止；原单对象 32 MiB、S3 网络 deadline 与解析深度边界仍归对象 owner。
- 发现阶段与后续实际导入共享原 cache；小材料可以免于再次读取。只把公开 base 补到本次 metadata，不改账号或原材料。
  source-hash 命中仍不 normalize/upsert，旧未变 hash 的 DB 自动升级没有在本批偷偷完成。
- v1 交叉审查发现 `kiro-platform` 经原 canonicalization 得到 legacy `kiro`，可绕过仅 Key/compatible 的 trigger；v2 已补 trigger，
  两段路径和 payload surface override 均经真实路径解析覆盖。不是修改原 alias 规则或另建排序实现。

生产 owner：
[account selection](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/provider_credential_folder_sync/account_selection.rs)、
[实际 import 接线](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/provider_credential_folder_sync/import.rs)。
现有 classification、accounts 排序、cache、DB metadata/getter、对象读取和严格 Key normalizer 源码未改。

## 2. 验证与过程保留

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-kiro-key-object-discovery-20261105`。
Cargo：`--locked --jobs 4 --no-default-features --features runtime-defaults`；测试栈 16 MiB，TEMP/TMP 在本批目录。
未启默认 management-ui 发布/清理 hook，不升级、清理或重装依赖。

| 检查 | 实际结果 |
| --- | --- |
| 行为红测 | `red.log`：1 passed / 6 failed，复现无法发现 Key、旧 OAuth 误选 Key、失败被 inline winner 隐藏等原行为；不是编译失败 |
| 第一版 folder 全组 | `green-folder-v1.log`：183 passed / 14 ignored；仍缺独立审查发现的 legacy alias，不当最终来源 |
| 修正后的 folder 全组 | `green-folder-v2.log`：185 passed / 0 failed / 14 ignored，包含本批 14 项新增；两个 legacy alias 入口均通过 |
| 既有对象读取回归 | `object-storage.log`：29 passed / 0 failed / 0 ignored，包含真实 local/S3 mock JSON roundtrip、大小/流式边界与 stalled body deadline |
| 静态门禁 | Rustfmt、checker 31 项、无报告写入的 ratchet、diff 通过；6 项源码 UTF-8 无 BOM / SHA 核对通过；最终文档检查见 `closeout-exits.json` |

v2 新增共 14 项测试，覆盖正确 Key/旧 OAuth 选择、仅 Key 不进入旧 family、懒发现/稳定身份、inline 不读、原排序、snake base、cache 命中、
错误透传、共享 deadline/当前 future 取消、外部取消、晚 ready、base 留存预算，以及两个真实 legacy alias 入口。
红测原 7 项与 v2 完整 14 项不是同一输入：共享 deadline 的 DropProbe 已从“任意完成”收紧到“第二个当前读取被 drop”，后增资源/alias 用例。
14 项 ignored 是既有依赖专用 PostgreSQL/Redis 的合同，不计通过。注入 error code 是合成 loader 词汇，不宣称真实 parser 返回同名 code。
独立审查与 v2 所读 SHA 见证据目录 `review.md`，不将源码审查当运行验收。
最终主配置按完整名称去重为 214 passed / 14 ignored；不叠加红测、v1、旧批 303 项或重复运行。
既有 Key folder 8 项源码与前批冻结 SHA 全部匹配；本批 6 项源码最大 227 有效行，新 owner 为 146 行，import 从 161 降到 147 行。
ratchet 扫描 4,348 项、14 immutable runtime assets，governed source >700 为 0；不改 baseline/exception、不覆盖既有 dirty report。
本批未重跑未变化的 UI、旧 Kiro 大组或代表性关闭配置；不把已有关闭配置证据冒充本批新运行。

## 3. 边界与下一步

1. 本批证明的是实际 import 使用的发现/选择/cache owner、真实路径解析和 Key 材料转换的合成闭环；真实 PG getter→object storage→upsert/cache 同步没有执行。
   错误会停止当前 import，不执行后续缺失删除；不承诺回滚此前其他文件已完成的写入，也不新增整扫描事务。
2. 只支持独立 Key account；跨产品混合 credential override 仍按前批严格拒绝。未实现未变 source-hash DB 行或旧 route 的自动升级。
3. 下一步继续公共 Chat/Messages/Responses × JSON/SSE 完整 router/pipeline：实际 normalize、身份认证、route、精确凭据/model map、send、终态与 durable finalize。
   Responses generation controls 当前存在与 Key planner allowlist 的差异，必须明确支持或拒绝，不能为测通而静默删参数。
   随后接正常 UI→真实管理服务→保存→调用与重启；不重复已经完成的模板/暂存/普通 Key 编辑。
4. 当前最小 scope/profile/订阅/ARN/业务 Region、跨应用 receipt、Vertex OAuth/Express、Luma Agents、Grok 余项、其他 Web/媒体与 Qwen OAuth 受限待决继续保留。

累计新增模板仍 13，不增加整卡 offline_ready。总目标与 P50/P60/P70 继续；不要求现在录制凭据。
本批没有读取 `.ng`/历史凭据、登录、真实推理、创建 Redis 容器、出包、改在线实例、提交或推送；未删除 linshi 外内容。

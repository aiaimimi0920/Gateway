# Nous Portal：管理登录、明确保存与模型目录

继续 Session `01a1bc88-ad6b-74a2-b8da-b9dbe8d0c3b7` 最后未完的管理／目录／UI 切片。
原三图 42 项、红框 23 张卡及聚合子渠道全部保留。本记录不宣称全看板、Nous 全能力或真实账号验收完成。

## 实际实现

- `src/console/nous_oauth/` 分别拥有有界 session、轮询、registry、目标验证和明确保存。
  Device grant 要求操作者提供获准的 Client ID，不内置或借用 `hermes-cli`，不读取 Hermes profile。
- 管理入口为 `/v1/internal/gateway/console/nous-auth-sessions` 及其查询、`poll`、`cancel`、`import` 动作。
  管理身份和原 revision 绑定会话，写操作还需敏感访问确认；拒绝未知字段，body 上限 16 KiB，所有响应 `no-store`。
- 用户明确检查授权才发送 device poll。状态 GET 不换 token；interval／pending／slow_down 由既有协议负责。
  Registry 持有已接受的操作，HTTP 断开不取消已发送请求的有界响应接收。
- 收到 token 响应后先保留 raw receipt，再判断 JWT、scope、固定 inference target、取消、到期和配置漂移。
  未通过准入不恢复 challenge，也不重放未知消耗。取消禁止保存，但不丢弃在途已发送请求的接收结果。
- 用户明确 `import` 才经过普通 SecretPatch 和原 revision 的一次 CAS 保存。
  保存结果不确定或 CAS 冲突时不 rebase、不自动重试；重复 import 只返回原操作状态。
  重新授权保留账号 ID、操作员 metadata 和其他 secret；替换要求同 client、已知且相同的 subject hint。
  两个 unknown 不视为同身份；解码 subject hint 不冒充已验签身份。
- `/v1/internal/gateway/console/nous-model-catalog` 独立读取已保存账号的目录。
  目录失败不撤销账号保存；成功结果必须另行应用到表单，再由普通配置 CAS 保存。
  保留手动模型和 secret Keep，不把目录 ID 推断为免费价格、工具能力、权益或调用成功。
- UI 增加 Nous 独立模板、显式 device 登录／重新授权、目录读取／应用。
  模板不预填模型，不自动发付费测试；保存回执跨对话框关闭重开保留，revision 变化不改写已接受操作的目标。
- Nous 模型编辑及后端准入使用独立的 512 字节规则。长 `org/model` ID 可经过真实对话框提交及普通 CAS；
  CSV 无法无损表示的 ID 明确拒绝，不扩宽其他服务商规则。
- 浏览器授权链接及桌面外部打开白名单只接收 Nous 固定 Portal origin，禁止外来域、userinfo、fragment 和非 HTTPS。
  复用既有 launcher 的有界执行、取消及进程回收机制。

## 初始授权与持久刷新边界

初始 device grant 尚无已保存 credential／旧 refresh，不能伪造接入要求这些前提的 durable refresh reservation。
本次初始 receipt 是 **有界、进程内临时保管**，不是跨进程重启持久化：最多 16 个 session，
challenge 最长 600 秒，结果保留窗口另加 300 秒；正在接收或保存的任务完成前不被回收。
正常 shutdown 等待在途接收和保存；进程崩溃／强制退出后的未保存初始授权需要重新登录。
已保存账号的单次 refresh 继续使用已有 durable reservation／Received／apply owner，不另建平行刷新器。

## 验证证据与失败保留

日志根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-nous-management-20261109`。
验证使用合成 JWT、随机 loopback peer 与隔离临时目录，没有真实上游登录或付费调用。

| 证据 | 当前结果 |
| --- | --- |
| `cargo-management-r1.log` | 夹具编译失败：`enabled` 应为 `Option<bool>`、provider 无 `name` 字段；已修夹具，不放宽生产类型 |
| `cargo-management-r2.log` | 1 passed / 4 failed；失败命令整体仍为 red，不算成功 |
| `guard-capture.rs` | 最小 Rust 程序确认字段级 async capture 导致整个 Drop guard 提前释放；poll／save 已改为完整 guard 的方法调用 |
| `cargo-nous-r3.log` | 52 passed / 1 failed；登录、取消、回执保留、重新授权、CAS 冲突与 shutdown 通过，目录测试末尾误检查 DB credential 字段，命令整体仍为失败 |
| `cargo-catalog-r4.log` | 修正为真实 route-config 的 `payload.credential_id`，并检查长 upstream model 原样保留；定向重跑 1 passed / 0 failed |
| `model-selection-r3.log` | 复用 r3 实际测试二进制，公共模型选择回归 5 passed；r4 只改目录测试，不改变这部分生产源码 |
| `native-browser-r1.log` | 桌面授权 launcher 9 passed：Nous 固定 origin／单 argv、既有站点白名单、并发启动、超时和退出时子进程回收 |
| `ui-tests-r2.log` | Node 22 下 125 passed / 1 failed，并有 2 个未处理异常；实际提交路径 `props is not defined`，命令整体失败 |
| `ui-dialog-r3.log` | 修正 callback 解构后，5 个文件、32 passed；覆盖 Nous 真实提交、普通凭据与 DIM 对话框 |
| `ui-nous-panel-r3.log` | 使用已有 `nt-stack` 布局后，Nous 面板 9 passed |
| `typecheck-r3.log` | TypeScript 检查通过；保留此前 `typecheck-r2.log` 的 TS2304 失败 |
| `ui-build-r1.log` | 实际 Web entry 构建通过；临时配置只移除发布 hook、关闭清理并把输出放在 `linshi/web-r1`，不覆盖运行产物 |
| `checker-r1.log`、`ratchet-r4.log` | checker tests 31 passed；最终 ratchet 通过，5241 files、14 个 classified immutable runtime assets、governed source >700=0 |
| `rustfmt-r4.log`、`diff-check-r1.log` | 24 份 Rust 的官方格式检查与 `git diff --check` 通过 |

UI 与 checker 使用明确的 Node `v22.22.2`，不把较早 PATH 中 Node 20 的结果当最终依赖环境证明。
Rust 构建 jobs=2，未并行大型 Rust 构建，也未删除缓存、升级或重装依赖。
路由的 `provider_credential_id` 用于 DB 凭据；普通配置池使用 `payload.credential_id`。
上述最后一项只修测试断言，不更改账号路由生产语义；不把分批结果说成存在一次 53 全绿的命令。
本地管理／目录新增 8 项均已有通过证据；其余 Nous 协议、存储、刷新和公共推理沿用 r3 的 45 项通过结果。
UI 的 125／32／9 也是重叠的分批结果，不重复相加成独立测试总数。

浏览器视觉检查尝试被环境阻断：`The browser access host is unavailable`；
原生管道也返回 `系统找不到指定的文件。 (os error 2)`，浏览器及应用 inventory 为空。
已停止此次 loopback 视觉夹具 server；未取得截图、未完成浏览器或原生交互验收。

## 大小与审查

测量与源码哈希范围见 `scope-r4.json`，明确 `wholeTreeProvenance=false`，不冒充干净 HEAD 或整仓交付证明。
新增 Rust owner 单文件最大 235 effective lines；原 `providerCatalog.ts` 496 → 499，
`ConsoleGlobalDialogs.tsx` 407 → 430；`management_config.rs` 472 → 472，只有路由委派接线。
不修改 baseline 或 exceptions。保持独立 owner、固定目标、敏感信息脱敏、有界 session／任务和正常 shutdown join。
最终源码与上述测试快照未漂移，相关源码和文档均为 UTF-8 无 BOM；最终回执见 `final-validation.json`。

## 仍未验证或未完成

- 真实 Client ID 资格、账号登录／重新授权、订阅／Welcome 权益、真实旋转与费用。
- 浏览器窄／宽布局、键盘与原生桌面交互，实际 EXE／包、Server／Redis、跨应用验收。
- Nous Embeddings、legacy Completions 公共 wire、媒体、Welcome 公共推理等此前未验能力。
- feature compiled-out 构建分支，以及原全范围看板中的其他厂商／子渠道。

没有提交、推送、部署、重启运行实例、修改真实 `.ng`／数据库，也没有生成手测或正式发布包。

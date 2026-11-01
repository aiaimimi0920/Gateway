# Google Flow：严格 HTTP-only 试接入验收

日期：2026-11-01。用户选择：维持严格 HTTP-only；不能生成则如实标记。

## 结论

本轮完成独立 `flow-web` 模板、专用账号导入、原生 HTTP 只读控制面及本地失败合同。
**未完成 Flow 图片／视频生成，不能称为生成服务商已可用，也没有标蓝。**

最新参考路线要求每次生成携带 fresh reCAPTCHA proof。现有参考中的浏览器取 proof、
第三方服务不符合本轮边界；参考中的 HTTP fallback 没有当前真实生成端接受证据。
本轮不将“生成 POST 使用 HTTP”误称为“整条调用无需浏览器”，也没有证明所有 HTTP-only 路线都不可行。

## 改动范围

- 独立 preset、adapter、auth family、官方 base URL；不与 FlowMusic 或 Gemini Canvas 混用。
- UI 支持网页登录导入和明确 schema 的 Cookie JSON 导入，继续使用既有敏感访问授权、revision/CAS 和私有对象存储。
- 网页登录阶段无调试／自动化连接；用户关闭专用窗口后短暂重开以导入必要 Cookie，随后关闭所拥有的浏览器。
- 原生 HTTP 只读验证限定 `flow.google.com`，禁止重定向，保留 Cookie 域／路径／到期语义，限制时间与响应大小。
- 网页导入保存本次只读验证后的 Cookie 快照，不保存验证前已轮换的旧材料。
- 模板和导入草稿不添加未经验收的模型，不自动创建生成路由；连通探测不等于生成成功。
- 图片／视频请求进入执行器后，在凭据读取、网络和进程启动前返回 `flow_web_fresh_proof_unavailable`。
  HTTP 503、不可自动重试、禁止 fallback；不返回占位成品。

未实现生成 wire encoder、上传、视频轮询和成品回收。解锁这些功能前，必须先完成真正的无浏览器 proof 获取和真实生成验收。

## 验证状态

### 通过

- 默认 features 的 Rust Flow 定向测试：9 passed、1 ignored（真实人工登录只读测试），共两个 suite。
  命令：`cargo test --locked -j 1 --lib flow_web --test flow_web_contract -- --nocapture`。
- 共享 Google Web JSON 单元测试：7/7。
- 管理 API JSON 导入合同：4/4，覆盖四个 Google Web family 的授权、目标绑定、离线导入及配置不变性。
- 前端相关八个文件共 54 个用例分组通过；其中目录对话框一次高资源压力下超时，单独单 worker 重跑 10/10 通过，未调大超时。
- 桌面 TypeScript typecheck 通过。
- Flow 导入 worker 合同：2/2。
- effective-line checker 自测：31/31；repository ratchet、Rust formatter 和 `git diff --check` 通过。
- 本轮新增 Flow 源文件均不超过 250 有效行；已测源码 UTF-8 无 BOM。

### 未通过的非目标配置

尝试 `--no-default-features` 时，已有 Gemini disabled 模块缺少 speech/media exports，编译失败。
未扩大范围修复；最终默认 features 的上述 Rust 定向测试已实际通过。不能将无默认 features 构建称为通过。

### 未执行／受阻

- 未执行真实 Google Flow 登录和只读额度请求，正常账号导入全链尚未真实验收。
- 图片／视频真实生成受 HTTP-only fresh proof 缺口阻塞；没有生成结果，也没有消费额度。
- 未编译或交付新 EXE，未修改运行实例、现有凭据或数据库，未提交、推送。
- 保留本轮开始前已有的其他服务商与 request-recovery 工作区修改。

## 证据与说明

本轮临时证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-flow-web-20261101`。
`before-lines.json` 记录最初测量值，`after-lines.json` 为终检测量；`before=null` 表示没有起始测量值，不表示该文件均为本轮新增。

协议边界、Cookie JSON 格式及已联网核对的参考 commit 见 [Flow Web 说明](../flow-web.md)。

# Google Web 服务商的网页会话 JSON 导入

`gemini-web`、`gemini-canvas`、`aistudio-web`、`flow-web`、`stitch-web` 是独立服务商模板，各自接受绑定该类型的
JSON。`chatgpt-web` 保留原有格式；不同服务商的文件不能直接混用。本入口不是通用
Playwright `storageState`、浏览器配置目录或 OAuth 文件导入器。

## 使用方式

1. 在“添加账号”选择已保存的目标服务商，可填写账号名称。
2. 将“录入方式”改为“网页会话 JSON”，完成敏感信息访问授权。
3. 粘贴下列对应格式，或选择 UTF-8 JSON 文件；选择文件不会自动提交。
4. 点击“导入 JSON”。材料在本地解析，账号沿现有草稿、secret patches 和 revision/CAS
   保存流程合并，不替换池中已有账号，不自动测试账号。

输入在提交、关闭、切换服务商／方式或授权撤销时清空，不存入 localStorage，也不写入
普通路由 JSON。文件选择通过页面的 File API 读取，不接受路径字段或远程下载地址。
请只使用自己的必要会话材料，不要把真实 Cookie 发到聊天、日志、截图或仓库。
若没有符合格式的材料，继续使用原有“网页登录”即可，无需导出完整浏览器状态。

导入不启动浏览器、不访问 Google，也不验证账号、套餐、模型或 Cookie 是否被服务器撤销。
“已解析并准备账号草稿”不等于真实调用成功；过期或被撤销时仍需重新认证。JSON 路径本身
不依赖 Windows 浏览器；既有两阶段网页登录入口仍受其本地 Windows 宿主限制。

## Gemini Web

```json
{
  "schemaVersion": 1,
  "provider": "gemini-web",
  "authUser": "0",
  "cookies": [
    {
      "name": "__Secure-1PSID",
      "value": "REPLACE_WITH_YOUR_OWN_VALUE",
      "domain": ".google.com",
      "path": "/",
      "expires": -1,
      "secure": true,
      "httpOnly": true,
      "sameSite": "Lax"
    }
  ]
}
```

必须有有效的 `__Secure-1PSID`；可以用同样格式补充 `__Secure-1PSIDTS` 及必要 Cookie。
此 JSON 路径当前只接受 `authUser: "0"`：既有 Gemini Web HTTP owner 的 bootstrap 使用
默认账号路径，尚不应用该索引；非零索引明确拒绝，避免静默调用另一账号。Canvas／
AI Studio 的现有 HTTP owner 支持数字索引，不受该专属限制影响。
只接受 `google.com`、`gemini.google.com` 的 host/domain scope，所有 path 必须为 `/`，
合成 Cookie header 时同名歧义会拒绝，不能悄悄扩大 Cookie 的路径作用域。

可选 `session` 对象只接受 `modelHeader`、`modelHeader2`、`modelHeader3`、
`requestContextHeader`、`language`，值须为可打印 ASCII 字符串、每项不超过 4096 字节。
不导入 `accessToken`、`sessionId` 缓存，由正常 HTTP bootstrap 获取新值。
原始 Cookie 只进入已有授权 secret patches；普通配置保留非敏感元数据。

## Gemini Canvas

```json
{
  "schemaVersion": 1,
  "provider": "gemini-canvas",
  "authUser": "0",
  "cookies": [
    {
      "name": "SAPISID",
      "value": "REPLACE_WITH_YOUR_OWN_VALUE",
      "domain": ".google.com",
      "path": "/",
      "expires": -1,
      "secure": true
    }
  ]
}
```

至少包含一个有效的 `SAPISID`、`__Secure-1PAPISID` 或 `__Secure-3PAPISID`；可以补齐
同账号的其他必要 Cookie。域白名单与 Gemini Web 相同，必要认证 Cookie 必须能匹配
`https://gemini.google.com/`。仅保存最小 Cookie 对象和账号索引，不保存浏览器 profile、
运行时引用或网页 origins。调用继续走已有 HTTP-only Canvas owner。
实验性 `gemini-canvas-chat`／Business 不在本次 JSON 格式范围，保留既有入口。

## AI Studio Web

```json
{
  "schemaVersion": 1,
  "provider": "aistudio-web",
  "authUser": "0",
  "cookies": [
    {
      "name": "SAPISID",
      "value": "REPLACE_WITH_YOUR_OWN_SAPISID",
      "domain": ".google.com",
      "path": "/",
      "expires": -1,
      "secure": true
    },
    {
      "name": "__Secure-1PAPISID",
      "value": "REPLACE_WITH_YOUR_OWN_1PAPISID",
      "domain": ".google.com",
      "path": "/",
      "expires": -1,
      "secure": true
    },
    {
      "name": "__Secure-3PAPISID",
      "value": "REPLACE_WITH_YOUR_OWN_3PAPISID",
      "domain": ".google.com",
      "path": "/",
      "expires": -1,
      "secure": true
    }
  ]
}
```

三个签名 Cookie 全部必需且须匹配 `https://aistudio.google.com/`。允许的域为
`google.com`、`aistudio.google.com`、`alkalimakersuite-pa.clients6.google.com`，保留
原有 host/domain、path、expires、secure 约束。保存为已有 schemaVersion 1 的
`aistudio-web` 最小 Cookie bundle，沿现有纯 HTTP worker／进程内 proof 引擎调用，
不引入浏览器回退。是否实际可用仍取决于账号状态和必要的完整 Cookie 材料。

## Stitch Web

```json
{
  "schemaVersion": 1,
  "provider": "stitch-web",
  "authUser": "0",
  "cookies": [{
    "name": "OSID",
    "value": "REPLACE_WITH_YOUR_OWN_VALUE",
    "domain": "stitch.withgoogle.com",
    "path": "/",
    "expires": -1,
    "secure": true,
    "httpOnly": true
  }]
}
```

这是格式示例，不保证仅一个 Cookie 足够；请保留同账号必要本站 Cookie 的真实属性。
只接受 `.withgoogle.com`、`stitch.withgoogle.com`、`.stitch.withgoogle.com`，
不接受或改写 `.google.com` Cookie。必须有能匹配本站根地址的有效 Cookie。
目前只接受 `authUser: "0"`，不接受 `session` 元数据。
凭据沿独立 Stitch 原生 HTTP owner 使用；`stitch-design` 是界面设计操作别名，
不是动态获取的 Google 模型标识。详见 `docs/stitch-web.md`。

## 校验、保存和模型边界

- 原始 JSON 最多 64 KiB；Cookie 数量为 1–64，序列化材料预算不超过约 32 KiB。
  每个 value 最多 8192 字节；拒绝 Cookie／header 控制字符和分隔符注入。
- `authUser` 是 1–3 位数字字符串（Gemini Web、Flow、Stitch 限 `"0"`）。`expires` 为 Unix 秒数，`-1` 表示会话 Cookie；
  不应为绕过校验而将真实已过期 Cookie 改为 `-1`。过期可选项丢弃，必要认证项失效则拒绝。
- Cookie 的必填字段见示例；`httpOnly` 和 `sameSite` 可选，后者限 `Strict`、`Lax`、
  `None`。拒绝重复 Cookie 身份、未知字段、partitionKey、完整 storageState、origins、
  localStorage、refreshToken、任意对象路径、URL、可执行文件和任意请求头。
- 管理 API 要求 management token、独立 `x-secret-grant`、当前配置 revision、匹配的
  preset 和对应 Google 官方 base URL。迟到响应在授权、修订或目标改变后不得应用。
- Canvas／AI Studio／Flow／Stitch 的 Cookie 由正常 Gateway object-storage owner 保存；管理响应及
  路由配置只含对象引用。Gemini Web 的敏感编辑仅通过已授权的瞬态响应交给 secret owner。
  所有成功响应为 `Cache-Control: no-store`，JSON 结果不进入可轮询的浏览器会话 manager。
- 默认模型只写 `model_selection.manual`，`fetched` 为空，不制造目录发现、
  `verified_models` 或真实 Google 验收证据，不因 JSON 导入扩大蓝色模板登记范围。

管理入口复用 `POST /v1/internal/gateway/console/gemini-auth-sessions`，新增可选
`webSessionJson` 字符串和 `expectedRevision`。不提供 JSON 时保留原网页登录流程；
提供 JSON 时只执行离线解析与草稿生成，随后走已有路由保存 API。
该管理请求信封另有限制 512 KiB，给嵌套 JSON 字符串的转义预留空间，不能据此放宽
64 KiB 的原始材料上限。附带 `expectedRevision` 却缺少 JSON 时拒绝，不回退打开浏览器。

# ChatGPT Web 独立会话导入：实现与持久化验收

## 已实现范围

本轮实现的是“已有完整 Web 材料文件 → 独立 UI 导入 → 正常配置事务保存 → 重启读取”，不是浏览器自动采集，也不是自动续期。

- `chatgpt-web-reverse` preset 的账号窗口使用“网页会话 JSON”文件入口；不渲染 OAuth 面板，不读取 Codex 模型目录。
- Codex OAuth 的授权、交换和持久化实现未修改；原有其他服务商 API Key 录入保留。
- 文件内容仅保留在临时表单状态，取消、切换服务商、权限撤回后丢弃；异步读取有 generation fence。读取新文件先清空旧材料，失败时不会误存上一份。
- 通过原有账号草稿与自动保存 owner 提交；使用已有 secret grant、secret patches 和 revision 校验，不直接写配置文件。
- token、Cookie、可选 sessionId 均通过 secret patch；普通 JSON 不包含这些秘密值。Cookie 名称大小写替换时移除旧 keep patch，避免保留两个不同值。

## 接收格式与边界

根对象接收 worker 输出或等价 JSON：

- access token：`authToken`、`accessToken`、`access_token`。
- Cookie：`cookieHeader` 或 `headers.Cookie` / `headers.cookie`。
- User-Agent：`userAgent` 或 `headers["User-Agent"]`。
- deviceId 必填；sessionId 可选，缺失时显式清除目标账号旧 sessionId。
- token 到期时间：优先不晚于 JWT exp，可显式提供 `accessTokenExpiresAt` / `expiresAt`；不使用网页 session 的 `expires`。保存为 UTC Z 格式。
- 白名单可选字段：clientVersion、clientBuildNumber、language、languageCode、timezone、chatgptPowSources、chatgptPowDataBuild。null 可选值忽略；不透传 rawSource、调试输出、任意 extraBody 或浏览器存储。
- 文件上限 128 KiB，并限制各字段长度与控制字符。错误不回显输入值。含 refreshToken、refresh_token 或 oauthTokens 的文件拒绝，不能用这个入口导入 OAuth 刷新凭据。

本轮没有改变文件夹同步 normalizer；该路径仍不能等同于这个控制台入口。没有补本地请求时 browser worker 刷新结果的自动写回；本轮证明的是显式导入保存。

## 真实发现与修正

1. 第一次真实管理接口提交返回 422：extra_body/sessionId 被秘密分类器识别。改为专用 secret patch；该字段不能在普通 JSON 中以 null 占位，必须省略。
2. 已有真实 worker 样本的 sessionId 为 null。改为可选，不为了导入伪造一个 session ID；已有样本重新解析通过。
3. 保存后重启验收使用合成数据，未执行上游调用，避免把凭据持久化检查混同于生成验收。

## 验证证据

- 新解析器先因实现缺失失败，再运行到通过。
- 定向 Vitest 覆盖解析、秘密分离、Cookie大小写、独立 UI、权限撤回、异步过期、错误文件替换、真实 editor hook 提交和原有账号编辑路径。
- 最终定向回归 9 个文件：56 passed、1 skipped；跳过的是默认不读取私有材料的 opt-in case，该 case 已在显式提供本机样本时单独通过。TypeScript typecheck、web 前端构建、checker 31 项测试、effective-line ratchet、diff check 均通过。
- 修改前/后有效行：CredentialDialog 372→388、credentialDocument 448→453、useCredentialDialogEditor 315→320、ConsoleGlobalDialogs 258→259；新实现文件分别 89、44 行，无需超限例外。
- 显式读取前轮 `headless-material.private.json` 做解析兼容性检查通过，未打印或复制材料到仓库，未执行联网请求。
- 前端 hook 产出的合成 document/secretPatches 直接交给真实 4300 管理接口；正常提交、停止该测试子进程、同数据重启后 revision/metadata/秘密描述符仍保持，凭据通过 secret grant 读取核对一致。Cookie 在本地持久化，普通 API 视图不泄漏，sessionId clear 也通过。
- 后端复用此前已构建的 web devtest EXE，SHA256 `a9b1386c66a9d2eaa78c783c4b265421469f2418e1c107d2d3d6a9eba1cb31dc`；本轮未改 Rust，不能将它说成包含新 UI 的新 EXE。
- 证据：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-chatgpt-web-20261028/web-import-restart-result.json`；验证脚本同目录 `verify-web-import-restart.py`，只使用合成凭据。

4200 的原进程保持运行，测试结束关闭本轮 4300 子进程。不编译桌面整包、不标记自动续期成功、不批量测试模型。

## 后续独立阶段

在此导入合同上增加“从指定已登录浏览器准备 Web 材料”的显式动作，让用户无需手工导出 JSON。再补浏览器刷新后的本地事务写回、失败与并发保护。两者都不得接入或借用 Codex OAuth 的回调来假装获得网页登录态。

# Gemini Web 图片单次提交修复（r10）

## 原因与改动范围

普通 Web 的 modular adapter 在一次图片结果不确定后，可能依次进入 HTTP 模板、另一发送器、
JSON 和浏览器生成；浏览器 worker 与页面轮询也会再次提交。网络失败、无图片或下载失败都不能
证明上游没有生成，因此这些恢复方式可能重复消耗额度。

本轮只收紧 `gemini_web_reverse_modular_compatible` 的图片生成/编辑，以及它使用的浏览器
媒体发送边界；不把默认 `gemini_web_compatible` 宣称为已经支持图片输出。旧 Canvas 与 program
adapter 的 legacy HTTP 恢复路径没有在本轮整体改写。

改动：

- Web 图片 HTTP 使用单次 StreamGenerate。已采集模板由 Rust 发一次，禁止重定向重发；无模板
  时保留 bootstrap、模式准备和上传，再用 heavy builder 发一次，不在错误后重试 XSRF。
- 不进入旧的 JSON 再生成、模板再发送和携带原 prompt 的 follow-up；结果只从本次响应解析。
  没有最终资产就明确失败。已有资产的读取/下载可以保留，但下载失败不重新生成。
- HTTP 生成出错不再切到浏览器生成；浏览器执行是发送前的配置选择，而不是事后重放。
- 浏览器图片 worker 的 `/invoke` 不再自动重试；页面发送开启 singleSubmit，轮询不再重新提交。
- 已派发到远端的 Gemini 图片/音乐/视频不能在远端故障、重定向、坏 JSON、缺失结果后切本地。
  普通文字与其他 provider 的既有 LocalAllowed 行为保持不变。
- 图片最终错误设置不可重试和 Abort，覆盖解析/下载错误与跨账号 fallback；错误投影不包含
  私有上游正文。外层 provider retry 同时登记图片端点的单次约束。
- mixed-lane 路由保留显式 `pureHttpMode` / `pure_http_mode`，不再吞掉 required/disabled。
  Required 图片不在下载失败后启动浏览器；当前仅有浏览器实现的音乐/视频遇到 Required 时在
  发送前拒绝，图片编辑遇到 Disabled 时同样明确拒绝，不暗中违反配置。

## 验证记录

- 新图片 singleSubmit 断言先红：旧参数为 false；修复后通过。
- 合成 loopback worker 先红：一次图片失败收到 3 次 `/invoke`，预期为 1。
- 远端派发 loopback 先红：旧实现返回 `Ok(None)`，会让调用者继续走本地 worker。
- Node 12 个相邻测试文件共 158 项通过，包含无图片超时、已有图片不重发、单击失败和资源清理。
- Rust 最终定向测试 34 项通过，包含 captured/heavy 图片与编辑发送、远端 307/308 不重发、
  缺结果不转本地、显式模式和旧文字 fallback 保护；Web 相邻回归 42 项通过。两组存在重叠，
  不相加成去重后的测试总数。测试夹具最初使用了错误的协议枚举字符串，已改用类型序列化后
  重新编译通过，不把这次夹具失败归因到上游。
- effective-line checker 31 项通过，ratchet 和嵌套 worker 包合同通过。
- 公共 ingress 合同 7 项通过；fmt、diff、MJS syntax 与 UTF-8 无 BOM 检查通过。
- 最终包与空账号启动的结果以本轮 `linshi/.../r10/` 回执为准；构建前不得
  将源码检查或旧包的运行结果当成新 EXE 的验收。

## 明确的能力边界

这里保证的是单个 Gateway 调用中不进行应用层自动再生成，不是分布式 exactly-once。
调用端主动重新发送、跨重启 checkpoint、多个 worker 的 CAS 都不在该保证内。

为避免不确定重放，本轮不复用旧的 prompt 型异步 follow-up。若上游只返回进度/locator 而没有
最终图片，会失败而不会偷偷再生成；后续需要补的是绑定已接受任务标识的只读结果恢复，而不是
恢复旧的多发送器重试。普通文字和 r9 的媒体理解/转写仍走各自原有 HTTP 路径。

本轮未执行真实账号生成，不导出账号状态、不重试此前被策略拒绝的真实账号 worker 组合，
也不重放已消费的生成 guard。Canvas 音乐/视频可信宿主合同、普通 Web 多模态公共 API 的真实
账号成功验收、跨重启 checkpoint、多 worker CAS 与 S3 writeback 仍未完成。

现有 Gateway、凭据、数据库和旧手测包保持不动；没有提交或推送代码。

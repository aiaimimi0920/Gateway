# L2 Web worker：正确识别混合换行的 SSE 完成事件（2026-11-21）

在已有 L1／L4／L6 证据之后推进 L2。旧 Web worker 只查找成对的 LF、CRLF 或 CR；`LF + CRLF` 和 `CRLF + CR` 这样的合法 SSE 空行边界可能被漏掉。修复前，由同一动作返回的合成产物已经完成，但序列化页面夹具返回 `lumalabs_missing_signed_url`，本机实际 Fetch 保持订阅直到 `lumalabs_browser_timeout`。

按 [HTML Standard 的 SSE 解析规则](https://html.spec.whatwg.org/multipage/server-sent-events.html#interpreting-an-event-stream)，[共用浏览器操作](../../scripts/lumalabs-session/browser-operation.mjs)将流里的三种换行统一为 LF，并保留跨 chunk 的 CRLF 状态。单个拆开的 CRLF 不算空行；只有收到空行才消费帧，EOF 仍不接受未终止尾帧。扫描保留上次检查位置，不重扫旧尾部。仍须匹配原动作唯一 output ID 与 completed 状态；字节上限、UTF-8 校验、截止时间、取消／释放和单次 POST 规则不变。

## 验证

- 修改前：两个受影响测试失败（序列化夹具丢结果、实际 loopback Fetch 超时），另 10 个通过。
- 修改后：`scripts/tests/lumalabs-*.test.mjs` 的 40 项通过。覆盖混合与跨 chunk 换行、未终止尾帧拒绝、真实 loopback Fetch 关闭、单次 POST、字节上限及既有 executor／worker 生命周期。供应商数据均由夹具生成。
- 修改文件与直接调用方的 Node 语法检查通过；有效行检查器 31 项及 ratchet 通过。生产操作 368 → 377、序列化合同测试 62 → 82、loopback Fetch 测试 47 → 49 有效行。

## 尚缺验收

本机浏览器桥接未能取得已登录的 Luma 页面，不据此判断用户的登录状态。此次只修复现有 worker 的可复现问题：未取得当前官网会话／board／model-action／任务或媒体协议，未发真实生成或扣费请求，未重打包、部署或修改在线实例。接续只需授权的已存在页面记录或脱敏样本，不需要导出原始 HAR、Cookie、Authorization 或签名 URL。

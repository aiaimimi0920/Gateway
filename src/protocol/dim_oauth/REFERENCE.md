# DIM 协议参考

这是 Gateway 的独立 Rust 实现，不加载或执行 magpie 插件。服务商登记、管理授权会话、
旋转刷新持久化和逐模型发送尚在实现中；此模块本身不能作为完整服务商已接入的证据。

协议和模型目录映射参考：
[arcships/magpie-dim-oauth](https://github.com/arcships/magpie-dim-oauth/tree/a7ba461071fb100985cd9ae36edd15be47965ee9)，
固定 SHA `a7ba461071fb100985cd9ae36edd15be47965ee9`，`src/oauth.ts`、`src/catalog.ts`、`src/http.ts`。
保留其许可如下。Gateway 使用自有 User-Agent/X-Title，不内置该插件或官方客户端的 client ID。

2026-11-08 匿名联网核对两站 `/.well-known/openid-configuration`：authorization/token/userinfo/JWKS
均与所选 issuer 同源，PKCE S256、public client `none`、ID token RS256。
这不是注册资格、登录、订阅权益或模型推理验收。用户明确暂时没有 Gateway 客户端注册信息，先完成本地实现。

## 上游 MIT 许可

MIT License

Copyright (c) 2026 Arcships

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

//! Gateway 默认 HTTP 构建入口；代理选择与客户端/TLS 配置分离。
mod proxy_policy;

/// 保持旧环境/Windows 代理合同。显式代理调用者应先 `.no_proxy()` 再添加自己的代理。
pub fn builder() -> rquest::ClientBuilder {
    proxy_policy::configure(rquest::Client::builder())
}

/// 与 Client::new 相同的构造失败策略，不改变调用者的 timeout/TLS 配置。
pub fn client() -> rquest::Client {
    builder()
        .build()
        .expect("failed to build default HTTP client")
}

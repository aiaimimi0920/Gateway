//! Ordinary Local Keys over real TCP: ordered tools/history, settlement and caller disconnect.
#![cfg(feature = "family-bedrock-converse-official-api")]
#[path = "bedrock_public_tcp_contract/cancellation.rs"]
mod cancellation;
#[path = "bedrock_reasoning_response_contract/fixtures.rs"]
#[allow(dead_code)]
mod fixture;
#[path = "bedrock_native_outbound_contract/fixture.rs"]
#[allow(dead_code)]
mod native_wire;
#[path = "bedrock_public_tcp_contract/outbound.rs"]
mod outbound;
#[path = "bedrock_public_tcp_contract/requests.rs"]
mod requests;
#[path = "bedrock_public_tcp_contract/runtime.rs"]
mod runtime;
#[path = "bedrock_public_tcp_contract/success.rs"]
mod success;
#[path = "console_contract_support/mod.rs"]
mod support;
#[path = "bedrock_public_tcp_contract/upstream.rs"]
mod upstream;
#[path = "bedrock_upstream_eventstream_contract/fixtures.rs"]
#[allow(dead_code)]
mod wire;

fn run<F: std::future::Future<Output = ()>>(build: impl FnOnce() -> F + Send + 'static) {
    // Only this integration harness gets the established explicit stack budget.
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    tokio::time::timeout(std::time::Duration::from_secs(120), build())
                        .await
                        .unwrap()
                });
        })
        .unwrap()
        .join()
        .unwrap();
}

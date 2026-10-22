//! Native build stays package-scoped; other Gateway TLS backends are unchanged.
fn main() {
    gateway_cpa_bssl_build::build();
}

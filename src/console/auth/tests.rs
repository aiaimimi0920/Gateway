use std::fs;
use std::net::IpAddr;
use std::path::PathBuf;

use uuid::Uuid;

use super::{AuthenticatedConsoleActor, ConsoleAuthRuntime, ConsoleRequestContext};
use crate::console::{ConsoleConfig, ConsoleConfigValues};
use crate::error::GatewayError;

struct TestConsoleFixture {
    root: PathBuf,
    config: ConsoleConfig,
}

impl TestConsoleFixture {
    fn new(remote_access_enabled: bool) -> Self {
        Self::with_secret_grant_ttl(remote_access_enabled, None)
    }

    fn with_secret_grant_ttl(
        remote_access_enabled: bool,
        secret_grant_ttl_secs: Option<u64>,
    ) -> Self {
        let root = std::env::temp_dir().join(format!("gateway-console-auth-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let routes_file = root.join("routes.yaml");
        fs::write(&routes_file, b"providers: []\n").unwrap();
        let config = ConsoleConfig::from_values(ConsoleConfigValues {
            state_dir: Some(root.join("state")),
            routes_file: Some(routes_file),
            remote_access_enabled: Some(remote_access_enabled),
            secret_grant_ttl_secs,
            ..ConsoleConfigValues::default()
        })
        .unwrap();
        Self { root, config }
    }
}

impl Drop for TestConsoleFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn remote_access_enabled_allows_non_loopback_management_auth() {
    let fixture = TestConsoleFixture::new(true);
    let runtime = ConsoleAuthRuntime::new(&fixture.config, Some("123456".to_string())).unwrap();
    let request = ConsoleRequestContext::new(
        IpAddr::from([192, 0, 2, 10]),
        "http://127.0.0.1:4210".to_string(),
    );

    let actor = runtime
        .authenticate_management_token(&request, "123456")
        .expect("remote access flag should allow configured non-loopback console auth");

    assert!(!actor.secret_access_granted());
}

#[test]
fn loopback_only_default_still_rejects_non_loopback_management_auth() {
    let fixture = TestConsoleFixture::new(false);
    let runtime = ConsoleAuthRuntime::new(&fixture.config, Some("123456".to_string())).unwrap();
    let request = ConsoleRequestContext::new(
        IpAddr::from([192, 0, 2, 10]),
        "http://127.0.0.1:4210".to_string(),
    );

    let error = runtime
        .authenticate_management_token(&request, "123456")
        .expect_err("loopback-only default must reject non-loopback clients");

    assert_eq!(
        error.code.as_deref(),
        Some("console_remote_access_forbidden")
    );
}

#[test]
fn internal_management_verification_accepts_environment_token() {
    let fixture = TestConsoleFixture::new(false);
    let runtime =
        ConsoleAuthRuntime::new(&fixture.config, Some("environment-token".to_string())).unwrap();

    runtime
        .verify_management_token("environment-token")
        .expect("environment management token must authorize internal selectors");
    let error = runtime
        .verify_management_token("wrong-token")
        .expect_err("wrong environment management token must fail closed");
    assert_eq!(
        error.code.as_deref(),
        Some("console_management_token_invalid")
    );
}

#[test]
fn internal_management_verification_accepts_bootstrapped_token() {
    let fixture = TestConsoleFixture::new(false);
    let runtime = ConsoleAuthRuntime::new(&fixture.config, None).unwrap();
    runtime
        .bootstrap(&ConsoleRequestContext::loopback(), "bootstrap-token")
        .expect("bootstrap management token");

    runtime
        .verify_management_token("bootstrap-token")
        .expect("bootstrapped management token must authorize internal selectors");
    let error = runtime
        .verify_management_token("wrong-token")
        .expect_err("wrong bootstrapped management token must fail closed");
    assert_eq!(
        error.code.as_deref(),
        Some("console_management_token_invalid")
    );
}

#[test]
fn secret_grant_is_bound_to_exact_actor_origin_and_client_ip() {
    let fixture = TestConsoleFixture::new(true);
    let runtime = ConsoleAuthRuntime::new(&fixture.config, Some("123456".to_string())).unwrap();
    let request = ConsoleRequestContext::new(
        IpAddr::from([127, 0, 0, 1]),
        "http://127.0.0.1:4200".to_string(),
    );
    let grant = runtime
        .confirm_secret_access(&request, "123456", "123456")
        .expect("confirm secret access");
    let actor = runtime
        .authenticate_management_token(&request, "123456")
        .expect("authenticate exact grant actor");

    runtime
        .verify_secret_grant(&request, &actor, &grant.grant)
        .expect("exact grant context must pass");

    let different_origin =
        ConsoleRequestContext::new(request.client_ip(), "http://localhost:4200".to_string());
    let different_origin_actor = runtime
        .authenticate_management_token(&different_origin, "123456")
        .expect("authenticate different-origin actor");
    assert_secret_grant_required(runtime.verify_secret_grant(
        &different_origin,
        &different_origin_actor,
        &grant.grant,
    ));

    let different_ip = ConsoleRequestContext::new(
        IpAddr::from([192, 0, 2, 10]),
        request.origin_key().to_string(),
    );
    let different_ip_actor = runtime
        .authenticate_management_token(&different_ip, "123456")
        .expect("authenticate different-IP actor");
    assert_secret_grant_required(runtime.verify_secret_grant(
        &different_ip,
        &different_ip_actor,
        &grant.grant,
    ));

    let different_fingerprint_actor = AuthenticatedConsoleActor {
        token_fingerprint: "different-token-fingerprint".to_string(),
        secret_access_granted: false,
    };
    assert_secret_grant_required(runtime.verify_secret_grant(
        &request,
        &different_fingerprint_actor,
        &grant.grant,
    ));
    assert_secret_grant_required(runtime.verify_secret_grant(
        &request,
        &actor,
        &format!(" {}", grant.grant),
    ));
}

#[test]
fn expired_secret_grant_is_rejected() {
    let fixture = TestConsoleFixture::with_secret_grant_ttl(true, Some(0));
    let runtime = ConsoleAuthRuntime::new(&fixture.config, Some("123456".to_string())).unwrap();
    let request = ConsoleRequestContext::loopback();
    let grant = runtime
        .confirm_secret_access(&request, "123456", "123456")
        .expect("confirm immediately expiring secret access");
    let actor = runtime
        .authenticate_management_token(&request, "123456")
        .expect("authenticate actor");

    assert_secret_grant_required(runtime.verify_secret_grant(&request, &actor, &grant.grant));
}

fn assert_secret_grant_required(result: Result<(), GatewayError>) {
    let error = result.expect_err("secret grant context must be rejected");
    assert_eq!(
        error.code.as_deref(),
        Some("console_secret_access_required")
    );
    assert_eq!(error.http_status, Some(403));
}

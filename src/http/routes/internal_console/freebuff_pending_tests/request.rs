//! Trusted typed caller fixture, not a public AuthAdapter or completed public executor.
use super::*;
use crate::{
    auth::session::AuthenticatedSession,
    pipeline::PipelineContext,
    protocol::freebuff::catalog::{
        BusinessOrigin, CatalogSession, ClientSurface, DeviceKey, DeviceRegistration,
    },
    routing::{candidate::RouteCandidate, config::RouteConfigSnapshot},
};
use ring::signature::{Ed25519KeyPair, KeyPair};
pub(super) struct TestRequest {
    pub ctx: PipelineContext,
    pub snapshot: Arc<RouteConfigSnapshot>,
    pub candidate: RouteCandidate,
    pub body: Vec<u8>,
    pub session: CatalogSession,
    pub device: DeviceRegistration,
}
impl TestRequest {
    pub async fn new(f: &ConsoleStateFixture, peer: &wire::Peer) -> Self {
        let mut ctx = PipelineContext::new(
            crate::protocol::openai::normalize_chat_completions(
                json!({"model":"open","messages":[{"role":"user","content":"private prompt"}]}),
            )
            .unwrap(),
            None,
        );
        ctx.session = Some(AuthenticatedSession {
            project_id: "project".into(),
            tenant_id: "tenant".into(),
            user_id: Some("untrusted-body-user".into()),
            credential_ref: Some("caller-ref".into()),
            scopes: vec![],
            api_key_id: Some("caller-key-id".into()),
            user_credential_id: None,
            access_key_id: Some("access-key-id".into()),
            access_key_kind: Some("normal".into()),
        });
        let snapshot = f.state.route_config.snapshot();
        let candidate = snapshot
            .resolve_candidates(Some("open"))
            .into_iter()
            .find(|candidate| candidate.payload.credential_id.as_deref() == Some("b"))
            .unwrap();
        assert!(
            candidate.provider_credential_id.is_none(),
            "exercise actual local-pool identity"
        );
        let mut session =
            CatalogSession::new(BusinessOrigin::Codebuff, "synthetic-b", ClientSurface::Cli)
                .unwrap();
        let pkcs8 = Ed25519KeyPair::generate_pkcs8(&ring::rand::SystemRandom::new()).unwrap();
        let pair = Ed25519KeyPair::from_pkcs8(pkcs8.as_ref()).unwrap();
        let key =
            Arc::new(DeviceKey::from_pkcs8(pkcs8.as_ref(), pair.public_key().as_ref()).unwrap());
        let device =
            crate::protocol::freebuff::catalog::with_test_origin(peer.origin.clone(), async {
                session.refresh(None, Duration::from_secs(3)).await.unwrap();
                session
                    .register_device(key, Duration::from_secs(3))
                    .await
                    .unwrap()
            })
            .await;
        Self {
            ctx,
            snapshot,
            candidate,
            body:
                br#"{"model":"fbm1.open","messages":[{"role":"user","content":"private prompt"}]}"#
                    .to_vec(),
            session,
            device,
        }
    }
    pub fn binding(&self) -> RequestBinding {
        RequestBinding::capture(
            &self.ctx,
            &self.snapshot,
            &self.candidate,
            "open",
            &self.body,
        )
        .unwrap()
    }
    pub async fn begin(
        &self,
        f: &ConsoleStateFixture,
        peer: &wire::Peer,
        remaining: Duration,
    ) -> Result<WaitingRequest, crate::error::GatewayError> {
        let binding = self.binding();
        let context = crate::protocol::freebuff::catalog::PurchaseContext::new(
            "pool",
            "b",
            self.snapshot.revision().id(),
        )
        .unwrap();
        let requests = self
            .session
            .business_requests(&self.device, "open")
            .unwrap();
        let quote = crate::protocol::freebuff::catalog::with_test_origin(
            peer.origin.clone(),
            requests.purchase_quote(&context, Duration::from_secs(3)),
        )
        .await
        .unwrap();
        f.state
            .lifecycle
            .freebuff_purchases
            .begin(binding, quote, remaining)
    }
}

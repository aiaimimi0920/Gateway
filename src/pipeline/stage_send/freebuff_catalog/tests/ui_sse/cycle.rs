//! The fixture advances only callers/EOF/ack gates and verifies durable effects of browser actions.
use super::{caller::Caller, *};
pub(super) struct Cycle {
    pub host: Host,
    peer: peer::Peer,
    caller: Option<Caller>,
    pause: Option<std::sync::Arc<peer::Pause>>,
    expected: Vec<(&'static str, u64)>,
    ids: Vec<String>,
    original: Value,
    revision: String,
}
impl Cycle {
    pub async fn new(root: PathBuf) -> Self {
        let peer = peer::Peer::new().await;
        *peer.mode.lock().unwrap() = "public_sse_known".into();
        let host = Box::pin(Host::new_at(&peer, root, Duration::from_secs(300))).await;
        host.seed_device(&peer).await;
        let original = serde_json::to_value(host.state.route_config.snapshot().document()).unwrap();
        let revision = host
            .state
            .route_config
            .snapshot()
            .revision()
            .id()
            .to_owned();
        Self {
            host,
            peer,
            caller: None,
            pause: None,
            expected: Vec::new(),
            ids: Vec::new(),
            original,
            revision,
        }
    }
    pub fn check_document(&self) {
        assert_eq!(
            serde_json::to_value(self.host.state.route_config.snapshot().document()).unwrap(),
            self.original
        );
        assert_eq!(
            self.host.state.route_config.snapshot().revision().id(),
            self.revision
        );
    }
    pub async fn apply(&mut self, action: &str) {
        let n = self.ids.len();
        if action.starts_with("begin_") {
            assert!(self.caller.is_none());
            let index = if action == "begin_cancel" { 0 } else { n };
            assert_eq!(counts(&self.host).await, (0, n as i64));
            wire(&self.peer, n, n);
            self.pause = Some(self.peer.pause("/api/v1/agent-runs", (n + 1) * 2));
            self.caller = Some(Caller::start(&self.host, index));
            let pending = wait_pending(&self.host, &self.caller.as_ref().unwrap().task).await;
            assert_eq!(pending["quote"]["walletSpend"], 2.0);
        } else if action == "cancelled" {
            self.caller.take().unwrap().cancelled().await;
            self.expected.push(("failed", 0));
            audits(&self.host, &self.expected, 10).await;
            assert_eq!(counts(&self.host).await, (0, 0));
            wire(&self.peer, 0, 0);
        } else if action.starts_with("live_") || action.starts_with("eof_") {
            if action.starts_with("eof_") {
                self.peer.live.eof.notify_one();
                self.pause.as_ref().unwrap().reached().await;
                assert_eq!(self.peer.finishes().len(), n + 1);
            } else {
                assert_eq!(self.peer.finishes().len(), n);
            }
            let text = self.caller.as_ref().unwrap().live().await;
            assert_eq!(
                self.host
                    .state
                    .concurrency_registry
                    .get_or_create("pool")
                    .snapshot()
                    .active_count,
                1
            );
            assert_eq!(counts(&self.host).await, (1, n as i64));
            ui::write(
                &self.host.root,
                &format!("caller-{action}.json"),
                json!({"phase":action,"text":text,"provisional":true,"permit":1}),
            );
        } else if action.starts_with("completed_") {
            self.pause.take().unwrap().resume();
            let text = self.caller.take().unwrap().completed().await;
            self.ids.push(attempt(&self.peer, n));
            self.expected.push(("completed", 1));
            audits(&self.host, &self.expected, 9 - n as i64).await;
            assert_eq!(counts(&self.host).await, (1, n as i64));
            wire(&self.peer, n + 1, n);
            ui::write(
                &self.host.root,
                &format!("caller-{action}.json"),
                json!({"phase":action,"text":text,"terminalCount":1,"permit":0}),
            );
            // FINISH is not session end. A new public request still cannot buy or generate.
            let mut body = caller::input(n);
            body["stream"] = json!(false);
            let (status, error) = self.host.public_at(caller::path(n), body).await.unwrap();
            assert_eq!(status, 409, "{error}");
            assert_eq!(error["error"]["code"], "freebuff_public_existing_claim");
            self.expected.push(("failed", 0));
            audits(&self.host, &self.expected, 9 - n as i64).await;
            wire(&self.peer, n + 1, n);
        } else if action == "end_cancelled" {
            assert_eq!(counts(&self.host).await, (1, 0));
            wire(&self.peer, 1, 0);
        } else if action.starts_with("ended_") {
            assert_eq!(counts(&self.host).await, (0, n as i64));
            wire(&self.peer, n, n);
        } else {
            panic!("Unknown fixture action {action}");
        }
    }
    pub async fn close(self, sequence: usize, checkpoints: Vec<Value>) {
        assert!(self.caller.is_none());
        assert_eq!(self.ids.len(), 3);
        assert_eq!(
            self.ids
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len(),
            3
        );
        assert_eq!(counts(&self.host).await, (0, 3));
        audits(&self.host, &self.expected, 7).await;
        wire(&self.peer, 3, 3);
        self.check_document();
        let root = self.host.root.clone();
        self.host.stop().await;
        self.peer.server.close().await;
        ui::write(
            &root,
            "completed.json",
            json!({"sequence":sequence,"protocols":["chat","responses","messages"],"attemptIds":self.ids,
            "active":0,"history":3,"balance":7,"admissions":3,"modelRequests":3,"finishes":3,"signedDeletes":3,
            "checkpoints":checkpoints,"sameDocument":true,"sameRevision":true,"joined":true,"syntheticUpstream":true}),
        );
    }
}

//! One lazy, bounded credential-free egress probe worker; never borrow upstream authentication/cookies.
use super::*;
use crate::state::AppState;
use std::{sync::Arc, time::Duration};

const TARGETS: [&str; 2] = [
    "https://www.gstatic.com/generate_204",
    "https://cp.cloudflare.com/generate_204",
];

pub(crate) fn schedule(state: &Arc<AppState>) {
    let runtime = &state.lifecycle.model_circuits;
    {
        let mut network = runtime.network.lock();
        if network.started || state.shutdown.is_requested() {
            return;
        }
        network.started = true;
        network.enabled = true;
    }
    let weak = Arc::downgrade(state);
    let shutdown = state.shutdown.clone();
    let wake = runtime.wake.clone();
    tokio::spawn(async move {
        let client = match crate::http_client::builder()
            .timeout(Duration::from_secs(4))
            .redirect(rquest::redirect::Policy::none())
            .build()
        {
            Ok(client) => client,
            Err(_) => {
                if let Some(state) = weak.upgrade() {
                    let mut network = state.lifecycle.model_circuits.network.lock();
                    network.enabled = false;
                    network.started = false;
                    network.checking = false;
                    network.unstable = false;
                    network.baseline.clear();
                }
                tracing::error!("Cannot construct egress probe client");
                return;
            }
        };
        loop {
            let Some(state) = weak.upgrade() else {
                return;
            };
            if let Some(epoch) = state.lifecycle.model_circuits.network_probe_due() {
                let reachable = tokio::select! {
                    _=shutdown.wait()=>return,
                    result=probe(&client,&TARGETS)=>result,
                };
                let result = health_control::complete_probe(&state, epoch, reachable).await;
                if result.is_err() {
                    // Keep the incident active and retry recovery, including partial storage failures.
                    state
                        .lifecycle
                        .model_circuits
                        .network
                        .lock()
                        .probe_in_flight = false;
                    tracing::warn!(
                        "Egress recovery could not confirm health rollback; it will retry"
                    );
                } else {
                    tracing::info!(reachable, epoch, "Gateway egress network probe completed");
                }
            }
            drop(state);
            tokio::select! { _=shutdown.wait()=>return, _=wake.notified()=>{}, _=tokio::time::sleep(Duration::from_secs(15))=>{} }
        }
    });
}

pub(super) async fn probe(client: &rquest::Client, targets: &[&str]) -> bool {
    // Fixed HTTPS targets in production; tests use loopback. Do not follow redirects or read bodies.
    let results = futures::future::join_all(targets.iter().take(2).map(|url| async move {
        client
            .get(*url)
            .send()
            .await
            .is_ok_and(|response| response.status().is_success())
    }))
    .await;
    results.into_iter().any(|reachable| reachable)
}

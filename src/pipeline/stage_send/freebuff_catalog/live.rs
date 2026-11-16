//! Own the original paid operation across HTTP handoff, with bounded co-polled delivery only.
use super::*;
use std::sync::{atomic::AtomicBool, Mutex};
mod chat;
mod projected;
mod projection;
mod source;
pub(super) async fn prepare(
    ctx: &PipelineContext,
    state: &Arc<AppState>,
    candidate: &RouteCandidate,
    model: &str,
    gate: &stage_rate_limit::ProviderAttemptGate,
    ready: Ready,
    permit: crate::concurrency::aimd::ConcurrencyPermit,
) -> Result<
    (
        ByteStream,
        UsageHandle,
        Arc<Mutex<Option<String>>>,
        Arc<AtomicBool>,
    ),
    GatewayError,
> {
    let input = paid::Input::capture(ctx, state, candidate, model, gate, ready)?;
    let terminal = Arc::new(AtomicBool::new(false));
    let usage = Arc::new(Mutex::new(None));
    let usage_owner = usage.clone();
    let semantics = Arc::new(Mutex::new(None));
    let semantics_owner = semantics.clone();
    let (sender, receiver) = tokio::sync::mpsc::channel(1);
    let projection = Arc::new(Mutex::new(projection::Projection::new(ctx)));
    let observe_projection = projection.clone();
    let observe_sender = sender.clone();
    let future = async move {
        let _permit = permit;
        let mut tail = None;
        let completed = paid::execute_observed(
            input,
            move |frame| {
                let sender = observe_sender.clone();
                let bytes = observe_projection.lock().unwrap().frame(frame);
                async move {
                    sender
                        .send(source::Delivery {
                            bytes: bytes?,
                            terminal: false,
                        })
                        .await
                        .map_err(|_| BusinessError::Cancelled)
                }
            },
            |body| {
                tail = Some(projection.lock().unwrap().finish(body)?);
                Ok(serde_json::Value::Null)
            },
        )
        .await?;
        *usage_owner.lock().unwrap() = completed.usage;
        *semantics_owner.lock().unwrap() = completed.semantics;
        let bytes = tail.ok_or_else(|| business(BusinessError::InvalidResponse))?;
        sender
            .send(source::Delivery {
                bytes,
                terminal: true,
            })
            .await
            .map_err(|_| business(BusinessError::Cancelled))?;
        Ok(())
    };
    // The test-only origin must cover body polling too, not merely dispatch before HTTP handoff.
    #[cfg(all(test, feature = "line-freebuff-web-reverse-api"))]
    let future = {
        let origin = crate::protocol::freebuff::catalog::current_test_origin();
        async move {
            match origin {
                Some(origin) => {
                    crate::protocol::freebuff::catalog::with_test_origin(origin, future).await
                }
                None => future.await,
            }
        }
    };
    let mut source = source::Source::new(future, receiver, terminal.clone());
    source.prefetch().await?;
    Ok((Box::pin(source), usage, semantics, terminal))
}

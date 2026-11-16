//! Request-owned operator consent transport, never an idle purchase or a detached generation task.
mod binding;
mod waiting;
use crate::{
    error::GatewayError,
    protocol::freebuff::catalog::{PurchaseContext, PurchaseQuote},
};
pub(crate) use binding::RequestBinding;
use parking_lot::Mutex;
use serde::Serialize;
use serde_json::Value;
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::{sync::oneshot, time::Instant};
use uuid::Uuid;
pub(crate) use waiting::{ApprovedRequest, WaitingRequest};
pub(crate) type Fence = Arc<dyn Fn() -> Result<(), GatewayError> + Send + Sync>;
const CAPACITY: usize = 16;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PendingView {
    pending_id: String,
    quote_id: String,
    request_id: String,
    caller_fingerprint: String,
    request_sha256: String,
    quote: Value,
}
struct Entry {
    context: PurchaseContext,
    view: PendingView,
    deadline: Instant,
    // None means a decision was delivered. Retain its slot until the request drops/consumes it.
    sender: Option<oneshot::Sender<Decision>>,
}
enum Decision {
    Approve(Fence),
    Cancel,
}
#[derive(Default)]
struct Entries {
    closed: bool,
    entries: HashMap<Uuid, Entry>,
}
#[derive(Default)]
pub(crate) struct Registry(Mutex<Entries>);
impl std::fmt::Debug for Registry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let entries = self.0.lock();
        f.debug_struct("FreeBuffPurchaseRegistry")
            .field("closed", &entries.closed)
            .field("slots", &entries.entries.len())
            .finish()
    }
}
pub(super) fn failure(code: &str) -> GatewayError {
    GatewayError::conflict("FreeBuff request consent is unavailable, expired, cancelled or no longer bound to this request. No replacement purchase was authorized.").with_code(code)
}
fn retain(entries: &mut Entries) {
    let now = Instant::now();
    entries.entries.retain(|_, entry| {
        entry.deadline > now
            && entry
                .sender
                .as_ref()
                .is_none_or(|sender| !sender.is_closed())
    });
}
impl Registry {
    /// Only an actual request owner can call this with an original non-deserializable quote.
    /// The public producer must retain the original quote, request binding and signed executor.
    pub(crate) fn begin(
        self: &Arc<Self>,
        binding: RequestBinding,
        quote: PurchaseQuote,
        request_remaining: Duration,
    ) -> Result<WaitingRequest, GatewayError> {
        if !cfg!(feature = "line-freebuff-web-reverse-api") {
            return Err(failure("gateway_provider_line_compiled_out"));
        }
        let queued_at = Instant::now();
        let lifetime = quote
            .pending_lifetime(&binding.context, &binding.model)
            .map_err(|e| failure(&e.to_string()))?
            .min(request_remaining);
        if lifetime.is_zero() {
            return Err(failure("freebuff_purchase_quote_expired"));
        }
        let id = Uuid::new_v4();
        let (sender, receiver) = oneshot::channel();
        // Serialize only the bounded typed meter outside the registry lock; never retain prompt/token.
        let view = PendingView {
            pending_id: id.to_string(),
            quote_id: Uuid::new_v4().to_string(),
            request_id: binding.request_id.to_string(),
            caller_fingerprint: hex::encode(binding.caller),
            request_sha256: hex::encode(binding.body),
            quote: serde_json::to_value(quote.view())
                .map_err(|_| failure("freebuff_purchase_quote_unavailable"))?,
        };
        let deadline = queued_at + lifetime;
        let mut entries = self.0.lock();
        retain(&mut entries);
        if entries.closed {
            return Err(failure("freebuff_purchase_queue_closed"));
        }
        if entries.entries.len() >= CAPACITY {
            return Err(failure("freebuff_purchase_queue_full"));
        }
        if entries
            .entries
            .values()
            .any(|entry| entry.view.request_id == view.request_id)
        {
            return Err(failure("freebuff_purchase_request_already_pending"));
        }
        entries.entries.insert(
            id,
            Entry {
                context: binding.context.clone(),
                view,
                deadline,
                sender: Some(sender),
            },
        );
        drop(entries);
        Ok(WaitingRequest {
            lease: Lease {
                registry: self.clone(),
                id,
            },
            binding,
            quote,
            receiver,
            deadline,
        })
    }
    pub(crate) fn list(&self, context: &PurchaseContext) -> Vec<PendingView> {
        let mut entries = self.0.lock();
        retain(&mut entries);
        let mut views: Vec<_> = entries
            .entries
            .values()
            .filter(|entry| entry.context == *context && entry.sender.is_some())
            .map(|entry| entry.view.clone())
            .collect();
        views.sort_by(|a, b| a.pending_id.cmp(&b.pending_id));
        views
    }
    /// Authorization is evaluated outside the lock and carried to the live caller, not converted to bool.
    pub(crate) fn decide(
        &self,
        context: &PurchaseContext,
        id: Uuid,
        quote_id: Uuid,
        approve: bool,
        fence: Fence,
    ) -> Result<(), GatewayError> {
        fence()?;
        let sender = {
            let mut entries = self.0.lock();
            retain(&mut entries);
            let entry = entries
                .entries
                .get_mut(&id)
                .filter(|entry| {
                    entry.context == *context && entry.view.quote_id == quote_id.to_string()
                })
                .ok_or_else(|| failure("freebuff_purchase_pending_gone"))?;
            entry
                .sender
                .take()
                .ok_or_else(|| failure("freebuff_purchase_pending_gone"))?
        };
        let decision = if approve {
            Decision::Approve(fence)
        } else {
            Decision::Cancel
        };
        sender
            .send(decision)
            .map_err(|_| failure("freebuff_purchase_pending_gone"))
    }
    pub(crate) fn close(&self) {
        let mut entries = self.0.lock();
        entries.closed = true;
        entries.entries.clear();
    }
}
struct Lease {
    registry: Arc<Registry>,
    id: Uuid,
}
impl Lease {
    fn check(&self) -> Result<(), GatewayError> {
        let mut entries = self.registry.0.lock();
        retain(&mut entries);
        if entries.closed || !entries.entries.contains_key(&self.id) {
            Err(failure("freebuff_purchase_pending_gone"))
        } else {
            Ok(())
        }
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        self.registry.0.lock().entries.remove(&self.id);
    }
}

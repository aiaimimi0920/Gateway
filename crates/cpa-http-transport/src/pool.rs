//! Bounded origin pool. HTTP/1 owns its slot through EOF; HTTP/2 multiplexes.
use crate::{dial, wire, Error, Result};
use http::{Request, Response, Uri};
use hyper_util::client::proxy::matcher::Matcher;
use rama_boring::ssl::SslConnector;
use std::{collections::HashMap, future::Future, sync::Arc, time::Duration};
use tokio::{
    sync::{watch, Mutex, OwnedMutexGuard, OwnedSemaphorePermit, Semaphore},
    task::JoinHandle,
    time::Instant,
};
use wreq_proto::{
    body::Incoming,
    conn::{http1, http2},
    rt::Executor,
};

type Slot = Arc<Mutex<Option<Connection>>>;
type Activity = watch::Sender<(usize, Instant)>;

#[derive(Clone)]
struct TokioExecutor;
impl<F: Future<Output = ()> + Send + 'static> Executor<F> for TokioExecutor {
    fn execute(&self, future: F) {
        tokio::spawn(future);
    }
}

enum Sender {
    H1(http1::SendRequest<rquest::Body>),
    H2(http2::SendRequest<rquest::Body>),
}
struct Connection {
    sender: Sender,
    driver: JoinHandle<()>,
    activity: Activity,
}
impl Drop for Connection {
    fn drop(&mut self) {
        self.driver.abort();
    }
}

// Before response headers, cancellation leaves no reusable HTTP/1 exchange boundary.
// Invalidate under the origin lock before a subsequent explicit request can acquire it.
struct H1Pending(Option<OwnedMutexGuard<Option<Connection>>>);
impl Drop for H1Pending {
    fn drop(&mut self) {
        if let Some(guard) = self.0.as_mut() {
            **guard = None;
        }
    }
}

pub(crate) struct Lease {
    activity: Activity,
    _slot: Slot,
    _h1: Option<OwnedMutexGuard<Option<Connection>>>,
    _admission: OwnedSemaphorePermit,
    pub(crate) body_complete: bool,
}
impl Drop for Lease {
    fn drop(&mut self) {
        if !self.body_complete {
            if let Some(guard) = self._h1.as_mut() {
                **guard = None;
            }
        }
        self.activity.send_modify(|(count, last)| {
            *count -= 1;
            *last = Instant::now();
        });
    }
}

#[derive(Default)]
pub(crate) struct Pool {
    slots: Mutex<HashMap<String, Vec<Slot>>>,
}
impl Pool {
    pub(crate) async fn send(
        &self,
        origin: &Uri,
        mut request: Request<rquest::Body>,
        tls: &SslConnector,
        proxy: &Matcher,
        admission: OwnedSemaphorePermit,
    ) -> Result<(Response<Incoming>, Lease)> {
        let key = format!(
            "{}://{}",
            origin.scheme_str().ok_or(Error::Target)?,
            origin.authority().ok_or(Error::Target)?
        );
        let (slot, mut guard) = {
            let mut slots = self.slots.lock().await;
            // Global bounds include idle connections. Evict only unleased idle slots.
            if (!slots.contains_key(&key) && slots.len() >= 64)
                || slots.values().map(Vec::len).sum::<usize>() >= 128
            {
                slots.retain(|_, list| {
                    list.retain(|slot| Arc::strong_count(slot) > 1);
                    !list.is_empty()
                });
            }
            if !slots.contains_key(&key) && slots.len() >= 64 {
                return Err(Error::Capacity);
            }
            let count = slots.values().map(Vec::len).sum::<usize>();
            let list = slots.entry(key).or_default();
            // Reserve under the index lock: two concurrent callers must not both choose
            // one idle HTTP/1 slot and serialize behind its subsequently leased body.
            let idle = list.iter().find_map(|slot| {
                slot.clone()
                    .try_lock_owned()
                    .ok()
                    .map(|guard| (slot.clone(), guard))
            });
            if let Some(idle) = idle {
                idle
            } else {
                if count >= 128 {
                    return Err(Error::Capacity);
                }
                let slot = Slot::default();
                let guard = slot.clone().try_lock_owned().expect("new unshared slot");
                list.push(slot.clone());
                (slot, guard)
            }
        };
        if guard.as_ref().is_some_and(|c| c.driver.is_finished()) {
            *guard = None;
        }
        if guard.is_none() {
            *guard = Some(open(origin, tls, proxy).await?);
        }
        let conn = guard.as_mut().ok_or(Error::Network)?;
        let activity = conn.activity.clone();
        activity.send_modify(|(count, _)| *count += 1);
        let mut lease = Lease {
            activity,
            _slot: slot,
            _h1: None,
            _admission: admission,
            body_complete: false,
        };
        wire::prepare(&mut request, matches!(conn.sender, Sender::H2(_)))?;
        match &mut conn.sender {
            Sender::H1(_) => {
                let mut pending = H1Pending(Some(guard));
                let connection = pending
                    .0
                    .as_mut()
                    .and_then(|guard| guard.as_mut())
                    .ok_or(Error::Network)?;
                let Sender::H1(sender) = &mut connection.sender else {
                    unreachable!()
                };
                sender.ready().await.map_err(|_| Error::Network)?;
                let result = sender
                    .try_send_request(request)
                    .await
                    .map_err(|_| Error::Network)?;
                lease._h1 = pending.0.take();
                Ok((result, lease))
            }
            Sender::H2(sender) => {
                let mut sender = sender.clone();
                drop(guard);
                sender.ready().await.map_err(|_| Error::Network)?;
                let result = sender
                    .try_send_request(request)
                    .await
                    .map_err(|_| Error::Network)?;
                Ok((result, lease))
            }
        }
    }
}

async fn idle(mut activity: watch::Receiver<(usize, Instant)>) {
    loop {
        let (count, last) = *activity.borrow_and_update();
        if count == 0 {
            tokio::select! {
                _ = tokio::time::sleep_until(last + Duration::from_secs(90)) => return,
                value = activity.changed() => { if value.is_err() { return; } }
            }
        } else if activity.changed().await.is_err() {
            return;
        }
    }
}

async fn open(origin: &Uri, tls: &SslConnector, proxy: &Matcher) -> Result<Connection> {
    let (io, h2) = dial::connect(origin, tls, proxy).await?;
    let (activity, receiver) = watch::channel((0, Instant::now()));
    let (sender, driver) = if h2 {
        let (sender, conn) = http2::Builder::new(TokioExecutor)
            .options(wire::options())
            .handshake(io)
            .await
            .map_err(|_| Error::Network)?;
        (
            Sender::H2(sender),
            tokio::spawn(async move {
                tokio::select! { _ = conn => {}, _ = idle(receiver) => {} }
            }),
        )
    } else {
        let (sender, conn) = http1::Builder::default()
            .handshake(io)
            .await
            .map_err(|_| Error::Network)?;
        (
            Sender::H1(sender),
            tokio::spawn(async move {
                tokio::select! { _ = conn => {}, _ = idle(receiver) => {} }
            }),
        )
    };
    Ok(Connection {
        sender,
        driver,
        activity,
    })
}

pub(crate) fn admission() -> Arc<Semaphore> {
    Arc::new(Semaphore::new(128))
}

use super::producer_browser_worker_tree::ProcessTreeGuard;
use std::future::Future;
use std::io;
use std::sync::Arc;
use std::time::Duration;
use tokio::process::Child;
use tokio::sync::OwnedSemaphorePermit;
use tokio::task::JoinHandle;
use tokio::time::timeout;

pub(super) async fn terminate_and_reap<T: Send + Sync + 'static>(
    child: Child,
    tree: ProcessTreeGuard,
    permit: Arc<OwnedSemaphorePermit>,
    resource: Arc<T>,
) {
    tree.terminate();
    let root = LiveRoot { child, _tree: tree };
    let _ = reap_owned(root, permit, resource, Duration::from_secs(2)).await;
}

// Separate OS waiting from ownership transfer so wait faults can be tested without live OS damage.
trait ReapRoot: Send + 'static {
    fn start_kill(&mut self);
    fn wait(&mut self) -> impl Future<Output = io::Result<()>> + Send;
}

struct LiveRoot {
    child: Child,
    _tree: ProcessTreeGuard,
}

impl ReapRoot for LiveRoot {
    fn start_kill(&mut self) {
        let _ = self.child.start_kill();
    }

    async fn wait(&mut self) -> io::Result<()> {
        self.child.wait().await.map(|_| ())
    }
}

async fn reap_owned<R: ReapRoot, T: Send + Sync + 'static>(
    mut root: R,
    permit: Arc<OwnedSemaphorePermit>,
    resource: Arc<T>,
    grace: Duration,
) -> Option<JoinHandle<()>> {
    root.start_kill();
    if matches!(timeout(grace, root.wait()).await, Ok(Ok(()))) {
        return None;
    }
    // Retain all owners until successful reaping, not merely until the caller's grace expires.
    Some(tokio::spawn(async move {
        let _resource = resource;
        let _permit = permit;
        loop {
            root.start_kill();
            if root.wait().await.is_ok() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        drop(root);
    }))
}

#[cfg(test)]
#[path = "gemini_canvas_encoder_reaper_tests.rs"]
mod tests;

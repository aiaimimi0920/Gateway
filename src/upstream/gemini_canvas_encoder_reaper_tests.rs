use super::*;
use crate::upstream::gemini_canvas_encoder_workspace::EncoderWorkspace;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::{oneshot, Semaphore};

enum WaitStep {
    Result(io::Result<()>),
    Gate(oneshot::Sender<()>, oneshot::Receiver<io::Result<()>>),
}

struct FaultRoot {
    steps: VecDeque<WaitStep>,
    kills: Arc<AtomicUsize>,
    drops: Arc<AtomicUsize>,
}

impl ReapRoot for FaultRoot {
    fn start_kill(&mut self) {
        self.kills.fetch_add(1, Ordering::SeqCst);
    }

    async fn wait(&mut self) -> io::Result<()> {
        match self.steps.pop_front().expect("unexpected extra wait") {
            WaitStep::Result(result) => result,
            WaitStep::Gate(started, release) => {
                let _ = started.send(());
                release.await.expect("fault gate abandoned")
            }
        }
    }
}

impl Drop for FaultRoot {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}

struct Fixture {
    slots: Arc<Semaphore>,
    kills: Arc<AtomicUsize>,
    drops: Arc<AtomicUsize>,
    directory: PathBuf,
}

impl Fixture {
    fn new(
        steps: impl IntoIterator<Item = WaitStep>,
    ) -> (
        Self,
        FaultRoot,
        Arc<EncoderWorkspace>,
        Arc<OwnedSemaphorePermit>,
    ) {
        let slots = Arc::new(Semaphore::new(1));
        let permit = Arc::new(slots.clone().try_acquire_owned().unwrap());
        let workspace =
            Arc::new(EncoderWorkspace::create(b"source", "png", permit.clone()).unwrap());
        let fixture = Self {
            slots,
            kills: Arc::new(AtomicUsize::new(0)),
            drops: Arc::new(AtomicUsize::new(0)),
            directory: workspace.source.parent().unwrap().to_owned(),
        };
        let root = FaultRoot {
            steps: steps.into_iter().collect(),
            kills: fixture.kills.clone(),
            drops: fixture.drops.clone(),
        };
        (fixture, root, workspace, permit)
    }

    fn assert_retained(&self) {
        assert!(self.directory.join("source.png").exists());
        assert_eq!(self.slots.available_permits(), 0);
        assert_eq!(self.drops.load(Ordering::SeqCst), 0);
    }

    async fn assert_released(&self) {
        let permit = timeout(Duration::from_secs(5), self.slots.clone().acquire_owned())
            .await
            .unwrap()
            .unwrap();
        assert!(!self.directory.exists());
        assert_eq!(self.drops.load(Ordering::SeqCst), 1);
        drop(permit);
    }
}

fn gate() -> (
    WaitStep,
    oneshot::Receiver<()>,
    oneshot::Sender<io::Result<()>>,
) {
    let (started, ready) = oneshot::channel();
    let (release, released) = oneshot::channel();
    (WaitStep::Gate(started, released), ready, release)
}

async fn entered(ready: oneshot::Receiver<()>) {
    timeout(Duration::from_secs(5), ready)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn gemini_canvas_encoder_reaper_foreground_success_releases_owners() {
    let (fixture, root, workspace, permit) = Fixture::new([WaitStep::Result(Ok(()))]);
    assert!(reap_owned(root, permit, workspace, Duration::from_secs(1))
        .await
        .is_none());
    assert_eq!(fixture.kills.load(Ordering::SeqCst), 1);
    fixture.assert_released().await;
}

#[tokio::test]
async fn gemini_canvas_encoder_reaper_timeout_transfers_owners() {
    let (foreground, _, old_release) = gate();
    let (background, ready, release) = gate();
    let (fixture, root, workspace, permit) = Fixture::new([foreground, background]);
    let task = reap_owned(root, permit, workspace, Duration::from_millis(20))
        .await
        .unwrap();
    entered(ready).await;
    assert!(
        old_release.send(Ok(())).is_err(),
        "foreground wait was not cancelled"
    );
    fixture.assert_retained();
    assert_eq!(fixture.kills.load(Ordering::SeqCst), 2);
    release.send(Ok(())).unwrap();
    timeout(Duration::from_secs(5), task)
        .await
        .unwrap()
        .unwrap();
    fixture.assert_released().await;
}

#[tokio::test]
async fn gemini_canvas_encoder_reaper_wait_errors_retain_owners_until_retry_success() {
    let (first, first_ready, first_release) = gate();
    let (second, second_ready, second_release) = gate();
    let (fixture, root, workspace, permit) = Fixture::new([
        WaitStep::Result(Err(io::Error::other("foreground fault"))),
        first,
        second,
    ]);
    let task = reap_owned(root, permit, workspace, Duration::from_secs(1))
        .await
        .unwrap();
    entered(first_ready).await;
    fixture.assert_retained();
    let retry_start = tokio::time::Instant::now();
    first_release
        .send(Err(io::Error::other("background fault")))
        .unwrap();
    entered(second_ready).await;
    assert!(retry_start.elapsed() >= Duration::from_millis(50));
    fixture.assert_retained();
    assert_eq!(fixture.kills.load(Ordering::SeqCst), 3);
    second_release.send(Ok(())).unwrap();
    timeout(Duration::from_secs(5), task)
        .await
        .unwrap()
        .unwrap();
    fixture.assert_released().await;
}

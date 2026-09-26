use super::gemini_canvas_encoder_reaper::terminate_and_reap;
use super::producer_browser_worker_tree::ProcessTreeGuard;
use std::io;
use std::process::{Output, Stdio};
use std::sync::{Arc, LazyLock};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;
use tokio::sync::{oneshot, OwnedSemaphorePermit, Semaphore};
use tokio::task::JoinHandle;
use tokio::time::{timeout_at, Instant};

pub(super) struct EncoderAdmission {
    deadline: Instant,
    permit: Arc<OwnedSemaphorePermit>,
}

impl EncoderAdmission {
    pub(super) async fn acquire(budget: Duration) -> Option<Self> {
        Self::acquire_from(PROCESS_SLOTS.clone(), budget).await
    }

    async fn acquire_from(slots: Arc<Semaphore>, budget: Duration) -> Option<Self> {
        let deadline = Instant::now().checked_add(budget)?;
        let permit = timeout_at(deadline, slots.acquire_owned())
            .await
            .ok()?
            .ok()?;
        if deadline <= Instant::now() {
            return None;
        }
        Some(Self {
            deadline,
            permit: Arc::new(permit),
        })
    }

    pub(super) fn lease(&self) -> Arc<OwnedSemaphorePermit> {
        self.permit.clone()
    }

    #[cfg(test)]
    async fn run(self, command: Command) -> Option<Output> {
        self.run_retaining(command, Arc::new(())).await
    }

    pub(super) async fn run_retaining<T: Send + Sync + 'static>(
        self,
        command: Command,
        resource: Arc<T>,
    ) -> Option<Output> {
        run_admitted_process(command, self, resource).await
    }
}

#[cfg(test)]
async fn run_encoder_process(command: Command, budget: Duration) -> Option<Output> {
    EncoderAdmission::acquire(budget).await?.run(command).await
}

async fn run_admitted_process<T: Send + Sync + 'static>(
    command: Command,
    admission: EncoderAdmission,
    resource: Arc<T>,
) -> Option<Output> {
    let EncoderAdmission { deadline, permit } = admission;
    let (sender, receiver) = oneshot::channel();
    let mut owner = EncoderTask {
        cancellation: Some(sender),
        task: tokio::spawn(supervise(
            command,
            deadline,
            receiver,
            permit,
            resource,
            tokio::time::sleep_until(deadline),
        )),
    };
    let output = (&mut owner.task).await.ok().flatten();
    owner.cancellation.take();
    output
}

const MAX_PIPE_BYTES: usize = 64 * 1024;
static PROCESS_SLOTS: LazyLock<Arc<Semaphore>> = LazyLock::new(|| Arc::new(Semaphore::new(2)));

struct EncoderTask {
    cancellation: Option<oneshot::Sender<()>>,
    task: JoinHandle<Option<Output>>,
}

impl Drop for EncoderTask {
    fn drop(&mut self) {
        if let Some(sender) = self.cancellation.take() {
            let _ = sender.send(());
        }
    }
}

async fn supervise<T: Send + Sync + 'static, E: std::future::Future<Output = ()> + Send>(
    mut command: Command,
    deadline: Instant,
    mut cancellation: oneshot::Receiver<()>,
    permit: Arc<OwnedSemaphorePermit>,
    resource: Arc<T>,
    expiry: E,
) -> Option<Output> {
    if deadline <= Instant::now()
        || cancellation.try_recv() != Err(oneshot::error::TryRecvError::Empty)
    {
        return None;
    }
    command
        .kill_on_drop(true)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut tree = ProcessTreeGuard::prepare(&mut command).ok()?;
    let mut child = command.spawn().ok()?;
    if tree.attach(&child).is_err() {
        terminate_and_reap(child, tree, permit, resource).await;
        return None;
    }
    let (Some(stdout), Some(stderr)) = (child.stdout.take(), child.stderr.take()) else {
        terminate_and_reap(child, tree, permit, resource).await;
        return None;
    };
    let output = {
        let collect = async {
            let (status, stdout, stderr) =
                tokio::try_join!(child.wait(), read_pipe(stdout), read_pipe(stderr))?;
            Ok::<_, io::Error>(Output {
                status,
                stdout,
                stderr,
            })
        };
        tokio::pin!(expiry);
        tokio::select! {
            biased;
            _ = &mut cancellation => None,
            output = collect => output.ok(),
            _ = &mut expiry => None,
        }
    };
    if output.is_none() {
        terminate_and_reap(child, tree, permit, resource).await;
    }
    output
}

async fn read_pipe(reader: impl AsyncRead + Unpin) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader
        .take(MAX_PIPE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .await?;
    if bytes.len() > MAX_PIPE_BYTES {
        return Err(io::Error::other(
            "encoder output exceeded diagnostic budget",
        ));
    }
    Ok(bytes)
}

#[cfg(test)]
#[path = "gemini_canvas_encoder_process_tests.rs"]
mod tests;

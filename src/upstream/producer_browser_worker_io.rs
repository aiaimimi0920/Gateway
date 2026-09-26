use std::process::ExitStatus;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, ChildStderr, ChildStdin, ChildStdout};

use crate::error::GatewayError;
use crate::protocol::upstream_body::MAX_ACCUMULATED_UPSTREAM_BODY_BYTES;

const WORKER_STDERR_MAX_BYTES: usize = 64 * 1024;
const WORKER_READ_CHUNK_BYTES: usize = 8 * 1024;

#[derive(Clone, Copy)]
pub(super) struct WorkerOutputLimits {
    pub(super) stdout: usize,
    pub(super) stderr: usize,
}

impl WorkerOutputLimits {
    pub(super) const fn production() -> Self {
        Self {
            stdout: MAX_ACCUMULATED_UPSTREAM_BODY_BYTES,
            stderr: WORKER_STDERR_MAX_BYTES,
        }
    }
}

#[derive(Clone, Copy)]
enum WorkerOutputStream {
    Stdout,
    Stderr,
}

impl WorkerOutputStream {
    const fn label(self) -> &'static str {
        match self {
            Self::Stdout => "stdout",
            Self::Stderr => "stderr",
        }
    }
}

#[derive(Debug)]
pub(crate) struct ProducerBrowserWorkerProcessOutput {
    pub(crate) stdout: Vec<u8>,
    pub(crate) stderr: Vec<u8>,
}

pub(super) async fn collect_worker_process_output(
    child: &mut Child,
    stdin: ChildStdin,
    stdout: ChildStdout,
    stderr: ChildStderr,
    stdin_json: &[u8],
    limits: WorkerOutputLimits,
) -> Result<ProducerBrowserWorkerProcessOutput, GatewayError> {
    let ((), status, stdout, stderr) = tokio::try_join!(
        write_worker_input(stdin, stdin_json),
        wait_for_worker(child),
        collect_bounded_worker_output(stdout, WorkerOutputStream::Stdout, limits.stdout),
        collect_bounded_worker_output(stderr, WorkerOutputStream::Stderr, limits.stderr),
    )?;
    if !status.success() {
        let stderr = String::from_utf8_lossy(&stderr);
        return Err(
            crate::protocol::producer::browser_worker_nonzero_exit_error(
                status.code(),
                stderr.trim(),
            ),
        );
    }
    Ok(ProducerBrowserWorkerProcessOutput { stdout, stderr })
}

async fn write_worker_input(mut stdin: ChildStdin, stdin_json: &[u8]) -> Result<(), GatewayError> {
    stdin.write_all(stdin_json).await.map_err(|error| {
        crate::protocol::producer::browser_worker_stdin_error(error.to_string().as_str())
    })
}

async fn wait_for_worker(child: &mut Child) -> Result<ExitStatus, GatewayError> {
    child.wait().await.map_err(|error| {
        crate::protocol::producer::browser_worker_wait_failed_error(error.to_string().as_str())
    })
}

async fn collect_bounded_worker_output<R>(
    mut reader: R,
    stream: WorkerOutputStream,
    max_bytes: usize,
) -> Result<Vec<u8>, GatewayError>
where
    R: AsyncRead + Unpin,
{
    let mut output = Vec::new();
    let mut chunk = [0_u8; WORKER_READ_CHUNK_BYTES];
    loop {
        let read = reader.read(&mut chunk).await.map_err(|error| {
            crate::protocol::producer::browser_worker_wait_failed_error(error.to_string().as_str())
        })?;
        if read == 0 {
            return Ok(output);
        }
        let next_len = output.len().checked_add(read).ok_or_else(|| {
            crate::protocol::producer::browser_worker_output_too_large_error(
                stream.label(),
                max_bytes,
            )
        })?;
        if next_len > max_bytes {
            return Err(
                crate::protocol::producer::browser_worker_output_too_large_error(
                    stream.label(),
                    max_bytes,
                ),
            );
        }
        reserve_worker_output_capacity(&mut output, next_len, max_bytes, stream)?;
        output.extend_from_slice(&chunk[..read]);
    }
}

fn reserve_worker_output_capacity(
    output: &mut Vec<u8>,
    required_len: usize,
    max_bytes: usize,
    stream: WorkerOutputStream,
) -> Result<(), GatewayError> {
    if required_len <= output.capacity() {
        return Ok(());
    }
    let target_capacity = output
        .capacity()
        .max(WORKER_READ_CHUNK_BYTES)
        .saturating_mul(2)
        .min(max_bytes)
        .max(required_len);
    output
        .try_reserve_exact(target_capacity.saturating_sub(output.len()))
        .map_err(|_| {
            crate::protocol::producer::browser_worker_output_buffer_allocation_error(stream.label())
        })
}

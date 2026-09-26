use super::MAX_DRIVER_OUTPUT_BYTES;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, ChildStdin, ChildStdout};

pub(super) async fn collect_output(
    child: &mut Child,
    request: &[u8],
    timeout: Duration,
) -> anyhow::Result<Vec<u8>> {
    let result = tokio::time::timeout(timeout, exchange(child, request))
        .await
        .unwrap_or_else(|_| Err(anyhow::anyhow!("credential automation script timed out")));
    if result.is_err() {
        // Cancel pipe futures before killing and reaping the direct child.
        let _ = child.start_kill();
        let _ = child.wait().await;
    }
    result
}

async fn exchange(child: &mut Child, request: &[u8]) -> anyhow::Result<Vec<u8>> {
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| anyhow::anyhow!("failed to write credential automation script input"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow::anyhow!("credential automation script failed to exit cleanly"))?;
    let mut stderr = child
        .stderr
        .take()
        .ok_or_else(|| anyhow::anyhow!("credential automation script failed to exit cleanly"))?;
    let discard_stderr = async move {
        tokio::io::copy(&mut stderr, &mut tokio::io::sink())
            .await
            .map_err(|_| anyhow::anyhow!("credential automation script failed to exit cleanly"))
    };
    let wait = async {
        child
            .wait()
            .await
            .map_err(|_| anyhow::anyhow!("credential automation script failed to exit cleanly"))
    };
    let (_, stdout, _, status) = tokio::try_join!(
        write_input(stdin, request),
        read_stdout(stdout),
        discard_stderr,
        wait
    )?;
    if !status.success() {
        anyhow::bail!("credential automation script returned a non-zero exit status");
    }
    Ok(stdout)
}

async fn write_input(mut stdin: ChildStdin, request: &[u8]) -> anyhow::Result<()> {
    async {
        stdin.write_all(request).await?;
        // Windows pipes can accept buffered writes before the OS reports delivery errors.
        stdin.flush().await
    }
    .await
    .map_err(|_| anyhow::anyhow!("failed to write credential automation script input"))
}

async fn read_stdout(mut stdout: ChildStdout) -> anyhow::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut chunk = [0; 8 * 1024];
    loop {
        let count = stdout
            .read(&mut chunk)
            .await
            .map_err(|_| anyhow::anyhow!("credential automation script failed to exit cleanly"))?;
        if count == 0 {
            return Ok(bytes);
        }
        if count > MAX_DRIVER_OUTPUT_BYTES.saturating_sub(bytes.len()) {
            anyhow::bail!("credential automation driver response exceeded the size limit");
        }
        bytes
            .try_reserve(count)
            .map_err(|_| anyhow::anyhow!("credential automation script failed to exit cleanly"))?;
        bytes.extend_from_slice(&chunk[..count]);
    }
}

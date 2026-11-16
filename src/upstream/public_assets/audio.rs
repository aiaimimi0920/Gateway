//! Bounded anonymous MP3 verification: exact framing plus full PCM decode, not a MIME sniff.
mod framing;
mod reservoir;
#[cfg(test)]
mod tests;
use super::Failure;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, LazyLock,
};
use symphonia::core::{
    audio::{AudioBufferRef, Signal},
    codecs::{CodecParameters, CODEC_TYPE_MP3},
    formats::Packet,
};

// Admission covers both compressed bytes and blocking work; cancellation cannot release a
// permit while its decoder is still running. Only one frame of PCM is retained per decoder.
static OWNERS: LazyLock<Arc<tokio::sync::Semaphore>> =
    LazyLock::new(|| Arc::new(tokio::sync::Semaphore::new(2)));
pub(in crate::upstream) async fn verify_url(
    raw: &str,
    deadline: tokio::time::Instant,
    limit: usize,
) -> Result<usize, Failure> {
    let permit = tokio::time::timeout_at(deadline, OWNERS.clone().acquire_owned())
        .await
        .map_err(|_| Failure::Timeout)?
        .map_err(|_| Failure::Limit)?;
    let (bytes, mime) = super::download(raw, deadline, limit).await?;
    if !matches!(
        mime.as_str(),
        "audio/mpeg" | "audio/mp3" | "application/octet-stream"
    ) {
        return Err(Failure::Type);
    }
    let stop = Stop(Arc::new(AtomicBool::new(false)));
    let cancelled = stop.0.clone();
    tokio::time::timeout_at(
        deadline,
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            decode(&bytes, &cancelled, deadline)?;
            Ok(bytes.len())
        }),
    )
    .await
    .map_err(|_| Failure::Timeout)?
    .map_err(|_| Failure::Type)?
}
struct Stop(Arc<AtomicBool>);
impl Drop for Stop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

fn decode(
    bytes: &[u8],
    cancelled: &AtomicBool,
    deadline: tokio::time::Instant,
) -> Result<(), Failure> {
    let mut frames = framing::payload(bytes)?;
    let mut decoder = symphonia::default::get_codecs()
        .make(
            CodecParameters::new().for_codec(CODEC_TYPE_MP3),
            &Default::default(),
        )
        .map_err(|_| Failure::Type)?;
    let mut samples = 0u64;
    let mut unread = 0;
    let mut signal = None;
    while !frames.is_empty() {
        if cancelled.load(Ordering::Relaxed) || tokio::time::Instant::now() >= deadline {
            return Err(Failure::Timeout);
        }
        let size = framing::frame_size(frames)?;
        // The decoder deliberately conceals reservoir underflow as silence. Reject missing
        // history before decoding, rather than counting repaired PCM as an intact asset.
        reservoir::advance(&frames[..size], &mut unread)?;
        let packet = Packet::new_from_slice(0, samples, 0, &frames[..size]);
        let decoded = decoder.decode(&packet).map_err(|_| Failure::Type)?;
        let AudioBufferRef::F32(pcm) = decoded else {
            return Err(Failure::Type);
        };
        let spec = *pcm.spec();
        if pcm.frames() == 0
            || spec.rate == 0
            || spec.rate > 48000
            || spec.channels.count() > 2
            || signal.is_some_and(|previous| previous != spec)
            || (0..spec.channels.count()).any(|c| pcm.chan(c).iter().any(|s| !s.is_finite()))
        {
            return Err(Failure::Type);
        }
        signal = Some(spec);
        samples += pcm.frames() as u64;
        if samples > u64::from(spec.rate) * 20 * 60 {
            return Err(Failure::Limit);
        }
        frames = &frames[size..];
    }
    if samples == 0 {
        return Err(Failure::Type);
    }
    Ok(())
}

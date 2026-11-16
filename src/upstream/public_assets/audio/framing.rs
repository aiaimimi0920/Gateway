//! Do not let a resynchronizing demuxer hide truncated frames, trailing HTML or junk.
use super::Failure;

pub(super) fn payload(mut bytes: &[u8]) -> Result<&[u8], Failure> {
    if bytes.starts_with(b"ID3") {
        let header = bytes.get(..10).ok_or(Failure::Type)?;
        let version = header[3];
        let allowed = match version {
            2 => 0xc0,
            3 => 0xe0,
            4 => 0xf0,
            _ => return Err(Failure::Type),
        };
        if header[4] == 0xff
            || header[5] & !allowed != 0
            || header[6..10].iter().any(|b| b & 0x80 != 0)
        {
            return Err(Failure::Type);
        }
        let size = header[6..10]
            .iter()
            .fold(0usize, |s, b| (s << 7) | usize::from(*b));
        if size > 1024 * 1024 {
            return Err(Failure::Limit);
        }
        let end = 10 + size;
        let footer = version == 4 && header[5] & 0x10 != 0;
        if footer {
            let tail = bytes.get(end..end + 10).ok_or(Failure::Type)?;
            if &tail[..3] != b"3DI" || tail[3..] != header[3..] {
                return Err(Failure::Type);
            }
        }
        bytes = bytes
            .get(end + if footer { 10 } else { 0 }..)
            .ok_or(Failure::Type)?;
    }
    if bytes.len() >= 128 && bytes[bytes.len() - 128..].starts_with(b"TAG") {
        bytes = &bytes[..bytes.len() - 128];
    }
    if bytes.is_empty() {
        return Err(Failure::Type);
    }
    Ok(bytes)
}

pub(super) fn frame_size(bytes: &[u8]) -> Result<usize, Failure> {
    let h = bytes.get(..4).ok_or(Failure::Type)?;
    let version = (h[1] >> 3) & 3;
    let bitrate = usize::from(h[2] >> 4);
    let rate = usize::from((h[2] >> 2) & 3);
    if h[0] != 0xff
        || h[1] & 0xe0 != 0xe0
        || version == 1
        || (h[1] >> 1) & 3 != 1
        || bitrate == 0
        || bitrate == 15
        || rate == 3
        || h[3] & 3 == 2
    {
        return Err(Failure::Type);
    }
    let table = if version == 3 {
        [
            0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320,
        ]
    } else {
        [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160]
    };
    let rate = [44100, 48000, 32000][rate]
        >> match version {
            3 => 0,
            2 => 1,
            _ => 2,
        };
    let size = (if version == 3 { 144 } else { 72 }) * table[bitrate] * 1000 / rate
        + usize::from((h[2] >> 1) & 1);
    if size > bytes.len() {
        return Err(Failure::Type);
    }
    Ok(size)
}

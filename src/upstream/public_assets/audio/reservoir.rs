//! MP3 side-info admission only; spectral/scalefactor decoding remains Symphonia's job.
use super::Failure;

pub(super) fn advance(frame: &[u8], unread: &mut usize) -> Result<(), Failure> {
    // Called only after exact Layer III frame-size admission. Fixed side-info slots carry
    // the back-reference and per-granule/channel part2_3 lengths, even for short blocks.
    let mpeg1 = frame[1] & 0x18 == 0x18;
    let mono = frame[3] >> 6 == 3;
    let channels = if mono { 1 } else { 2 };
    let side_len = match (mpeg1, mono) {
        (true, true) => 17,
        (true, false) => 32,
        (false, true) => 9,
        (false, false) => 17,
    };
    let start = 4 + if frame[1] & 1 == 0 { 2 } else { 0 };
    let side = frame.get(start..start + side_len).ok_or(Failure::Type)?;
    let back = bits(side, 0, if mpeg1 { 9 } else { 8 });
    if back > *unread {
        return Err(Failure::Type);
    }
    let slots = channels * if mpeg1 { 2 } else { 1 };
    let first = match (mpeg1, mono) {
        (true, true) => 18,
        (true, false) => 20,
        (false, true) => 9,
        (false, false) => 10,
    };
    let stride = if mpeg1 { 59 } else { 63 };
    let used = (0..slots)
        .map(|s| bits(side, first + s * stride, 12))
        .sum::<usize>()
        .div_ceil(8);
    let available = back + frame.len() - start - side_len;
    *unread = available.checked_sub(used).ok_or(Failure::Type)?;
    Ok(())
}

fn bits(bytes: &[u8], start: usize, len: usize) -> usize {
    // All offsets above are within the admitted fixed side-info size; never parse metadata.
    (start..start + len).fold(0, |n, bit| {
        (n << 1) | usize::from((bytes[bit / 8] >> (7 - bit % 8)) & 1)
    })
}

//! Structural admission of native export containers, not a decoder or intelligibility check.
pub(super) fn valid_wav(bytes: &[u8]) -> bool {
    if bytes.len() < 44
        || &bytes[..4] != b"RIFF"
        || &bytes[8..12] != b"WAVE"
        || u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize + 8 != bytes.len()
    {
        return false;
    }
    let (mut offset, mut alignment, mut data) = (12usize, None, false);
    while offset + 8 <= bytes.len() {
        let length = u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap()) as usize;
        let start = offset + 8;
        let Some(end) = start.checked_add(length).filter(|end| *end <= bytes.len()) else {
            return false;
        };
        match &bytes[offset..offset + 4] {
            b"fmt " => {
                if alignment.is_some() || length < 16 {
                    return false;
                }
                let tag = u16::from_le_bytes(bytes[start..start + 2].try_into().unwrap());
                let channels = u16::from_le_bytes(bytes[start + 2..start + 4].try_into().unwrap());
                let rate = u32::from_le_bytes(bytes[start + 4..start + 8].try_into().unwrap());
                let byte_rate =
                    u32::from_le_bytes(bytes[start + 8..start + 12].try_into().unwrap());
                let align = u16::from_le_bytes(bytes[start + 12..start + 14].try_into().unwrap());
                let bits = u16::from_le_bytes(bytes[start + 14..start + 16].try_into().unwrap());
                if tag != 1
                    || !(1..=8).contains(&channels)
                    || !(1..=192_000).contains(&rate)
                    || !matches!(bits, 8 | 16 | 24 | 32)
                    || align != channels * (bits / 8)
                    || byte_rate != rate * u32::from(align)
                {
                    return false;
                }
                alignment = Some(usize::from(align));
            }
            b"data" => {
                if data || !alignment.is_some_and(|align| length > 0 && length % align == 0) {
                    return false;
                }
                data = true;
            }
            _ => {}
        }
        offset = end + length % 2;
    }
    offset == bytes.len() && alignment.is_some() && data
}

fn ogg_crc(page: &[u8]) -> u32 {
    let mut crc = 0u32;
    for (index, byte) in page.iter().enumerate() {
        crc ^= u32::from(if (22..26).contains(&index) { 0 } else { *byte }) << 24;
        for _ in 0..8 {
            crc = if crc & 0x8000_0000 != 0 {
                (crc << 1) ^ 0x04c1_1db7
            } else {
                crc << 1
            };
        }
    }
    crc
}

pub(super) fn valid_ogg_opus(bytes: &[u8]) -> bool {
    let (mut offset, mut sequence, mut serial, mut packets, mut pending, mut ended) =
        (0usize, 0u32, None, 0usize, Vec::new(), false);
    while offset + 27 <= bytes.len() && !ended {
        let header = &bytes[offset..offset + 27];
        if &header[..4] != b"OggS" || header[4] != 0 || header[5] & !7 != 0 {
            return false;
        }
        let page_serial = u32::from_le_bytes(header[14..18].try_into().unwrap());
        let page_sequence = u32::from_le_bytes(header[18..22].try_into().unwrap());
        if serial.is_some_and(|s| s != page_serial)
            || page_sequence != sequence
            || (header[5] & 1 != 0) != !pending.is_empty()
            || (header[5] & 2 != 0) != (sequence == 0)
        {
            return false;
        }
        serial = Some(page_serial);
        sequence += 1;
        let start = offset + 27;
        let count = usize::from(header[26]);
        let Some(lacing) = bytes.get(start..start + count) else {
            return false;
        };
        let length: usize = lacing.iter().map(|v| usize::from(*v)).sum();
        let Some(body) = bytes.get(start + count..start + count + length) else {
            return false;
        };
        let end = start + count + length;
        if ogg_crc(&bytes[offset..end]) != u32::from_le_bytes(header[22..26].try_into().unwrap()) {
            return false;
        }
        let mut cursor = 0;
        for length in lacing {
            let length = usize::from(*length);
            // Export admission needs only the first 19 header bytes, not a packet-sized copy.
            let keep = (19usize.saturating_sub(pending.len())).min(length);
            pending.extend_from_slice(&body[cursor..cursor + keep]);
            cursor += length;
            if length == 255 {
                continue;
            }
            if (packets == 0
                && (pending.len() < 19
                    || !pending.starts_with(b"OpusHead")
                    || pending[8] != 1
                    || pending[9] == 0))
                || (packets == 1 && !pending.starts_with(b"OpusTags"))
                || pending.is_empty()
            {
                return false;
            }
            packets += 1;
            pending.clear();
        }
        ended = header[5] & 4 != 0;
        offset = end;
    }
    ended && offset == bytes.len() && pending.is_empty() && packets >= 3
}

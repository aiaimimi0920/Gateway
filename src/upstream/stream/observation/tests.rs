use super::*;

fn collect(
    lines: &mut ObservationLines,
    chunks: &[&[u8]],
    rule: BoundaryRule,
) -> Vec<Option<String>> {
    let mut events = Vec::new();
    for chunk in chunks {
        lines.feed(chunk, rule, |event| {
            events.push(match event {
                ObservationLine::Text(line) => Some(line.to_string()),
                ObservationLine::Reset => None,
            });
        });
    }
    events
}

fn texts(values: &[&str]) -> Vec<Option<String>> {
    values
        .iter()
        .map(|value| Some((*value).to_string()))
        .collect()
}

#[test]
fn exact_byte_budget_includes_delimiter_across_chunks() {
    let mut lines = ObservationLines::new(8, 8);
    assert_eq!(
        collect(&mut lines, &[b"da", b"ta:x\n", b"\n"], BoundaryRule::Empty),
        texts(&["data:x", ""])
    );
    assert_eq!((lines.frame_bytes, lines.frame_lines), (0, 0));
    assert!(lines.line.capacity() <= 8);
}

#[test]
fn overflow_on_delimiter_resets_pending_material() {
    let mut lines = ObservationLines::new(7, 8);
    assert_eq!(
        collect(&mut lines, &[b"data:x\n\nok\n\n"], BoundaryRule::Empty),
        vec![
            Some("data:x".into()),
            None,
            Some("ok".into()),
            Some("".into())
        ]
    );
}

#[test]
fn completed_lines_share_the_raw_event_budget() {
    let mut lines = ObservationLines::new(12, 8);
    assert_eq!(
        collect(
            &mut lines,
            &[b"data:a\ndata:b\n\nz\n\n"],
            BoundaryRule::Empty
        ),
        vec![
            Some("data:a".into()),
            None,
            Some("z".into()),
            Some("".into())
        ]
    );
}

#[test]
fn exact_line_budget_includes_the_frame_delimiter() {
    let mut lines = ObservationLines::new(32, 3);
    assert_eq!(
        collect(&mut lines, &[b"data:a\ndata:b\n\n"], BoundaryRule::Empty),
        texts(&["data:a", "data:b", ""])
    );
}

#[test]
fn comments_and_empty_data_fields_consume_line_budget() {
    let mut lines = ObservationLines::new(100, 3);
    assert_eq!(
        collect(
            &mut lines,
            &[b":note\ndata:\ndata:\n\nok\n\n"],
            BoundaryRule::Empty
        ),
        vec![
            Some(":note".into()),
            Some("data:".into()),
            Some("data:".into()),
            None,
            Some("ok".into()),
            Some("".into()),
        ]
    );
}

#[test]
fn oversized_line_recovers_in_the_same_chunk_without_retaining_it() {
    let mut lines = ObservationLines::new(8, 8);
    assert_eq!(
        collect(
            &mut lines,
            &[b"too long across\n\nok\n\n"],
            BoundaryRule::Empty
        ),
        vec![None, Some("ok".into()), Some("".into())]
    );
    assert!(lines.line.capacity() <= 8);
}

#[test]
fn discarded_line_and_blank_delimiter_can_span_chunks() {
    let mut lines = ObservationLines::new(8, 8);
    assert_eq!(
        collect(&mut lines, &[b"too long across"], BoundaryRule::Empty),
        vec![None]
    );
    assert_eq!(lines.line.capacity(), 0);
    assert_eq!(
        collect(&mut lines, &[b"\n\r", b"\nz\n\n"], BoundaryRule::Empty),
        texts(&["z", ""])
    );
}

#[test]
fn recovery_keeps_each_observers_carriage_return_delimiter_rule() {
    let mut usage = ObservationLines::new(4, 8);
    let mut completion = ObservationLines::new(4, 8);
    assert_eq!(
        collect(&mut usage, &[b"x\n\r\r\nz\n\n"], BoundaryRule::Empty),
        vec![Some("x".into()), None]
    );
    assert_eq!(
        collect(
            &mut completion,
            &[b"x\n\r\r\nz\n\n"],
            BoundaryRule::TrimCarriageReturns
        ),
        vec![Some("x".into()), None, Some("z".into()), Some("".into())]
    );
}

#[test]
fn fragmented_utf8_is_completed_and_invalid_lines_are_skipped() {
    let mut lines = ObservationLines::new(32, 8);
    assert_eq!(
        collect(
            &mut lines,
            &[b"da", b"ta:\xc3", b"\xa9\n", b"\xff\n", b"\n"],
            BoundaryRule::Empty
        ),
        texts(&["data:\u{00e9}", ""])
    );
}

#[test]
fn partial_line_growth_is_geometric_and_capped_before_overflow() {
    let mut lines = ObservationLines::new(5000, 8);
    let mut capacities = Vec::new();
    for _ in 0..5000 {
        assert!(collect(&mut lines, &[b"x"], BoundaryRule::Empty).is_empty());
        let capacity = lines.line.capacity();
        assert!(capacity <= 5000);
        if capacities.last() != Some(&capacity) {
            capacities.push(capacity);
        }
    }
    assert!(
        capacities.len() <= 4,
        "unexpected allocation churn: {capacities:?}"
    );
    assert_eq!(lines.line.len(), 5000);
    assert_eq!(
        collect(&mut lines, &[b"x"], BoundaryRule::Empty),
        vec![None]
    );
    assert_eq!(lines.line.capacity(), 0);
}

#[test]
fn large_completed_line_storage_is_released() {
    let mut lines = ObservationLines::new(MAX_RETAINED_LINE_BYTES * 3, 8);
    let mut chunk = vec![b'x'; MAX_RETAINED_LINE_BYTES + 1];
    chunk.push(b'\n');
    let mut observed_length = None;
    lines.feed(&chunk, BoundaryRule::Empty, |event| match event {
        ObservationLine::Text(line) => observed_length = Some(line.len()),
        ObservationLine::Reset => panic!("line is within budget"),
    });
    assert_eq!(observed_length, Some(MAX_RETAINED_LINE_BYTES + 1));
    assert_eq!(lines.line.capacity(), 0);
}

#[test]
fn empty_chunks_do_not_advance_or_flush_state() {
    let mut lines = ObservationLines::new(8, 8);
    assert_eq!(
        collect(
            &mut lines,
            &[b"", b"x\n", b"", b"\n", b""],
            BoundaryRule::Empty
        ),
        texts(&["x", ""])
    );
    assert_eq!((lines.frame_bytes, lines.frame_lines), (0, 0));
}

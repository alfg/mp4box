//! Container nesting is parsed recursively, so a file that declares a deep
//! chain of containers costs stack rather than bytes. A container whose size
//! overruns its parent is clamped to the parent's end, which makes every
//! nested header cost only its 8 bytes — so ~90 KB of `gmhd` headers reaches
//! several thousand levels. `MAX_BOX_DEPTH` is what keeps that from
//! overflowing the stack. Found by the video-commander `mp4_boxes` fuzz target.

use mp4box::{MAX_BOX_DEPTH, get_boxes, get_boxes_tolerant};
use std::io::Cursor;

/// `levels` nested container headers, each declaring a size far larger than
/// the file so it gets clamped to its parent's end and its children span
/// everything that follows.
fn nested_containers(typ: &[u8; 4], levels: usize) -> Vec<u8> {
    let mut v = Vec::with_capacity(levels * 8);
    for _ in 0..levels {
        v.extend_from_slice(&u32::MAX.to_be_bytes());
        v.extend_from_slice(typ);
    }
    v
}

/// Well-formed nesting: each container exactly encloses the next.
fn wrapped(typ: &[u8; 4], levels: usize) -> Vec<u8> {
    let mut v = vec![0u8; 8];
    v[4..8].copy_from_slice(typ);
    v[0..4].copy_from_slice(&8u32.to_be_bytes());
    for _ in 1..levels {
        let size = (v.len() + 8) as u32;
        let mut outer = Vec::with_capacity(v.len() + 8);
        outer.extend_from_slice(&size.to_be_bytes());
        outer.extend_from_slice(typ);
        outer.extend_from_slice(&v);
        v = outer;
    }
    v
}

fn max_depth(boxes: &[mp4box::Box]) -> usize {
    boxes
        .iter()
        .map(|b| match &b.children {
            Some(kids) if !kids.is_empty() => 1 + max_depth(kids),
            _ => 1,
        })
        .max()
        .unwrap_or(0)
}

/// Run on a deliberately small stack: 512 KB is well under any default, so an
/// unbounded parser aborts the whole test binary here rather than passing by
/// accident on a roomy main thread.
fn on_small_stack<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .stack_size(512 * 1024)
        .spawn(f)
        .expect("spawn")
        .join()
        .expect("parser must not overflow the stack")
}

#[test]
fn deep_nesting_is_capped_in_tolerant_mode() {
    let data = nested_containers(b"gmhd", 8000);
    let (boxes, issues) = on_small_stack(move || {
        let len = data.len() as u64;
        let mut cur = Cursor::new(data);
        get_boxes_tolerant(&mut cur, len, true).expect("tolerant parse")
    });

    assert_eq!(boxes.len(), 1, "the chain has a single root");
    assert!(
        max_depth(&boxes) <= MAX_BOX_DEPTH + 1,
        "tree is {} deep, cap is {}",
        max_depth(&boxes),
        MAX_BOX_DEPTH
    );
    assert!(
        issues.iter().any(|i| i.message.contains("nesting deeper")),
        "expected a depth issue, got {issues:?}"
    );
}

#[test]
fn deep_nesting_errors_in_strict_mode() {
    let data = nested_containers(b"gmhd", 8000);
    let err = on_small_stack(move || {
        let len = data.len() as u64;
        let mut cur = Cursor::new(data);
        match get_boxes(&mut cur, len, true) {
            Ok(_) => panic!("strict parse must reject deep nesting"),
            Err(e) => e,
        }
    });
    assert!(
        err.to_string().contains("nesting deeper"),
        "unexpected error: {err}"
    );
}

/// The cap must not truncate anything a real file would contain. The deepest
/// standard path is around eight levels.
#[test]
fn nesting_under_the_cap_parses_in_full() {
    let levels = MAX_BOX_DEPTH;
    let data = wrapped(b"gmhd", levels);
    let len = data.len() as u64;
    let mut cur = Cursor::new(&data[..]);
    let boxes = get_boxes(&mut cur, len, true).expect("strict parse");
    assert_eq!(max_depth(&boxes), levels);
}

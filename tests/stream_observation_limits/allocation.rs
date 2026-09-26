use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    static LARGEST_REQUEST: Cell<Option<usize>> = const { Cell::new(None) };
}

struct RecordingAllocator;

#[global_allocator]
static ALLOCATOR: RecordingAllocator = RecordingAllocator;

fn record(size: usize) {
    let _ = LARGEST_REQUEST.try_with(|largest| {
        if let Some(previous) = largest.get() {
            largest.set(Some(previous.max(size)));
        }
    });
}

// Forward the allocator contract unchanged; only record this thread's request sizes.
unsafe impl GlobalAlloc for RecordingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record(layout.size());
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record(layout.size());
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        record(new_size);
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

pub(super) fn largest_request<T>(run: impl FnOnce() -> T) -> (T, usize) {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            LARGEST_REQUEST.with(|largest| largest.set(None));
        }
    }
    LARGEST_REQUEST.with(|largest| {
        assert!(
            largest.get().is_none(),
            "allocation observation must not nest"
        );
        largest.set(Some(0));
    });
    let reset = Reset;
    let result = run();
    let largest = LARGEST_REQUEST.with(|largest| largest.get().unwrap());
    drop(reset);
    (result, largest)
}

//! One thread-local counting allocator for the isolated allocation target.

#![allow(unsafe_code, clippy::expect_used)]

use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    sync::Mutex,
};

pub(crate) static ALLOCATION_TEST_LOCK: Mutex<()> = Mutex::new(());

thread_local! {
    static COUNTING: Cell<bool> = const { Cell::new(false) };
    static COUNT: Cell<usize> = const { Cell::new(0) };
}

struct CountingAllocator;

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if COUNTING.try_with(Cell::get).unwrap_or(false) {
            let _ = COUNT.try_with(|count| count.set(count.get().saturating_add(1)));
        }
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

struct CountingGuard;

impl Drop for CountingGuard {
    fn drop(&mut self) {
        let _ = COUNTING.try_with(|counting| counting.set(false));
    }
}

pub(crate) fn allocations_during(f: impl FnOnce()) -> usize {
    let _lock = ALLOCATION_TEST_LOCK.lock().expect("allocation lock");
    COUNT.with(|count| count.set(0));
    COUNTING.with(|counting| counting.set(true));
    {
        let _guard = CountingGuard;
        f();
    }
    COUNT.with(Cell::get)
}

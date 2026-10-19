//! The memory allocator in the browser: wasi-libc's malloc holds one lock for every thread, and
//! the planner's searches spent a third of their time waiting for it. Small blocks freed are
//! kept for the next allocations of their size in free lists, in shards each under a lock of
//! its own: a thread works with the shard its thread-local storage falls in, so that threads
//! rarely share one. Nothing is kept by a thread itself (its end needs no care: wasi-libc gives
//! no safe moment for it); larger blocks go to malloc.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::UnsafeCell;
use std::ptr::null_mut;
use std::sync::atomic::{AtomicBool, Ordering};

/// Block sizes kept (bytes); a block is the smallest that holds the size asked.
const CLASSES: [usize; 16] = [
    16, 32, 48, 64, 80, 96, 128, 160, 192, 256, 320, 384, 512, 768, 1024, 2048,
];
const ALIGN: usize = 16;
const SHARD_COUNT: usize = 64;

fn class(layout: Layout) -> Option<usize> {
    if layout.align() > ALIGN {
        return None;
    }
    CLASSES.iter().position(|&c| layout.size() <= c)
}

/// The free blocks of one shard, by class, chained by their first word, under a spin lock (held
/// for a pointer move).
#[repr(align(64))]
struct Shard {
    busy: AtomicBool,
    heads: UnsafeCell<[*mut u8; 16]>,
}

// SAFETY: `heads` is only touched with `busy` held.
unsafe impl Sync for Shard {}

#[expect(clippy::declare_interior_mutable_const, reason = "the initial value of each shard")]
const SHARD: Shard = Shard {
    busy: AtomicBool::new(false),
    heads: UnsafeCell::new([null_mut(); 16]),
};

static SHARDS: [Shard; SHARD_COUNT] = [SHARD; SHARD_COUNT];

impl Shard {
    fn with<T>(&self, f: impl FnOnce(&mut [*mut u8; 16]) -> T) -> T {
        while self
            .busy
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            std::hint::spin_loop();
        }
        // SAFETY: the lock is held.
        let out = f(unsafe { &mut *self.heads.get() });
        self.busy.store(false, Ordering::Release);
        out
    }
}

thread_local! {
    /// Only its address counts: it tells the threads apart.
    static MARK: u8 = const { 0 };
}

/// The shard of this thread.
fn shard() -> &'static Shard {
    let at = MARK.with(std::ptr::from_ref) as usize;
    // Thread-local blocks are far apart: mixed, their addresses spread over the shards.
    let h = at.wrapping_mul(0x9E37_79B9) >> 7;
    &SHARDS[h % SHARD_COUNT]
}

pub struct Sharded;

// SAFETY: blocks come from `System` with the size and alignment of their class, and go back to
// a free list of that class only.
unsafe impl GlobalAlloc for Sharded {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let Some(c) = class(layout) else {
            // SAFETY: the caller's layout.
            return unsafe { System.alloc(layout) };
        };
        let block = shard().with(|heads| {
            let block = heads[c];
            if !block.is_null() {
                // SAFETY: a free block of the list, its first word the next one.
                heads[c] = unsafe { block.cast::<*mut u8>().read_unaligned() };
            }
            block
        });
        if block.is_null() {
            // SAFETY: a valid layout (a class size, its alignment).
            unsafe { System.alloc(Layout::from_size_align_unchecked(CLASSES[c], ALIGN)) }
        } else {
            block
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let Some(c) = class(layout) else {
            // SAFETY: allocated by `System` with this layout.
            return unsafe { System.dealloc(ptr, layout) };
        };
        shard().with(|heads| {
            // SAFETY: a block of this class no longer used, at least a pointer long.
            unsafe { ptr.cast::<*mut u8>().write_unaligned(heads[c]) };
            heads[c] = ptr;
        });
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: the caller's layout, with the new size.
        let new_layout = unsafe { Layout::from_size_align_unchecked(new_size, layout.align()) };
        match (class(layout), class(new_layout)) {
            // SAFETY: allocated by `System` with this layout.
            (None, None) => unsafe { System.realloc(ptr, layout, new_size) },
            // Its block holds the new size too.
            (Some(a), Some(b)) if a == b => ptr,
            _ => {
                // SAFETY: a valid layout.
                let new = unsafe { self.alloc(new_layout) };
                if !new.is_null() {
                    // SAFETY: both blocks hold the smaller size.
                    unsafe {
                        std::ptr::copy_nonoverlapping(ptr, new, layout.size().min(new_size));
                        self.dealloc(ptr, layout);
                    }
                }
                new
            }
        }
    }
}

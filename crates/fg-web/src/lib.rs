//! The Factoruide app's backend for the browser: what the desktop app's backend does, on files
//! the page keeps in the browser's storage, with the addon reached through texts copied in and
//! out of the game instead of its folder.
//!
//! The page (web/src/host) runs the module in Web Workers sharing its memory: a call
//! (`fg_call`) is queued and answered (`host::reply`) by a thread of its own, so that a long
//! computation never holds the others up, and the planner's threads are threads.

#[cfg(any(target_os = "wasi", unix))]
#[expect(unsafe_code, reason = "an allocator, over wasi-libc's malloc")]
pub mod alloc;
mod app;
pub mod host;
mod store;
mod zip;

use std::collections::VecDeque;
use std::sync::{Condvar, Mutex};

#[cfg(target_os = "wasi")]
#[global_allocator]
static ALLOCATOR: alloc::Sharded = alloc::Sharded;

mod embedded {
    include!(concat!(env!("OUT_DIR"), "/embedded.rs"));
}

/// Calls waiting for a thread (id, request), and how many threads wait for one.
type Calls = (VecDeque<(u32, Vec<u8>)>, usize);

struct Queue {
    calls: Mutex<Calls>,
    ready: Condvar,
}

static QUEUE: Queue = Queue {
    calls: Mutex::new((VecDeque::new(), 0)),
    ready: Condvar::new(),
};

/// Queue a call (`{"cmd": name, "args": {...}}`) answered as `id`; a thread is started when
/// none waits.
pub fn call(id: u32, request: Vec<u8>) {
    let mut calls = QUEUE.calls.lock().unwrap();
    calls.0.push_back((id, request));
    if calls.1 == 0 {
        std::thread::spawn(serve);
    } else {
        QUEUE.ready.notify_one();
    }
}

/// A thread answering calls, one after the other, for good. One woken for a call that finds
/// more waiting with no other thread free starts one for them.
fn serve() {
    loop {
        let (id, request) = {
            let mut calls = QUEUE.calls.lock().unwrap();
            loop {
                if let Some(call) = calls.0.pop_front() {
                    if !calls.0.is_empty() && calls.1 == 0 {
                        std::thread::spawn(serve);
                    }
                    break call;
                }
                calls.1 += 1;
                calls = QUEUE.ready.wait(calls).unwrap();
                calls.1 -= 1;
            }
        };
        host::reply(id, &app::dispatch(&request));
    }
}

/// Take back a buffer of `fg_alloc`.
///
/// # Safety
/// `ptr` and `len` must be those of a buffer of `fg_alloc` not taken yet.
#[cfg(target_os = "wasi")]
#[expect(unsafe_code, reason = "buffers the page filled")]
unsafe fn take(ptr: *mut u8, len: usize) -> Vec<u8> {
    // SAFETY: see above; `fg_alloc` made the capacity `len.max(1)`.
    unsafe { Vec::from_raw_parts(ptr, len, len.max(1)) }
}

#[cfg(target_os = "wasi")]
#[expect(unsafe_code, reason = "the functions the page calls")]
mod exports {
    /// A buffer of `len` bytes for the page to fill; whoever it is handed to frees it.
    #[unsafe(no_mangle)]
    pub extern "C" fn fg_alloc(len: usize) -> *mut u8 {
        let mut v = std::mem::ManuallyDrop::new(Vec::<u8>::with_capacity(len.max(1)));
        // SAFETY: the capacity is at least `len`; the page writes every byte.
        unsafe { v.set_len(len) };
        v.as_mut_ptr()
    }

    use super::take;

    /// Free a buffer of `fg_alloc` the page did not hand over.
    #[unsafe(no_mangle)]
    pub extern "C" fn fg_free(ptr: *mut u8, len: usize) {
        // SAFETY: the page gives back its own buffer.
        drop(unsafe { take(ptr, len) });
    }

    /// A file saved by the page (before the first call).
    #[unsafe(no_mangle)]
    pub extern "C" fn fg_load(path: *mut u8, path_len: usize, data: *mut u8, data_len: usize) {
        // SAFETY: buffers of `fg_alloc`, handed over.
        let (path, data) = unsafe { (take(path, path_len), take(data, data_len)) };
        super::store::load(&String::from_utf8_lossy(&path), data);
    }

    /// Grow the memory now, before any thread starts, up to `max` bytes (in blocks of 256 MiB,
    /// as many as the browser gives): the allocator keeps it. Returns the bytes reserved.
    ///
    /// Memory grown while threads run made some of them trap: the browser checks the copies of
    /// memory (`memory.copy`) against the size each thread last saw, which lags behind for a
    /// moment after another thread grew it. The pages are only reserved: they take memory once
    /// written.
    #[unsafe(no_mangle)]
    pub extern "C" fn fg_reserve(max: usize) -> usize {
        const BLOCK: usize = 256 << 20;
        let layout = std::alloc::Layout::from_size_align(BLOCK, 16).expect("a valid layout");
        let mut blocks = Vec::new();
        while blocks.len() * BLOCK + BLOCK <= max {
            // SAFETY: a valid layout; every block is freed below.
            let block = unsafe { std::alloc::alloc(layout) };
            if block.is_null() {
                break;
            }
            blocks.push(block);
        }
        let reserved = blocks.len() * BLOCK;
        for block in blocks {
            // SAFETY: allocated above with this layout.
            unsafe { std::alloc::dealloc(block, layout) };
        }
        reserved
    }

    /// A call from the page (a JSON text in a buffer of `fg_alloc`, handed over).
    #[unsafe(no_mangle)]
    pub extern "C" fn fg_call(id: u32, request: *mut u8, len: usize) {
        // SAFETY: a buffer of `fg_alloc`, handed over.
        super::call(id, unsafe { take(request, len) });
    }
}

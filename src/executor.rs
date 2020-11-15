// Pasts
//
// Copyright (c) 2019-2020 Jeron Aldaron Lau
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// https://apache.org/licenses/LICENSE-2.0>, or the Zlib License, <LICENSE-ZLIB
// or http://opensource.org/licenses/Zlib>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
};

#[cfg(all(feature = "std", not(target_arch = "wasm32")))]
use std::{
    cell::RefCell,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Condvar, Mutex,
    },
};

#[cfg(any(target_arch = "wasm32", not(feature = "std")))]
use alloc::{boxed::Box, vec::Vec};
#[cfg(any(target_arch = "wasm32", not(feature = "std")))]
use core::{any::Any, cell::RefCell, marker::PhantomData};

// Either a Future or Output or Empty
#[cfg(any(target_arch = "wasm32", not(feature = "std")))]
enum Task {
    Future(Pin<Box<dyn Future<Output = ()>>>),
    Output(Box<dyn Any>),
    Empty,
}

#[cfg(any(target_arch = "wasm32", not(feature = "std")))]
impl Task {
    fn take(&mut self) -> Task {
        let mut output = Task::Empty;
        core::mem::swap(&mut output, self);
        output
    }
}

// Executor data.
struct Exec {
    #[cfg(all(feature = "std", not(target_arch = "wasm32")))]
    // The thread-safe waking mechanism: part 1
    mutex: Mutex<()>,

    #[cfg(all(feature = "std", not(target_arch = "wasm32")))]
    // The thread-safe waking mechanism: part 2
    cvar: Condvar,

    #[cfg(all(feature = "std", not(target_arch = "wasm32")))]
    // Flag set to verify `Condvar` actually woke the executor.
    state: AtomicBool,

    #[cfg(any(target_arch = "wasm32", not(feature = "std")))]
    // Pinned future.
    tasks: RefCell<Vec<Task>>,
}

impl Exec {
    #[cfg(all(feature = "std", not(target_arch = "wasm32")))]
    fn new() -> Self {
        Self {
            mutex: Mutex::new(()),
            cvar: Condvar::new(),
            state: AtomicBool::new(true),
        }
    }

    #[cfg(any(target_arch = "wasm32", not(feature = "std")))]
    fn new() -> Self {
        Self {
            tasks: RefCell::new(Vec::new()),
        }
    }

    #[cfg(all(feature = "std", not(target_arch = "wasm32")))]
    fn wake(&self) {
        // Wake the task running on a separate thread via CondVar
        if !self.state.compare_and_swap(false, true, Ordering::SeqCst) {
            // We notify the condvar that the value has changed.
            self.cvar.notify_one();
        }
    }

    #[cfg(any(target_arch = "wasm32", not(feature = "std")))]
    fn wake(&self) {
        // Wake the task running on this thread - one pass through executor.

        // Get a waker and context for this executor.
        let waker = waker(self);
        let mut cx = Context::from_waker(&waker);

        // Run through task queue
        let tasks_len = self.tasks.borrow().len();
        for task_id in 0..tasks_len {
            let task = { self.tasks.borrow_mut()[task_id].take() };
            if let Task::Future(f) = task {
                let mut f = f;
                if f.as_mut().poll(&mut cx).is_pending() {
                    self.tasks.borrow_mut()[task_id] = Task::Future(f);
                }
            }
        }
    }

    #[cfg(all(feature = "std", not(target_arch = "wasm32")))]
    #[allow(unsafe_code)]
    fn execute<T, F: Future<Output = T>>(&mut self, mut f: F) -> T {
        // Unsafe: f can't move after this, because it is shadowed
        let mut f = unsafe { Pin::new_unchecked(&mut f) };
        // Get a waker and context for this executor.
        let waker = waker(self);
        let context = &mut Context::from_waker(&waker);
        // Run Future to completion.
        loop {
            // Exit with future output, on future completion, otherwise…
            if let Poll::Ready(value) = f.as_mut().poll(context) {
                break value;
            }
            // Put the thread to sleep until wake() is called.
            let mut guard = self.mutex.lock().unwrap();
            while !self.state.compare_and_swap(true, false, Ordering::SeqCst) {
                guard = self.cvar.wait(guard).unwrap();
            }
        }
    }

    // Find an open index in the tasks array.
    #[cfg(any(target_arch = "wasm32", not(feature = "std")))]
    fn find_handle(&mut self) -> u32 {
        for (id, task) in self.tasks.borrow().iter().enumerate() {
            match task {
                Task::Empty => return id as u32,
                _ => { /* continue */ }
            }
        }
        self.tasks.borrow().len() as u32
    }

    #[cfg(any(target_arch = "wasm32", not(feature = "std")))]
    fn execute<F: Future<Output = ()>>(&mut self, handle: u32, f: F)
    where
        F: 'static,
    {
        // Add to task queue
        {
            let mut tasks = self.tasks.borrow_mut();
            tasks.resize_with(handle as usize + 1, || Task::Empty);
            tasks[handle as usize] = Task::Future(Box::pin(f));
        };
        // Begin Executor
        self.wake();
    }
}

// When the std library is available, use TLS so that multiple threads can
// lazily initialize an executor.
#[cfg(all(feature = "std"))]
thread_local! {
    static EXEC: RefCell<Exec> = RefCell::new(Exec::new());
}

// Without std, implement for a single thread.
#[cfg(not(feature = "std"))]
static mut EXEC: Option<Exec> = None;

/// Execute a future by spawning an asynchronous task.
///
/// On multi-threaded systems, this will start a new thread.  Similar to
/// `futures::executor::block_on()`, except that doesn't block.  Similar to
/// `std::thread::spawn()`, except that tasks don't detach, and will join on
/// `Drop` (except when the **std** feature is not enabled, where it is expected
/// that you enter a "sleep" state).
///
/// # Example
/// ```rust
/// async fn async_main() {
///     /* your code here */
/// }
///
/// pasts::spawn(async_main);
/// ```
pub fn spawn<T, F: Future<Output = T>, G: Fn() -> F>(g: G) -> JoinHandle<T>
where
    T: 'static + Send + Unpin,
    G: 'static + Send,
{
    // Can start tasks on their own threads.
    #[cfg(all(feature = "std", not(target_arch = "wasm32")))]
    {
        let waker = Arc::new(Mutex::new((None, None)));
        JoinHandle {
            waker: waker.clone(),
            handle: Some(std::thread::spawn(move || {
                let output = EXEC.with(|exec| exec.borrow_mut().execute(g()));
                let mut waker = waker.lock().unwrap();
                waker.0 = Some(output);
                if let Some(waker) = waker.1.take() {
                    waker.wake();
                }
            })),
        }
    }

    // Can allocate task queue.
    #[cfg(any(target_arch = "wasm32", not(feature = "std")))]
    #[allow(unsafe_code)]
    {
        JoinHandle {
            handle: unsafe {
                let exec = if let Some(ref mut exec) = EXEC {
                    exec
                } else {
                    EXEC = Some(Exec::new());
                    EXEC.as_mut().unwrap()
                };
                let handle = exec.find_handle();
                exec.execute(handle, async move {
                    let output = g().await;
                    let exec = EXEC.as_mut().unwrap();
                    let mut tasks = exec.tasks.borrow_mut();
                    let task = tasks.get_mut(handle as usize).unwrap();
                    *task = Task::Output(Box::new(output));
                });
                handle
            },
            _phantom: PhantomData,
        }
    }
}

/// An owned permission to join on a task (`.await` on its termination).
#[derive(Debug)]
pub struct JoinHandle<T>
where
    T: Unpin,
{
    #[cfg(all(feature = "std", not(target_arch = "wasm32")))]
    waker: Arc<Mutex<(Option<T>, Option<Waker>)>>,

    #[cfg(all(feature = "std", not(target_arch = "wasm32")))]
    handle: Option<std::thread::JoinHandle<()>>,

    #[cfg(any(target_arch = "wasm32", not(feature = "std")))]
    handle: u32,

    #[cfg(any(target_arch = "wasm32", not(feature = "std")))]
    _phantom: PhantomData<T>,
}

#[cfg(all(feature = "std", not(target_arch = "wasm32")))]
impl<T: Unpin> Drop for JoinHandle<T> {
    fn drop(&mut self) {
        self.handle.take().unwrap().join().unwrap();
    }
}

impl<T: Unpin + 'static> Future for JoinHandle<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<T> {
        #[cfg(all(feature = "std", not(target_arch = "wasm32")))]
        {
            let mut waker = self.waker.lock().unwrap();
            if let Some(output) = waker.0.take() {
                Poll::Ready(output)
            } else {
                waker.1 = Some(_cx.waker().clone());
                Poll::Pending
            }
        }

        #[cfg(any(target_arch = "wasm32", not(feature = "std")))]
        #[allow(unsafe_code)]
        unsafe {
            let task = {
                EXEC.as_mut().unwrap().tasks.borrow_mut()[self.handle as usize]
                    .take()
            };
            if let Task::Output(output) = task {
                let mut out = core::mem::MaybeUninit::uninit();
                core::ptr::copy_nonoverlapping(
                    output.downcast_ref().unwrap(),
                    out.as_mut_ptr(),
                    1,
                );
                Poll::Ready(out.assume_init())
            } else {
                Poll::Pending
            }
        }
    }
}

// Safe wrapper to create a `Waker`.
#[inline]
#[allow(unsafe_code)]
fn waker(exec: *const Exec) -> Waker {
    const RWVT: RawWakerVTable = RawWakerVTable::new(clone, wake, wake, drop);

    #[inline]
    unsafe fn clone(data: *const ()) -> RawWaker {
        RawWaker::new(data, &RWVT)
    }
    #[inline]
    unsafe fn wake(data: *const ()) {
        let exec: *const Exec = data.cast();
        (*exec).wake();
    }
    #[inline]
    unsafe fn drop(_: *const ()) {}

    unsafe { Waker::from_raw(RawWaker::new(exec.cast(), &RWVT)) }
}

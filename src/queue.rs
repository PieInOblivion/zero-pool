use crate::padded_type::PaddedType;
use crate::task_batch::TaskBatch;
use crate::{TaskFnPointer, TaskFuture, TaskParamPointer};
use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU8, AtomicUsize, Ordering};
use std::thread::{self, Thread};

const NOT_IN_CRITICAL: usize = usize::MAX;
const EPOCH_MASK: usize = usize::MAX >> 1; // use only lower bits for epoch

pub struct Queue {
    head: PaddedType<AtomicPtr<TaskBatch>>,
    tail: PaddedType<AtomicPtr<TaskBatch>>,
    reclaim_counter: PaddedType<AtomicU8>,
    oldest: PaddedType<AtomicPtr<TaskBatch>>,
    global_epoch: PaddedType<AtomicUsize>,
    local_epochs: Box<[PaddedType<AtomicUsize>]>,
    threads: Box<[UnsafeCell<MaybeUninit<Thread>>]>,
    shutdown: AtomicBool,
}

unsafe impl Sync for Queue {}

impl Queue {
    pub fn new(worker_count: usize) -> Self {
        fn noop(_: TaskParamPointer) {}
        let anchor_node = Box::into_raw(Box::new(TaskBatch::new::<u8>(
            noop,
            &[],
            TaskFuture::new(0),
        )));

        let mut epochs = Vec::with_capacity(worker_count);
        let mut t = Vec::with_capacity(worker_count);

        for _ in 0..worker_count {
            epochs.push(PaddedType::new(AtomicUsize::new(NOT_IN_CRITICAL)));
            t.push(UnsafeCell::new(MaybeUninit::uninit()));
        }

        Queue {
            head: PaddedType::new(AtomicPtr::new(anchor_node)),
            tail: PaddedType::new(AtomicPtr::new(anchor_node)),
            reclaim_counter: PaddedType::new(AtomicU8::new(0)),
            oldest: PaddedType::new(AtomicPtr::new(anchor_node)),
            global_epoch: PaddedType::new(AtomicUsize::new(0)),
            local_epochs: epochs.into_boxed_slice(),
            threads: t.into_boxed_slice(),
            shutdown: AtomicBool::new(false),
        }
    }

    pub fn push_task_batch<T>(&self, task_fn: fn(&T), params: &[T]) -> TaskFuture {
        if params.is_empty() {
            return TaskFuture::new(0);
        }

        let future = TaskFuture::new(params.len());

        let raw_fn: TaskFnPointer = unsafe { std::mem::transmute(task_fn) };
        let new_batch = Box::into_raw(Box::new(TaskBatch::new(raw_fn, params, future.clone())));

        let prev_tail = self.tail.swap(new_batch, Ordering::Release);
        unsafe {
            (*prev_tail).next.store(new_batch, Ordering::Release);
        }

        self.notify(params.len());
        future
    }

    pub fn update_epoch(&self, worker_id: usize) {
        let epoch = self.global_epoch.load(Ordering::Acquire) & EPOCH_MASK;
        // SeqCst acts as a full barrier to publish epoch before touching queue nodes,
        // preventing reclamation races on weak memory models
        self.local_epochs[worker_id].store(epoch, Ordering::SeqCst);
    }

    pub fn exit_epoch(&self, worker_id: usize) {
        self.local_epochs[worker_id].store(NOT_IN_CRITICAL, Ordering::Release);
    }

    pub fn get_next_batch(&self) -> Option<(&TaskBatch, TaskParamPointer, &TaskFuture)> {
        let mut current = self.head.load(Ordering::Acquire);

        loop {
            let batch = unsafe { &*current };

            if let Some(param) = batch.claim_next_param() {
                return Some((batch, param, &batch.future));
            }

            let next = batch.next.load(Ordering::Acquire);
            if next.is_null() {
                return None;
            }

            // try to advance head, but continue regardless
            match self.head.compare_exchange_weak(
                current,
                next,
                Ordering::Release,
                Ordering::Relaxed,
            ) {
                Ok(_) => current = next,
                Err(new_head) => current = new_head,
            }
        }
    }

    pub fn notify(&self, mut count: usize) {
        let num_workers = self.threads.len();
        count = count.min(num_workers);

        for i in 0..num_workers {
            if self.local_epochs[i].load(Ordering::Acquire) == NOT_IN_CRITICAL {
                unsafe {
                    (*self.threads[i].get()).assume_init_ref().unpark();
                    count -= 1;
                    if count == 0 {
                        break;
                    }
                }
            }
        }
    }

    pub fn register_worker_thread(&self, worker_id: usize) {
        unsafe {
            (*self.threads[worker_id].get()).write(thread::current());
        }
    }

    // wait until work is available or shutdown
    pub fn wait_for_signal(&self) {
        while !self.has_tasks() && !self.is_shutdown() {
            thread::park();
        }
    }

    fn can_reclaim(&self, reclaim_epoch: usize) -> bool {
        for local_epoch in self.local_epochs.iter() {
            let e = local_epoch.load(Ordering::Acquire);
            if e == NOT_IN_CRITICAL {
                continue;
            }

            // handle wraparound: compute distance wrapping around
            let distance = reclaim_epoch.wrapping_sub(e) & EPOCH_MASK;

            // if distance is small (<= EPOCH_MASK/2), worker is still on old epoch
            // this handles wraparound correctly
            if distance <= (EPOCH_MASK / 2) {
                return false;
            }
        }
        true
    }

    pub fn should_reclaim(&self) -> bool {
        // throttle reclamation: only run every 256 completed batches
        self.reclaim_counter.fetch_add(1, Ordering::Relaxed) == u8::MAX
    }

    pub fn reclaim(&self) {
        let head = self.head.load(Ordering::Acquire);
        let mut current = self.oldest.load(Ordering::Acquire);

        // quick check: is there anything to reclaim?
        let mut next = unsafe { (*current).next.load(Ordering::Acquire) };
        if next.is_null() || next == head {
            return;
        }

        // advance global epoch once for this reclamation cycle and mask it
        let reclaim_epoch = self.global_epoch.fetch_add(1, Ordering::Release) & EPOCH_MASK;

        // check if safe to reclaim anything
        if !self.can_reclaim(reclaim_epoch) {
            return;
        }

        // now do the actual reclamation
        while !next.is_null() && next != head {
            // try to advance oldest pointer
            match self.oldest.compare_exchange_weak(
                current,
                next,
                Ordering::Release,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    // we own current, safe to free
                    unsafe { drop(Box::from_raw(current)) };
                    current = next;
                    next = unsafe { (*current).next.load(Ordering::Acquire) };
                }
                Err(new_oldest) => {
                    current = new_oldest;
                    next = unsafe { (*current).next.load(Ordering::Acquire) };
                }
            }
        }
    }

    pub fn is_shutdown(&self) -> bool {
        self.shutdown.load(Ordering::Acquire)
    }

    pub fn has_tasks(&self) -> bool {
        let tail = self.tail.load(Ordering::Acquire);
        unsafe { (&*tail).has_unclaimed_tasks() }
    }

    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::Release);
        self.notify(usize::MAX);
    }
}

impl Drop for Queue {
    fn drop(&mut self) {
        let mut current = self.oldest.load(Ordering::Relaxed);
        while !current.is_null() {
            let batch = unsafe { Box::from_raw(current) };
            current = batch.next.load(Ordering::Relaxed);
        }
    }
}

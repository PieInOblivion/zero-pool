use std::{ptr::NonNull, time::Duration};

use crate::task_batch::TaskBatch;

pub struct TaskFuture {
    task: NonNull<TaskBatch>,
}

impl TaskFuture {
    pub(crate) fn from_batch(task: *mut TaskBatch) -> Self {
        TaskFuture {
            task: unsafe { NonNull::new_unchecked(task) },
        }
    }
    /// Check if all tasks are complete without blocking
    ///
    /// Returns `true` if all tasks have finished execution.
    /// This is a non-blocking operation using atomic loads.
    pub fn is_complete(&self) -> bool {
        unsafe { self.task.as_ref().is_complete() }
    }

    /// Wait for all tasks to complete
    ///
    /// First checks completion with an atomic load; if incomplete, parks the thread that sent the work.
    pub fn wait(&self) {
        unsafe { self.task.as_ref().wait() };
    }

    /// Wait for all tasks to complete with a timeout
    ///
    /// First checks completion with an atomic load; if incomplete, parks the thread that sent the work.
    /// Returns `true` if all tasks completed within the timeout,
    /// `false` if the timeout was reached first.
    pub fn wait_timeout(&self, timeout: Duration) -> bool {
        unsafe { self.task.as_ref().wait_timeout(timeout) }
    }
}

impl Clone for TaskFuture {
    fn clone(&self) -> Self {
        unsafe {
            self.task.as_ref().viewers_increment();
        }

        TaskFuture { task: self.task }
    }
}

impl Drop for TaskFuture {
    fn drop(&mut self) {
        unsafe {
            TaskBatch::release_ptr(self.task.as_ptr());
        }
    }
}

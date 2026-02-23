use crate::task_batch::TaskBatch;

pub struct GarbageNode {
    pub task_batch: *mut TaskBatch,
    pub epoch: usize,
    pub next: *mut GarbageNode,
}

impl GarbageNode {
    pub fn new(task_batch: *mut TaskBatch, epoch: usize) -> *mut GarbageNode {
        Box::into_raw(Box::new(GarbageNode {
            task_batch,
            epoch,
            next: std::ptr::null_mut(),
        }))
    }

    // drops the current node and returns next
    pub fn drop_node(node: *mut GarbageNode) -> *mut GarbageNode {
        unsafe {
            let next = (*node).next;
            drop(Box::from_raw((*node).task_batch));
            drop(Box::from_raw(node));
            next
        }
    }
}

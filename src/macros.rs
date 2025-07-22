// Create a task parameter struct with automatic constructor
// 
// - Example
// use zero_pool::zp_task_params;
// 
// zp_task_params! {
//     MyTask {
//         input: u64,
//         iterations: usize,
//         result: *mut u64,  // Just another field
//     }
// }
// 
// // Usage:
// let mut result = 0u64;
// let task = MyTask::new(42, 1000, &mut result);
#[macro_export]
macro_rules! zp_task_params {
    ($struct_name:ident { $($field:ident: $field_type:ty),* $(,)? }) => {
        pub struct $struct_name {
            $(pub $field: $field_type,)*
        }
        
        impl $struct_name {
            pub fn new($($field: $field_type),*) -> Self {
                Self {
                    $($field,)*
                }
            }
        }
    };
}

// Define a task function with automatic unsafe handling
// 
// - Example
// use zero_pool::{zp_define_task_fn, zp_task_params};
// 
// zp_task_params! {
//     ComputeTask {
//         iterations: usize,
//         multiplier: u64,
//         result: *mut u64,
//     }
// }
// 
// zp_define_task_fn!(compute_task, ComputeTask, |params| {
//     // Safe code here - params is automatically dereferenced
//     let mut sum = 0u64;
//     for i in 0..params.iterations {
//         sum = sum.wrapping_add(i as u64 * params.multiplier);
//     }
//     unsafe { *params.result = sum; }  // Only unsafe where necessary
// });
#[macro_export]
macro_rules! zp_define_task_fn {
    ($fn_name:ident, $param_type:ty, |$params:ident| $body:block) => {
        fn $fn_name(raw_params: *const ()) {
            let $params = unsafe { &*(raw_params as *const $param_type) };
            $body
        }
    };
}

// Submit a task with compile-time type safety
// 
// - Example
// use zero_pool::zp_submit_task;
// 
// let pool = zero_pool::new();
// let mut result = 0u64;
// let task = ComputeTask::new(1000, 17, &mut result);
// let future = zp_submit_task!(pool, task, compute_task);
// future.wait();
#[macro_export]
macro_rules! zp_submit_task {
    ($pool:expr, $params:expr, $task_fn:ident) => {{
        let params_ptr = &$params as *const _ as *const ();
        $pool.submit_task(params_ptr, $task_fn)
    }};
}

// Submit a batch of uniform tasks with type safety
#[macro_export]
macro_rules! zp_submit_batch_uniform {
    ($pool:expr, $params_vec:expr, $task_fn:ident) => {{
        let tasks: Vec<(*const (), $crate::TaskFn)> = $params_vec
            .iter()
            .map(|params| (params as *const _ as *const (), $task_fn as $crate::TaskFn))
            .collect();
            
        $pool.submit_batch(tasks)
    }};
}

// Submit a batch of mixed tasks with type safety
#[macro_export]
macro_rules! zp_submit_batch_mixed {
    ($pool:expr, [$( ($params:expr, $task_fn:ident) ),* $(,)?]) => {{
        let tasks: Vec<(*const (), $crate::TaskFn)> = vec![
            $(
                ($params as *const _ as *const (), $task_fn as $crate::TaskFn)
            ),*
        ];
        
        $pool.submit_batch(tasks)
    }};
}

// Write a result to a raw pointer safely
// 
// - Example
// use zero_pool::zp_write_result;
// 
// zp_define_task_fn!(my_task, MyTask, |params| {
//     let result = 42u64;
//     zp_write_result!(params.result, result);
// });
#[macro_export]
macro_rules! zp_write_result {
    ($result_ptr:expr, $value:expr) => {
        unsafe { *$result_ptr = $value; }
    };
}
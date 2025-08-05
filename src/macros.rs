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
// - Usage:
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
//     unsafe { *params.result = sum; }
// });
#[macro_export]
macro_rules! zp_define_task_fn {
    ($fn_name:ident, $param_type:ty, |$params:ident| $body:block) => {
        fn $fn_name(raw_params: $crate::TaskParamPointer) {
            let $params = unsafe { &*(raw_params as *const $param_type) };
            $body
        }
    };
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
macro_rules! zp_write {
    ($result_ptr:expr, $value:expr) => {
        unsafe {
            *$result_ptr = $value;
        }
    };
}

// Write a value to a specific index in a Vec or array via raw pointer
//
// - Example
// use zero_pool::zp_write_indexed;
//
// zp_task_params! {
//     BatchTask {
//         index: usize,
//         results: *mut Vec<u64>,
//     }
// }
//
// zp_define_task_fn!(batch_task, BatchTask, |params| {
//     let sum = 42u64;
//     zp_write_indexed!(params.results, params.index, sum);
// });
#[macro_export]
macro_rules! zp_write_indexed {
    ($collection_ptr:expr, $index:expr, $value:expr) => {
        unsafe {
            (&mut (*$collection_ptr))[$index] = $value;
        }
    };
}

// Submit a batch of mixed tasks with type safety
#[macro_export]
macro_rules! zp_submit_batch_mixed {
    ($pool:expr, [$( ($params:expr, $task_fn:ident) ),* $(,)?]) => {{
        let tasks: Vec<$crate::WorkItem> = vec![
            $(
                ($task_fn as $crate::TaskFnPointer, $params as *const _ as $crate::TaskParamPointer)
            ),*
        ];

        $pool.submit_raw_task_batch(&tasks)
    }};
}

// Convert a vector of (task_fn, params) tuples to Vec<(TaskFnPointer, TaskParamPointer)>
// Works with mixed parameter types
#[macro_export]
macro_rules! zp_mixed_tasks_to_pointers {
    ($tasks_vec:expr) => {{
        $tasks_vec
            .iter()
            .map(|(task_fn, params)| {
                (
                    *task_fn as $crate::TaskFnPointer,
                    params as *const _ as $crate::TaskParamPointer,
                )
            })
            .collect::<Vec<$crate::WorkItem>>()
    }};
}

/// Define a task function with automatic parameter dereferencing
///
/// This macro creates a task function that safely dereferences the raw
/// parameter pointer to the specified type, allowing safe access to fields.
///
/// # Examples
/// ```rust
/// use zero_pool::{zp_define_task_fn, zp_write};
///
/// // Define your task parameter struct
/// struct ComputeTaskStruct { iterations: usize, result: *mut u64 }
///
/// zp_define_task_fn!(compute_task, ComputeTaskStruct, |params| {
///     let mut sum = 0u64;
///     for i in 0..params.iterations {
///         sum = sum.wrapping_add(i as u64);
///     }
///     zp_write!(params.result, sum);
/// });
/// ```
#[macro_export]
macro_rules! zp_define_task_fn {
    ($fn_name:ident, $param_type:ty, |$params:ident| $body:block) => {
        #[inline(always)]
        fn $fn_name(raw_params: $crate::TaskParamPointer) {
            let $params = unsafe { &*(raw_params as *const $param_type) };
            $body
        }
    };
}

/// Write a result to a raw pointer (eliminates explicit unsafe blocks)
///
/// This macro wraps the unsafe pointer dereference, making task code cleaner.
///
/// # Examples
/// ```rust
/// use zero_pool::{zp_write, zp_define_task_fn};
///
/// struct MyTaskStruct { value: u64, result: *mut u64 }
///
/// zp_define_task_fn!(my_task, MyTaskStruct, |params| {
///     let result = 42u64;
///     zp_write!(params.result, result);
/// });
/// ```
#[macro_export]
macro_rules! zp_write {
    ($result_ptr:expr, $value:expr) => {
        unsafe {
            *$result_ptr = $value;
        }
    };
}

/// Write a value to a specific index in a collection via raw pointer
///
/// This macro eliminates explicit unsafe blocks when writing to indexed collections.
///
/// # Examples
/// ```rust
/// use zero_pool::{zp_write_indexed, zp_define_task_fn};
///
/// struct BatchTaskStruct { index: usize, results: *mut Vec<u64> }
///
/// zp_define_task_fn!(batch_task, BatchTaskStruct, |params| {
///     let sum = 42u64;
///     zp_write_indexed!(params.results, params.index, sum);
/// });
/// ```
#[macro_export]
macro_rules! zp_write_indexed {
    ($collection_ptr:expr, $index:expr, $value:expr) => {
        unsafe {
            (&mut (*$collection_ptr))[$index] = $value;
        }
    };
}

/// Submit a batch of mixed task types with type safety
///
/// This macro allows submitting tasks of different types in a single batch,
/// handling the type erasure automatically.
///
/// # Examples
/// ```rust
/// use zero_pool::{ZeroPool, zp_submit_batch_mixed, zp_define_task_fn, zp_write};
///
/// struct Task1Struct { value: u64, result: *mut u64 }
/// struct Task2Struct { value: u64, result: *mut u64 }
///
/// zp_define_task_fn!(task1_fn, Task1Struct, |params| {
///     zp_write!(params.result, params.value * 2);
/// });
/// zp_define_task_fn!(task2_fn, Task2Struct, |params| {
///     zp_write!(params.result, params.value * 3);
/// });
///
/// let pool = ZeroPool::new();
/// let mut result1 = 0u64;
/// let mut result2 = 0u64;
/// let task1 = Task1Struct { value: 5, result: &mut result1 };
/// let task2 = Task2Struct { value: 7, result: &mut result2 };
/// let future = zp_submit_batch_mixed!(pool, [
///     (&task1, task1_fn),
///     (&task2, task2_fn),
/// ]);
/// future.wait();
/// ```
#[macro_export]
macro_rules! zp_submit_batch_mixed {
    ($pool:expr, [$( ($params:expr, $task_fn:ident) ),* $(,)?]) => {{
        let tasks: Vec<$crate::TaskItem> = vec![
            $(
                ($task_fn as $crate::TaskFnPointer, $params as *const _ as $crate::TaskParamPointer)
            ),*
        ];

        $pool.submit_raw_task_batch(&tasks)
    }};
}

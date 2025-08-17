/// Create a task parameter struct with automatic constructor
///
/// This macro generates a struct with the specified fields and a `new` method
/// that takes all fields as parameters in declaration order.
///
/// # Examples
/// ```rust
/// use zero_pool::zp_task_params;
///
/// zp_task_params! {
///     MyTask {
///         input: u64,
///         iterations: usize,
///         result: *mut u64,
///     }
/// }
///
/// let mut result = 0u64;
/// let task = MyTask::new(42, 1000, &mut result);
/// ```
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

/// Define a task function with automatic parameter dereferencing
///
/// This macro creates a task function that safely dereferences the raw
/// parameter pointer to the specified type, allowing safe access to fields.
///
/// # Examples
/// ```rust
/// use zero_pool::{zp_define_task_fn, zp_task_params};
///
/// zp_task_params! {
///     ComputeTask {
///         iterations: usize,
///         result: *mut u64,
///     }
/// }
///
/// zp_define_task_fn!(compute_task, ComputeTask, |params| {
///     let mut sum = 0u64;
///     for i in 0..params.iterations {
///         sum = sum.wrapping_add(i as u64);
///     }
///     unsafe { *params.result = sum; }
/// });
/// ```
#[macro_export]
macro_rules! zp_define_task_fn {
    ($fn_name:ident, $param_type:ty, |$params:ident| $body:block) => {
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
/// use zero_pool::zp_write;
///
/// zp_define_task_fn!(my_task, MyTask, |params| {
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
/// use zero_pool::zp_write_indexed;
///
/// zp_define_task_fn!(batch_task, BatchTask, |params| {
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
/// use zero_pool::zp_submit_batch_mixed;
///
/// let future = zp_submit_batch_mixed!(pool, [
///     (&task1, task1_fn),
///     (&task2, task2_fn),
/// ]);
/// future.wait();
/// ```
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

/// Convert a vector of mixed task types to work item pointers
///
/// This macro takes a vector containing (task_fn, params) tuples of potentially
/// different types and converts them to the uniform pointer format needed for
/// raw batch submission. This is useful when you have pre-built task collections
/// with mixed parameter types.
///
/// # Examples
/// ```rust
/// use zero_pool::zp_mixed_tasks_to_pointers;
///
/// let mixed_tasks = vec![
///     (task1_fn, &task1_params),
///     (task2_fn, &task2_params),
/// ];
/// let work_items = zp_mixed_tasks_to_pointers!(mixed_tasks);
/// let future = pool.submit_raw_task_batch(&work_items);
/// ```
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

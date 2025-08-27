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

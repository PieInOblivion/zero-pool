# Miri Verification

**Last Verified:** Zero-Pool v0.7.1

This directory contains integration tests verified by **Miri** (Rust's MIR interpreter) to ensure the thread pool is free of data races, deadlocks, and undefined behavior.

## Usage

Run the following command with Nightly Rust:

```bash
MIRIFLAGS="-Zmiri-tree-borrows -Zmiri-preemption-rate=0 -Zmiri-ignore-leaks" cargo +nightly miri test
```

### Flags Explained
* **`-Zmiri-tree-borrows`**: Uses the Tree Borrows aliasing model, which correctly verifies the library's safe function pointer erasure pattern.
* **`-Zmiri-preemption-rate=0`**: Forces a context switch at every possible opportunity to maximize race condition detection.
* **`-Zmiri-ignore-leaks`**: Permits the global singleton pool (`global_pool`) to persist after the global pool test finishes.

## Verification Log (No tree-borrows)
```text
MIRIFLAGS="-Zmiri-preemption-rate=0 -Zmiri-ignore-leaks" cargo +nightly miri test
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.00s
     Running unittests src/lib.rs (target/miri/x86_64-unknown-linux-gnu/debug/deps/zero_pool-fe89ad47ce96d06d)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

     Running tests/integration.rs (target/miri/x86_64-unknown-linux-gnu/debug/deps/integration-5819b51c37a85b60)

running 13 tests
test test_basic_functionality ... ok
test test_benchmark_simulation ... ok
test test_complex_workload_scaling ... ok
test test_different_worker_counts ... ok
test test_empty_batch_submission ... ok
test test_global_pool_usage ... ok
test test_massive_simple_tasks ... ok
test test_rapid_pool_creation ... ok
test test_reclaim_trigger ... ok
test test_shutdown_and_cleanup ... ok
test test_single_worker_behavior ... ok
test test_stress_rapid_batches ... ok
test test_wait_timeout ... ok

test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 31.83s
```

## Verification Log (With tree-borrows)
```text
MIRIFLAGS="-Zmiri-tree-borrows -Zmiri-preemption-rate=0 -Zmiri-ignore-leaks" cargo +nightly miri test
   Compiling zero-pool v0.6.3 (/home/lucas/Code/zero-pool)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.33s
     Running unittests src/lib.rs (target/miri/x86_64-unknown-linux-gnu/debug/deps/zero_pool-fe89ad47ce96d06d)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

     Running tests/integration.rs (target/miri/x86_64-unknown-linux-gnu/debug/deps/integration-5819b51c37a85b60)

running 13 tests
test test_basic_functionality ... ok
test test_benchmark_simulation ... ok
test test_complex_workload_scaling ... ok
test test_different_worker_counts ... ok
test test_empty_batch_submission ... ok
test test_global_pool_usage ... ok
test test_massive_simple_tasks ... ok
test test_rapid_pool_creation ... ok
test test_reclaim_trigger ... ok
test test_shutdown_and_cleanup ... ok
test test_single_worker_behavior ... ok
test test_stress_rapid_batches ... ok
test test_wait_timeout ... ok

test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 31.83s

   Doc-tests zero_pool

running 5 tests
test src/pool.rs - pool::ZeroPool::new (line 24) ... ok
test src/pool.rs - pool::ZeroPool::submit_task (line 68) ... ok
test src/lib.rs - (line 23) ... ok
test src/pool.rs - pool::ZeroPool::with_workers (line 41) ... ok
test src/pool.rs - pool::ZeroPool::submit_batch (line 98) ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 19.88s

all doctests ran in 19.89s; merged doctests compilation took 0.01s
```
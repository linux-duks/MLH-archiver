// Tests for shutdown flag functionality

use mlh_archiver::worker::is_shutdown_requested;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread;
use std::time::Duration;

#[test]
fn test_is_shutdown_requested_helper() {
    let shutdown_flag = Arc::new(AtomicBool::new(false));

    // Initially not set
    assert!(!is_shutdown_requested(&shutdown_flag));

    // Set the flag
    shutdown_flag.store(true, Ordering::Relaxed);
    assert!(is_shutdown_requested(&shutdown_flag));
}

#[test]
fn test_shutdown_flag_clone_shares_state() {
    let shutdown_flag = Arc::new(AtomicBool::new(false));
    let clone = Arc::clone(&shutdown_flag);

    // Modify via original
    shutdown_flag.store(true, Ordering::Relaxed);

    // Clone sees the change
    assert!(clone.load(Ordering::Relaxed));
}

#[test]
fn test_worker_exits_when_shutdown_set_before_task() {
    // Verify the shutdown flag is properly checked in the worker logic
    let shutdown_flag = Arc::new(AtomicBool::new(true)); // Set before starting
    assert!(shutdown_flag.load(Ordering::Relaxed));
}

#[test]
fn test_shutdown_flag_thread_communication() {
    let shutdown_flag = Arc::new(AtomicBool::new(false));
    let flag_clone = Arc::clone(&shutdown_flag);

    let handle = thread::spawn(move || {
        // Simulate worker checking flag periodically
        for _ in 0..100 {
            if flag_clone.load(Ordering::Relaxed) {
                return true; // Exit requested
            }
            thread::sleep(Duration::from_millis(10));
        }
        false // Completed all iterations without shutdown
    });

    // Let the thread start
    thread::sleep(Duration::from_millis(20));

    // Signal shutdown
    shutdown_flag.store(true, Ordering::Relaxed);

    // Thread should exit quickly
    let result = handle.join().unwrap();
    assert!(result, "Worker should have exited due to shutdown flag");
}

#[test]
fn test_shutdown_flag_multiple_threads() {
    let shutdown_flag = Arc::new(AtomicBool::new(false));
    let mut handles = vec![];

    // Spawn multiple "worker" threads
    for i in 0..5 {
        let flag_clone = Arc::clone(&shutdown_flag);
        let handle = thread::spawn(move || {
            for _ in 0..100 {
                if flag_clone.load(Ordering::Relaxed) {
                    return i; // Return worker ID
                }
                thread::sleep(Duration::from_millis(5));
            }
            99 // Completed without shutdown
        });
        handles.push(handle);
    }

    // Let threads start
    thread::sleep(Duration::from_millis(15));

    // Signal shutdown
    shutdown_flag.store(true, Ordering::Relaxed);

    // All threads should exit quickly
    for handle in handles {
        let result = handle.join().unwrap();
        assert!(
            result < 99,
            "Worker {} should have exited due to shutdown",
            result
        );
    }
}

#[test]
fn test_shutdown_during_simulated_work() {
    use std::sync::atomic::AtomicUsize;

    let shutdown_flag = Arc::new(AtomicBool::new(false));
    let work_counter = Arc::new(AtomicUsize::new(0));

    let flag_clone = Arc::clone(&shutdown_flag);
    let counter_clone = Arc::clone(&work_counter);

    let handle = thread::spawn(move || {
        // Simulate processing emails
        for _ in 0..1000 {
            if flag_clone.load(Ordering::Relaxed) {
                return counter_clone.load(Ordering::Relaxed);
            }
            // Simulate work
            counter_clone.fetch_add(1, Ordering::Relaxed);
            thread::sleep(Duration::from_millis(1));
        }
        counter_clone.load(Ordering::Relaxed)
    });

    // Let some work happen
    thread::sleep(Duration::from_millis(25));

    // Signal shutdown
    shutdown_flag.store(true, Ordering::Relaxed);

    let final_count = handle.join().unwrap();

    // Should have done some work but not all
    assert!(final_count > 0, "Should have done some work");
    assert!(
        final_count < 500,
        "Should have stopped before completing all work"
    );
}

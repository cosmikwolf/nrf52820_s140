//! Common test utilities and setup for embedded tests
//!
//! This module provides shared functionality for all defmt-test based tests:
//! - Critical section implementation
//! - Common imports
//! - Test helpers

// Re-export commonly used items for tests (except conflicting macros)
pub use defmt_rtt as _; // global logger
// Also need the same embassy dependencies as the main firmware
pub use embassy_executor as _;
// Use nrf-softdevice which provides both interrupt vectors and critical section
pub use nrf_softdevice as _;
pub use panic_probe as _; // panic handler
pub use {embassy_nrf as _, embassy_sync as _, embassy_time as _};

// Global allocator for proptest (required for alloc feature in no_std)
pub extern crate alloc;
#[allow(unused)]
pub use alloc::vec;
use core::sync::atomic::{AtomicBool, Ordering};

pub use embedded_alloc::LlffHeap as Heap;

#[global_allocator]
pub static HEAP: Heap = Heap::empty();

// Define the global allocator backing store - 8KB heap for more complex tests
pub static mut HEAP_MEM: [u8; 8192] = [0; 8192];

// Global flag to ensure heap is only initialized once
static HEAP_INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Ensure heap is initialized exactly once (thread-safe)
pub fn ensure_heap_initialized() {
    if !HEAP_INITIALIZED.swap(true, Ordering::Relaxed) {
        unsafe {
            let ptr = HEAP_MEM.as_mut_ptr();
            let len = HEAP_MEM.len();
            HEAP.init(ptr as usize, len);
        }
    }
}

/// Test helper to create test data arrays
pub fn create_test_data(size: usize, pattern: u8) -> heapless::Vec<u8, 256> {
    let mut data = heapless::Vec::new();
    for i in 0..size {
        data.push(pattern.wrapping_add(i as u8)).unwrap();
    }
    data
}

/// Test helper to compare byte arrays
pub fn arrays_equal(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }

    for (x, y) in a.iter().zip(b.iter()) {
        if x != y {
            return false;
        }
    }

    true
}

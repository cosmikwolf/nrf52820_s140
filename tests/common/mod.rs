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

/// Common bonding test teardown - cleans up all bonded devices
pub async fn bonding_teardown() {
    use nrf52820_s140_firmware::ble::bonding::{bonded_device_count, get_all_bonded_handles, remove_bonded_device};
    
    let initial_count = bonded_device_count().await;
    if initial_count > 0 {
        defmt::debug!("CLEANUP: Starting cleanup with {} bonded devices", initial_count);
    }

    // Limit iterations to prevent infinite loops
    for cleanup_iteration in 0..5 {
        let remaining = bonded_device_count().await;
        if remaining == 0 {
            break;
        }

        defmt::debug!(
            "CLEANUP: Iteration {}, {} devices remaining",
            cleanup_iteration,
            remaining
        );

        // Get all currently bonded device handles and remove them
        let bonded_handles = get_all_bonded_handles().await;
        let mut removed_this_iteration = 0;

        for handle in bonded_handles {
            if remove_bonded_device(handle).await.is_ok() {
                removed_this_iteration += 1;
                defmt::debug!("CLEANUP: Removed handle {}", handle);
            }
        }

        defmt::debug!(
            "CLEANUP: Removed {} devices in iteration {}",
            removed_this_iteration,
            cleanup_iteration
        );

        // If we didn't remove any devices but there are still some remaining, break to avoid infinite loop
        if removed_this_iteration == 0 && remaining > 0 {
            defmt::error!(
                "CLEANUP: Failed to remove any devices but {} remain - breaking to avoid infinite loop",
                remaining
            );
            break;
        }
    }

    let final_count = bonded_device_count().await;
    if final_count > 0 {
        defmt::error!("CLEANUP: Cleanup incomplete - {} devices still remain", final_count);
    }
}

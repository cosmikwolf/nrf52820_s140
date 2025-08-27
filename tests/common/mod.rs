//! Common test utilities and setup for embedded tests
//! 
//! This module provides shared functionality for all defmt-test based tests:
//! - Critical section implementation
//! - Common imports
//! - Test helpers

// Re-export commonly used items for tests (except conflicting macros)
pub use defmt::{info, warn, error};
pub use defmt_rtt as _; // global logger
pub use panic_probe as _; // panic handler

// We need to provide interrupt vectors - use the nrf52820-pac
pub use nrf52820_pac as _; 

// Provide a basic critical section implementation for single-core ARM Cortex-M
struct CriticalSection;
critical_section::set_impl!(CriticalSection);

unsafe impl critical_section::Impl for CriticalSection {
    unsafe fn acquire() -> critical_section::RawRestoreState {
        let primask = cortex_m::register::primask::read();
        cortex_m::interrupt::disable();
        primask.is_active()
    }

    unsafe fn release(token: critical_section::RawRestoreState) {
        if token {
            cortex_m::interrupt::enable();
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

/// Test helper for logging test progress
pub fn log_test_start(test_name: &str) {
    info!("🧪 Starting test: {}", test_name);
}

pub fn log_test_pass(test_name: &str) {
    info!("✅ Test passed: {}", test_name);
}

pub fn log_test_info(message: &str) {
    info!("ℹ️  {}", message);
}
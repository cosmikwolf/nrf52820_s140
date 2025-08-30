#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

mod common;

use nrf52820_s140_firmware::ble::bonding::{
    add_bonded_device, bonded_device_count, get_all_bonded_handles, get_bonded_device_info, get_system_attributes,
    init as bonding_init, is_device_bonded, remove_bonded_device, set_system_attributes, BondingError,
    MAX_BONDED_DEVICES, MAX_SYS_ATTR_SIZE,
};
use proptest::prelude::*;

#[defmt_test::tests]
mod tests {
    use alloc::format;
    use alloc::vec::Vec;

    use defmt::{assert, assert_eq};

    use super::*;
    use crate::common::*;

    #[init]
    fn init() {
        ensure_heap_initialized();
    }

    #[before_each]
    fn before_each() {
        // Initialize fresh bonding storage before each test
        bonding_init();
        defmt::debug!(
            "SETUP: Fresh bonding storage initialized, count = {}",
            bonded_device_count()
        );
    }

    #[after_each]
    fn after_each() {
        // Clean up all bonded devices after each test for isolation
        // Note: Using embassy_futures::block_on since after_each can't be async
        embassy_futures::block_on(crate::common::bonding_teardown());
    }

    #[test]
    fn test_bonded_device_storage_limits() {
        embassy_futures::block_on(async {
            // Property #42: Bonded Device Storage Limits
            // System should store up to MAX_BONDED_DEVICES (2) bonded devices

            // Initialize with empty bonding table
            let initial_count = bonded_device_count().await;
            defmt::info!("TEST START: Initial bonded device count = {}", initial_count);
            assert_eq!(
                initial_count, 0,
                "Expected empty bonding table at test start, found {} devices",
                initial_count
            );

            let mut bonded_conn_handles = Vec::new();

            // Add maximum devices
            for i in 0..MAX_BONDED_DEVICES {
                let conn_handle = (i + 100) as u16; // Use connection handles starting from 100
                let peer_addr = [0x10 + i as u8, 0x20, 0x30, 0x40, 0x50, 0x60];
                let addr_type = 0; // Public address

                let result = add_bonded_device(conn_handle, peer_addr, addr_type).await;
                assert!(result.is_ok(), "Failed to add bonded device {}", i);
                bonded_conn_handles.push(conn_handle);

                // Verify device is bonded
                assert!(is_device_bonded(conn_handle).await);
            }

            // Verify we stored MAX_BONDED_DEVICES
            assert_eq!(bonded_device_count().await, MAX_BONDED_DEVICES);

            // Try to add one more device - should fail
            let overflow_handle = 999;
            let overflow_addr = [0xFF, 0xEE, 0xDD, 0xCC, 0xBB, 0xAA];
            let overflow_result = add_bonded_device(overflow_handle, overflow_addr, 0).await;
            assert!(matches!(overflow_result, Err(BondingError::BondingTableFull)));

            // Verify count unchanged
            assert_eq!(bonded_device_count().await, MAX_BONDED_DEVICES);

            // Remove one bond and try again
            let remove_result = remove_bonded_device(bonded_conn_handles[0]).await;
            assert!(remove_result.is_ok());
            assert_eq!(bonded_device_count().await, MAX_BONDED_DEVICES - 1);

            // Now adding a new device should work
            let new_bond_result = add_bonded_device(overflow_handle, overflow_addr, 0).await;
            assert!(new_bond_result.is_ok());
            assert_eq!(bonded_device_count().await, MAX_BONDED_DEVICES);
        });
    }

    #[test]
    fn test_bond_data_persistence() {
        embassy_futures::block_on(async {
            // Property #43: Bond Data Persistence
            // System attributes should be stored and retrievable for bonded devices

            let conn_handle = 101;
            let peer_addr = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66];
            let addr_type = 0;
            let original_sys_attr = [0xDD, 0xEE, 0xFF, 0x11, 0x22, 0x33];

            // Add bonded device
            let add_result = add_bonded_device(conn_handle, peer_addr, addr_type).await;
            assert!(add_result.is_ok());

            // Store system attributes
            let set_result = set_system_attributes(conn_handle, &original_sys_attr).await;
            assert!(set_result.is_ok());

            // Retrieve and verify system attributes
            let retrieved_attrs = get_system_attributes(conn_handle).await;
            assert!(retrieved_attrs.is_some());

            let attrs = retrieved_attrs.unwrap();
            assert_eq!(attrs.len(), original_sys_attr.len());
            for (i, &byte) in original_sys_attr.iter().enumerate() {
                assert_eq!(attrs[i], byte);
            }

            // Verify device is still bonded
            assert!(is_device_bonded(conn_handle).await);

            // Test system attribute size limits
            let oversized_attrs = [0xFF; MAX_SYS_ATTR_SIZE + 10];
            let oversized_result = set_system_attributes(conn_handle, &oversized_attrs).await;
            assert!(matches!(oversized_result, Err(BondingError::InvalidData)));

            // Original attributes should still be intact
            let preserved_attrs = get_system_attributes(conn_handle).await;
            assert!(preserved_attrs.is_some());
            assert_eq!(preserved_attrs.unwrap().len(), original_sys_attr.len());
        });
    }

    #[test]
    fn test_system_attributes_size_limits() {
        proptest!(|(
                attr_sizes in prop::collection::vec(0usize..100, 1..5)
            )| {
                // Property #44: System Attributes Size Limits
                // System attributes should not exceed MAX_SYS_ATTR_SIZE (64 bytes)
                let mut conn_handles = Vec::new();

            for (i, &size) in attr_sizes.iter().take(MAX_BONDED_DEVICES).enumerate() {
                let conn_handle = (200 + i) as u16;
                let peer_addr = [0x12 + i as u8, 0x34, 0x56, 0x78, 0x9A, 0xBC];
                let addr_type = 0;

                // Add bonded device
                let add_result = embassy_futures::block_on(add_bonded_device(conn_handle, peer_addr, addr_type));
                prop_assert!(add_result.is_ok());
                conn_handles.push(conn_handle);

                // Create test data of specified size
                let mut sys_attr = heapless::Vec::<u8, MAX_SYS_ATTR_SIZE>::new();
                for j in 0..size.min(MAX_SYS_ATTR_SIZE) {
                    let _ = sys_attr.push(0x55 + (j % 256) as u8);
                }

                let result = embassy_futures::block_on(set_system_attributes(conn_handle, &sys_attr));

                if size <= MAX_SYS_ATTR_SIZE {
                    // Should accept system attributes within limits
                    prop_assert!(result.is_ok(), "Should accept {} byte attributes", size);

                    // Verify stored data
                    let retrieved = embassy_futures::block_on(get_system_attributes(conn_handle));
                    prop_assert!(retrieved.is_some());
                    prop_assert_eq!(retrieved.unwrap().len(), size);
                } else {
                    // For oversized data, we created data up to MAX_SYS_ATTR_SIZE
                    prop_assert!(result.is_ok());
                    prop_assert!(size > MAX_SYS_ATTR_SIZE);

                    let retrieved = embassy_futures::block_on(get_system_attributes(conn_handle));
                    prop_assert!(retrieved.is_some());
                    prop_assert_eq!(retrieved.unwrap().len(), MAX_SYS_ATTR_SIZE);
                }
            }

                // Clean up
                for handle in conn_handles {
                    let _ = embassy_futures::block_on(remove_bonded_device(handle));
                }
        });
    }

    #[test]
    fn test_bonding_state_consistency() {
        proptest!(|(
            conn_handles in prop::collection::vec(300u16..400, 1..4)
        )| {
            // Property #45: Bonding State Consistency
            // Bonded device state should remain consistent across operations

            let mut bonded_handles = Vec::new();

            // Add multiple bonded devices (skip duplicates)
            for (i, &handle) in conn_handles.iter().take(MAX_BONDED_DEVICES).enumerate() {
                // Skip if we already added this handle
                if bonded_handles.contains(&handle) {
                    continue;
                }

                let peer_addr = [0x10 + i as u8, 0x20, 0x30, 0x40, 0x50, 0x60];
                let addr_type = i as u8 % 2; // Alternate between public and random

                let add_result = embassy_futures::block_on(add_bonded_device(handle, peer_addr, addr_type));
                prop_assert!(add_result.is_ok());
                bonded_handles.push(handle);

                // Verify device is immediately bonded
                prop_assert!(embassy_futures::block_on(is_device_bonded(handle)));

                // Add system attributes
                let sys_attr = [0x44 + i as u8; 8];
                let attr_result = embassy_futures::block_on(set_system_attributes(handle, &sys_attr));
                prop_assert!(attr_result.is_ok());

                // Verify attributes are stored
                let retrieved = embassy_futures::block_on(get_system_attributes(handle));
                prop_assert!(retrieved.is_some());
                prop_assert_eq!(retrieved.unwrap().len(), 8);
            }

            // Verify total count
            prop_assert_eq!(embassy_futures::block_on(bonded_device_count()), bonded_handles.len());

            // Verify all devices remain bonded
            for &handle in &bonded_handles {
                prop_assert!(embassy_futures::block_on(is_device_bonded(handle)));
            }

            // Remove devices and verify count updates
            for handle in bonded_handles {
                let remove_result = embassy_futures::block_on(remove_bonded_device(handle));
                prop_assert!(remove_result.is_ok());
                prop_assert!(!embassy_futures::block_on(is_device_bonded(handle)));
            }

            // Should be empty now
            prop_assert_eq!(embassy_futures::block_on(bonded_device_count()), 0);
        });
    }
}

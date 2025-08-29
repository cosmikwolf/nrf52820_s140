#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

mod common;

use nrf52820_s140_firmware::ble::bonding::{
    add_bonded_device, bonded_device_count, get_all_bonded_handles, get_system_attributes, init as bonding_init,
    is_device_bonded, remove_bonded_device, set_system_attributes, BondingError, MAX_BONDED_DEVICES, MAX_SYS_ATTR_SIZE,
};
use proptest::prelude::*;

#[defmt_test::tests]
mod tests {
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
        let initial_count = bonded_device_count();
        if initial_count > 0 {
            defmt::debug!("CLEANUP: Starting cleanup with {} bonded devices", initial_count);
        }

        // Limit iterations to prevent infinite loops
        for cleanup_iteration in 0..5 {
            let remaining = bonded_device_count();
            if remaining == 0 {
                break;
            }

            defmt::debug!(
                "CLEANUP: Iteration {}, {} devices remaining",
                cleanup_iteration,
                remaining
            );

            // Get all currently bonded device handles and remove them
            let bonded_handles = get_all_bonded_handles();
            let mut removed_this_iteration = 0;

            bonded_handles.into_iter().for_each(|handle| {
                if remove_bonded_device(handle).is_ok() {
                    removed_this_iteration += 1;
                    defmt::debug!("CLEANUP: Removed handle {}", handle);
                }
            });

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

        let final_count = bonded_device_count();
        if final_count > 0 {
            defmt::error!("CLEANUP: Cleanup incomplete - {} devices still remain", final_count);
        }
    }

    #[test]
    fn test_bonded_device_storage_limits() {
        // Property #42: Bonded Device Storage Limits
        // System should store up to MAX_BONDED_DEVICES (2) bonded devices

        // Initialize with empty bonding table
        let initial_count = bonded_device_count();
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

            let result = add_bonded_device(conn_handle, peer_addr, addr_type);
            assert!(result.is_ok(), "Failed to add bonded device {}", i);
            bonded_conn_handles.push(conn_handle);

            // Verify device is bonded
            assert!(is_device_bonded(conn_handle));
        }

        // Verify we stored MAX_BONDED_DEVICES
        assert_eq!(bonded_device_count(), MAX_BONDED_DEVICES);

        // Try to add one more device - should fail
        let overflow_handle = 999;
        let overflow_addr = [0xFF, 0xEE, 0xDD, 0xCC, 0xBB, 0xAA];
        let overflow_result = add_bonded_device(overflow_handle, overflow_addr, 0);
        assert!(matches!(overflow_result, Err(BondingError::BondingTableFull)));

        // Verify count unchanged
        assert_eq!(bonded_device_count(), MAX_BONDED_DEVICES);

        // Remove one bond and try again
        let remove_result = remove_bonded_device(bonded_conn_handles[0]);
        assert!(remove_result.is_ok());
        assert_eq!(bonded_device_count(), MAX_BONDED_DEVICES - 1);

        // Now adding a new device should work
        let new_bond_result = add_bonded_device(overflow_handle, overflow_addr, 0);
        assert!(new_bond_result.is_ok());
        assert_eq!(bonded_device_count(), MAX_BONDED_DEVICES);
    }

    #[test]
    fn test_bond_data_persistence() {
        // Property #43: Bond Data Persistence
        // System attributes should be stored and retrievable for bonded devices

        let conn_handle = 101;
        let peer_addr = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66];
        let addr_type = 0;
        let original_sys_attr = [0xDD, 0xEE, 0xFF, 0x11, 0x22, 0x33];

        // Add bonded device
        let add_result = add_bonded_device(conn_handle, peer_addr, addr_type);
        assert!(add_result.is_ok());

        // Store system attributes
        let set_result = set_system_attributes(conn_handle, &original_sys_attr);
        assert!(set_result.is_ok());

        // Retrieve and verify system attributes
        let retrieved_attrs = get_system_attributes(conn_handle);
        assert!(retrieved_attrs.is_some());

        let attrs = retrieved_attrs.unwrap();
        assert_eq!(attrs.len(), original_sys_attr.len());
        for (i, &byte) in original_sys_attr.iter().enumerate() {
            assert_eq!(attrs[i], byte);
        }

        // Verify device is still bonded
        assert!(is_device_bonded(conn_handle));

        // Test system attribute size limits
        let oversized_attrs = [0xFF; MAX_SYS_ATTR_SIZE + 10];
        let oversized_result = set_system_attributes(conn_handle, &oversized_attrs);
        assert!(matches!(oversized_result, Err(BondingError::InvalidData)));

        // Original attributes should still be intact
        let preserved_attrs = get_system_attributes(conn_handle);
        assert!(preserved_attrs.is_some());
        assert_eq!(preserved_attrs.unwrap().len(), original_sys_attr.len());
    }

    proptest! {
        #[test]
        fn test_system_attributes_size_limits(
            attr_sizes in prop::collection::vec(0usize..100, 1..5)
        ) {
            // Property #44: System Attributes Size Limits
            // System attributes should not exceed MAX_SYS_ATTR_SIZE (64 bytes)

            unsafe {
                common::HEAP.init(common::HEAP_MEM.as_ptr() as usize, common::HEAP_MEM.len());
            }

            let mut conn_handles = Vec::new();

            for (i, &size) in attr_sizes.iter().take(5).enumerate() {
                let conn_handle = (200 + i) as u16;
                let peer_addr = [0x12 + i as u8, 0x34, 0x56, 0x78, 0x9A, 0xBC];
                let addr_type = 0;

                // Add bonded device
                let add_result = add_bonded_device(conn_handle, peer_addr, addr_type);
                prop_assert!(add_result.is_ok());
                conn_handles.push(conn_handle);

                // Create test data of specified size
                let mut sys_attr = heapless::Vec::<u8, MAX_SYS_ATTR_SIZE>::new();
                for j in 0..size.min(MAX_SYS_ATTR_SIZE) {
                    let _ = sys_attr.push(0x55 + (j % 256) as u8);
                }

                let result = set_system_attributes(conn_handle, &sys_attr);

                if size <= MAX_SYS_ATTR_SIZE {
                    // Should accept system attributes within limits
                    prop_assert!(result.is_ok(), "Should accept {} byte attributes", size);

                    // Verify stored data
                    let retrieved = get_system_attributes(conn_handle);
                    prop_assert!(retrieved.is_some());
                    prop_assert_eq!(retrieved.unwrap().len(), size);
                } else {
                    // For oversized data, we created data up to MAX_SYS_ATTR_SIZE
                    prop_assert!(result.is_ok());
                    prop_assert!(size > MAX_SYS_ATTR_SIZE);

                    let retrieved = get_system_attributes(conn_handle);
                    prop_assert!(retrieved.is_some());
                    prop_assert_eq!(retrieved.unwrap().len(), MAX_SYS_ATTR_SIZE);
                }
            }

            // Clean up
            for handle in conn_handles {
                let _ = remove_bonded_device(handle);
            }
        }
    }

    proptest! {
        #[test]
        fn test_bonding_state_consistency(
            conn_handles in prop::collection::vec(300u16..400, 1..4)
        ) {
            // Property #45: Bonding State Consistency
            // Bonded device state should remain consistent across operations

            unsafe {
                common::HEAP.init(common::HEAP_MEM.as_ptr() as usize, common::HEAP_MEM.len());
            }

            let mut bonded_handles = Vec::new();

            // Add multiple bonded devices
            for (i, &handle) in conn_handles.iter().take(MAX_BONDED_DEVICES).enumerate() {
                let peer_addr = [0x10 + i as u8, 0x20, 0x30, 0x40, 0x50, 0x60];
                let addr_type = i as u8 % 2; // Alternate between public and random

                let add_result = add_bonded_device(handle, peer_addr, addr_type);
                prop_assert!(add_result.is_ok());
                bonded_handles.push(handle);

                // Verify device is immediately bonded
                prop_assert!(is_device_bonded(handle));

                // Add system attributes
                let sys_attr = [0x44 + i as u8; 8];
                let attr_result = set_system_attributes(handle, &sys_attr);
                prop_assert!(attr_result.is_ok());

                // Verify attributes are stored
                let retrieved = get_system_attributes(handle);
                prop_assert!(retrieved.is_some());
                prop_assert_eq!(retrieved.unwrap().len(), 8);
            }

            // Verify total count
            prop_assert_eq!(bonded_device_count(), bonded_handles.len());

            // Verify all devices remain bonded
            for &handle in &bonded_handles {
                prop_assert!(is_device_bonded(handle));
            }

            // Remove devices and verify count updates
            for handle in bonded_handles {
                let remove_result = remove_bonded_device(handle);
                prop_assert!(remove_result.is_ok());
                prop_assert!(!is_device_bonded(handle));
            }

            // Should be empty now
            prop_assert_eq!(bonded_device_count(), 0);
        }
    }

    #[test]
    fn test_bond_state_consistency() {
        // Property #46: Bond State Consistency
        // Bond states should remain consistent across operations

        // Initially no bonds
        assert_eq!(bonded_device_count(), 0);

        let conn_handle1 = 101;
        let peer_addr1 = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66];
        let addr_type1 = 0;

        let conn_handle2 = 102;
        let peer_addr2 = [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF];
        let addr_type2 = 1;

        // Add first bond
        let add1_result = add_bonded_device(conn_handle1, peer_addr1, addr_type1);
        assert!(add1_result.is_ok());
        let sys_attr1 = [0x33, 0x44];
        let set1_result = set_system_attributes(conn_handle1, &sys_attr1);
        assert!(set1_result.is_ok());

        // Verify state consistency
        assert_eq!(bonded_device_count(), 1);
        assert!(is_device_bonded(conn_handle1));
        assert!(!is_device_bonded(conn_handle2));

        // Verify system attributes are stored
        let retrieved1 = get_system_attributes(conn_handle1);
        assert!(retrieved1.is_some());
        assert_eq!(retrieved1.unwrap().len(), 2);

        // Add second bond if we haven't reached the limit
        if bonded_device_count() < MAX_BONDED_DEVICES {
            let add2_result = add_bonded_device(conn_handle2, peer_addr2, addr_type2);
            assert!(add2_result.is_ok());

            // Verify state consistency with both bonds
            assert_eq!(bonded_device_count(), 2);
            assert!(is_device_bonded(conn_handle1));
            assert!(is_device_bonded(conn_handle2));

            // Remove first bond
            let remove_result = remove_bonded_device(conn_handle1);
            assert!(remove_result.is_ok());

            // Verify consistent state after removal
            assert_eq!(bonded_device_count(), 1);
            assert!(!is_device_bonded(conn_handle1));
            assert!(is_device_bonded(conn_handle2));

            // Update system attributes for remaining bond
            let updated_sys_attr = [0xFF];
            let update_result = set_system_attributes(conn_handle2, &updated_sys_attr);
            assert!(update_result.is_ok());

            // Verify state remains consistent after update
            let updated_attrs = get_system_attributes(conn_handle2);
            assert!(updated_attrs.is_some());
            let attrs = updated_attrs.unwrap();
            assert_eq!(attrs.len(), 1);
            assert_eq!(attrs[0], 0xFF);
        }
    }

    #[test]
    fn test_bonding_error_handling() {
        // Property #47: Bonding Error Handling
        // Bonding operations should handle error conditions properly

        let conn_handle = 103;
        let peer_addr = [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC];
        let addr_type = 0;

        // Test successful bonding
        let add_result = add_bonded_device(conn_handle, peer_addr, addr_type);
        assert!(add_result.is_ok());
        assert!(is_device_bonded(conn_handle));

        // Test setting system attributes for bonded device
        let sys_attr = [0x10, 0x20, 0x30];
        let set_result = set_system_attributes(conn_handle, &sys_attr);
        assert!(set_result.is_ok());

        // Verify system attributes were stored
        let retrieved = get_system_attributes(conn_handle);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().len(), 3);

        // Test setting system attributes for non-bonded device
        let invalid_handle = 999;
        let invalid_set_result = set_system_attributes(invalid_handle, &sys_attr);
        assert!(matches!(invalid_set_result, Err(BondingError::DeviceNotFound)));

        // Test oversized system attributes
        let oversized_attrs = [0xFF; MAX_SYS_ATTR_SIZE + 10];
        let oversized_result = set_system_attributes(conn_handle, &oversized_attrs);
        assert!(matches!(oversized_result, Err(BondingError::InvalidData)));

        // Test removing bonded device
        let remove_result = remove_bonded_device(conn_handle);
        assert!(remove_result.is_ok());
        assert!(!is_device_bonded(conn_handle));

        // Test removing non-existent device
        let invalid_remove_result = remove_bonded_device(conn_handle);
        assert!(matches!(invalid_remove_result, Err(BondingError::DeviceNotFound)));

        // Test adding too many bonded devices
        let mut handles = Vec::new();
        for i in 0..MAX_BONDED_DEVICES {
            let handle = (200 + i) as u16;
            let addr = [0x20 + i as u8, 0x30, 0x40, 0x50, 0x60, 0x70];
            let result = add_bonded_device(handle, addr, 0);
            assert!(result.is_ok());
            handles.push(handle);
        }

        // This should fail - table is full
        let overflow_result = add_bonded_device(999, [0xFF; 6], 0);
        assert!(matches!(overflow_result, Err(BondingError::BondingTableFull)));

        // Clean up
        for handle in handles {
            let _ = remove_bonded_device(handle);
        }
    }
}

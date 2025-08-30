#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

mod common;

use nrf52820_s140_firmware::ble::bonding::{
    add_bonded_device, bonded_device_count, get_bonded_device_info, get_system_attributes, init as bonding_init,
    is_device_bonded, remove_bonded_device, set_system_attributes, BondingError, MAX_BONDED_DEVICES, MAX_SYS_ATTR_SIZE,
};
use proptest::prelude::*;

#[defmt_test::tests]
mod tests {
    use alloc::vec::Vec;
    extern crate alloc;
    use alloc::format;

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
    }

    #[after_each]
    fn after_each() {
        // Clean up all bonded devices after each test for isolation
        embassy_futures::block_on(crate::common::bonding_teardown());
    }

    #[test]
    fn test_device_data_combinations() {
        // Property #53: Device Data Combinations
        // Test various combinations of device data (handles, addresses, types, attributes)

        proptest!(|(
            device_specs in prop::collection::vec(
                (1u16..400, (0u8..=255, 0u8..=255, 0u8..=255, 0u8..=255, 0u8..=255, 0u8..=255).prop_map(|(a,b,c,d,e,f)| [a,b,c,d,e,f]), 0u8..=2, 0usize..=MAX_SYS_ATTR_SIZE),
                1..3
            )
        )| {
            let mut bonded_devices = Vec::new();

            for (handle, addr, addr_type, sys_attr_len) in device_specs.iter().take(MAX_BONDED_DEVICES) {
                // Add bonded device
                let add_result = embassy_futures::block_on(add_bonded_device(*handle, *addr, *addr_type));
                prop_assert!(add_result.is_ok());
                bonded_devices.push((*handle, *addr, *addr_type, *sys_attr_len));

                // Create system attributes of specified length
                let mut sys_attrs = heapless::Vec::<u8, MAX_SYS_ATTR_SIZE>::new();
                for i in 0..*sys_attr_len {
                    let _ = sys_attrs.push(0xAA + (i % 256) as u8);
                }

                // Set system attributes
                let attr_result = embassy_futures::block_on(set_system_attributes(*handle, &sys_attrs));
                prop_assert!(attr_result.is_ok());

                // Verify all data integrity
                let device_info = embassy_futures::block_on(get_bonded_device_info(*handle));
                prop_assert!(device_info.is_some());

                let info = device_info.unwrap();
                prop_assert_eq!(info.conn_handle, *handle);
                prop_assert_eq!(info.peer_addr, *addr);
                prop_assert_eq!(info.addr_type, *addr_type);

                // Verify system attributes
                let retrieved_attrs = embassy_futures::block_on(get_system_attributes(*handle));
                prop_assert!(retrieved_attrs.is_some());
                let attrs = retrieved_attrs.unwrap();
                prop_assert_eq!(attrs.len(), *sys_attr_len);

                // Verify attribute data integrity
                for (i, &byte) in attrs.iter().enumerate() {
                    prop_assert_eq!(byte, 0xAA + (i % 256) as u8);
                }

                prop_assert!(embassy_futures::block_on(is_device_bonded(*handle)));
            }

            // Verify final state
            prop_assert_eq!(embassy_futures::block_on(bonded_device_count()), bonded_devices.len());

            // Clean up and verify each removal
            for (handle, _, _, _) in bonded_devices {
                let remove_result = embassy_futures::block_on(remove_bonded_device(handle));
                prop_assert!(remove_result.is_ok());
                prop_assert!(!embassy_futures::block_on(is_device_bonded(handle)));
            }

            prop_assert_eq!(embassy_futures::block_on(bonded_device_count()), 0);
        });
    }
}

#![no_std]
#![no_main]

mod common;

use nrf52820_s140_firmware::ble::gatt_state::{ModemState, StateError, AdvertisingState, ServiceType};
use nrf_softdevice::ble::Uuid;
use nrf52820_s140_firmware::commands::CommandError;

#[defmt_test::tests]
mod tests {
    use super::*;
    use crate::common::*;
    use defmt::{assert, assert_eq};

    #[test]
    fn test_modem_state_initialization() {

        let state = ModemState::new();

        // Check initial state - connection should be None
        assert!(state.get_connection().is_none());
        assert_eq!(state.get_advertising_state(), AdvertisingState::Stopped);

    }

    #[test]
    fn test_uuid_base_registration() {

        let mut state = ModemState::new();

        // Test UUID base registration
        let uuid_base = [
            0x9e, 0x73, 0x12, 0xe0, 0x23, 0x54, 0x11, 0xeb,
            0x9f, 0x10, 0xfb, 0xc3, 0x0a, 0x62, 0xcf, 0x38
        ];

        let handle = state.register_uuid_base(uuid_base).unwrap();
        assert_eq!(handle, 0); // First registration should get handle 0

        // Verify we can retrieve it
        let retrieved = state.get_uuid_base(handle);
        assert!(retrieved.is_some());
        assert!(arrays_equal(&retrieved.unwrap().base, &uuid_base));

    }

    #[test]
    fn test_max_uuid_bases() {

        let mut state = ModemState::new();

        // Register maximum number of UUID bases (4)
        for i in 0..4 {
            let mut uuid_base = [0u8; 16];
            uuid_base[15] = i; // Make each UUID unique

            let result = state.register_uuid_base(uuid_base);
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), i);
        }


        // Try to register one more - should fail
        let uuid_base = [0xFF; 16];
        let result = state.register_uuid_base(uuid_base);

        match result {
            Err(StateError::UuidBasesExhausted) => {
            },
            other => panic!("Expected UuidBasesExhausted error, got: {:?}", other),
        }

    }

    #[test]
    fn test_service_registration() {

        let mut state = ModemState::new();

        // First register a UUID base
        let uuid_base = [
            0x9e, 0x73, 0x12, 0xe0, 0x23, 0x54, 0x11, 0xeb,
            0x9f, 0x10, 0xfb, 0xc3, 0x0a, 0x62, 0xcf, 0x38
        ];
        let _base_handle = state.register_uuid_base(uuid_base).unwrap();

        // Add a service (note: using available method add_service)
        let service_handle = 1;
        let uuid = Uuid::new_16(0x1234);
        let result = state.add_service(service_handle, uuid, ServiceType::Primary);
        assert!(result.is_ok());

        // Verify we can retrieve the service
        let retrieved = state.get_service(service_handle);
        assert!(retrieved.is_some());

    }

    #[test]
    fn test_command_error_conversions() {

        // Test that various error types can be converted to CommandError
        let state_error = StateError::UuidBasesExhausted;
        let cmd_error: CommandError = state_error.into();

        match cmd_error {
            CommandError::StateError(StateError::UuidBasesExhausted) => {
            },
            other => panic!("Expected StateError(UuidBasesExhausted), got: {:?}", other),
        }

    }

    #[test]
    fn test_multiple_uuid_registrations() {

        let mut state = ModemState::new();

        // Register first UUID base
        let uuid1 = [0x01; 16];
        let handle1 = state.register_uuid_base(uuid1).unwrap();
        assert_eq!(handle1, 0);

        // Register second UUID base
        let uuid2 = [0x02; 16];
        let handle2 = state.register_uuid_base(uuid2).unwrap();
        assert_eq!(handle2, 1);

        // Verify both can be retrieved
        assert!(state.get_uuid_base(handle1).is_some());
        assert!(state.get_uuid_base(handle2).is_some());
        assert!(arrays_equal(&state.get_uuid_base(handle1).unwrap().base, &uuid1));
        assert!(arrays_equal(&state.get_uuid_base(handle2).unwrap().base, &uuid2));


    }

    #[test]
    fn test_state_edge_cases() {

        let mut state = ModemState::new();

        // Test retrieving non-existent UUID base
        assert!(state.get_uuid_base(99).is_none());

        // Test registering different UUID patterns
        let patterns = [
            [0x00; 16],  // All zeros
            [0xFF; 16],  // All ones
            [0xAA, 0x55, 0xAA, 0x55, 0xAA, 0x55, 0xAA, 0x55,
             0xAA, 0x55, 0xAA, 0x55, 0xAA, 0x55, 0xAA, 0x55], // Alternating pattern
        ];

        for (i, &pattern) in patterns.iter().enumerate() {
            let handle = state.register_uuid_base(pattern).unwrap();
            assert_eq!(handle as usize, i);
        }

    }
}

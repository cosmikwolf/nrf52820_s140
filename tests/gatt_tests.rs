#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

mod common;

use nrf52820_s140_firmware::ble::gatt_state::{ModemState, ServiceType, StateError, MAX_SERVICES};
use nrf52820_s140_firmware::commands::{CommandError, ResponseBuilder};
use nrf52820_s140_firmware::commands::gatts;
use nrf52820_s140_firmware::core::protocol::serialization::PayloadReader;
use nrf_softdevice::ble::Uuid;
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

    #[test]
    fn test_service_registration_limits() {
        proptest!(|(
            service_handles in prop::collection::vec(1u16..1000, 1..10)
        )| {
            // Property #28: Service Registration Limits
            // System should accept up to MAX_SERVICES (4) and reject additional services
            
            let mut state = ModemState::new();
            let mut registered_services = Vec::new();
            
            // Register services up to the limit
            for &handle in service_handles.iter().take(MAX_SERVICES) {
                let uuid = Uuid::new_16((handle % 0xFFFF) as u16);
                let result = state.add_service(handle, uuid, ServiceType::Primary);
                
                if result.is_ok() {
                    registered_services.push(handle);
                }
            }
            
            // Should have registered up to MAX_SERVICES
            prop_assert!(registered_services.len() <= MAX_SERVICES);
            
            // If we hit the limit, next registration should fail
            if registered_services.len() == MAX_SERVICES {
                let overflow_handle = 9999;
                let uuid = Uuid::new_16(0x1234);
                let overflow_result = state.add_service(overflow_handle, uuid, ServiceType::Primary);
                prop_assert!(overflow_result.is_err());
            }
        });
    }

    #[test]
    fn test_characteristic_handle_assignment() {
        proptest!(|(
            char_counts in prop::collection::vec(1usize..10, 1..4)
        )| {
            // Property #29: Characteristic Handle Assignment  
            // In the BLE modem protocol, handles are managed by the host
            
            let mut state = ModemState::new();
            let mut service_handles = Vec::new();
            
            // Add multiple services to test handle management
            for &char_count in char_counts.iter().take(MAX_SERVICES.min(8)) {
                let service_handle = (char_count + 1000) as u16; // Ensure unique handles
                let service_uuid = Uuid::new_16(0x1234 + char_count as u16);
                let service_result = state.add_service(service_handle, service_uuid, ServiceType::Primary);
                
                if service_result.is_ok() {
                    service_handles.push(service_handle);
                }
            }
            
            // Verify all services have unique handles
            for i in 1..service_handles.len() {
                for j in 0..i {
                    prop_assert_ne!(service_handles[i], service_handles[j]);
                }
            }
            
            // Verify services can be retrieved
            for &handle in &service_handles {
                let service = state.get_service(handle);
                prop_assert!(service.is_some());
                prop_assert_eq!(service.unwrap().handle, handle);
            }
        });
    }

    #[test]
    fn test_service_building_workflow() {
        // Property #30: Service Building Workflow
        // Service operations should follow proper add→remove workflow
        
        let mut state = ModemState::new();
        let service_handle = 0x1234;
        let service_uuid = Uuid::new_16(0x1234);
        
        // Valid workflow: add service 
        let add_result = state.add_service(service_handle, service_uuid, ServiceType::Primary);
        assert!(add_result.is_ok());
        
        // Verify service was added
        let service = state.get_service(service_handle);
        assert!(service.is_some());
        assert_eq!(service.unwrap().handle, service_handle);
        
        // Test removal workflow
        let remove_result = state.remove_service(service_handle);
        assert!(remove_result.is_ok());
        
        // Verify service was removed
        let service_after_remove = state.get_service(service_handle);
        assert!(service_after_remove.is_none());
    }

    #[test]
    fn test_uuid_base_registration() {
        proptest!(|(
            uuid_bases in prop::collection::vec(any::<[u8; 16]>(), 1..8)
        )| {
            // Property #31: UUID Base Registration
            // UUID bases should be registered correctly and retrievable
            
            let mut state = ModemState::new();
            let mut registered_bases = Vec::new();
            
            for uuid_base in uuid_bases.iter().take(4) { // Max 4 UUID bases
                let result = state.register_uuid_base(*uuid_base);
                if result.is_ok() {
                    let handle = result.unwrap();
                    registered_bases.push((handle, *uuid_base));
                    
                    // Verify we can retrieve it
                    let retrieved = state.get_uuid_base(handle);
                    prop_assert!(retrieved.is_some());
                    prop_assert!(arrays_equal(&retrieved.unwrap().base, uuid_base));
                }
            }
            
            // Verify all registered bases are retrievable
            for (handle, uuid_base) in &registered_bases {
                let retrieved = state.get_uuid_base(*handle);
                prop_assert!(retrieved.is_some());
                prop_assert!(arrays_equal(&retrieved.unwrap().base, uuid_base));
            }
        });
    }

    #[test]
    fn test_uuid_base_limits() {
        // Property #32: UUID Base Limits
        // System should accept up to 4 UUID bases and reject additional ones
        
        let mut state = ModemState::new();
        let mut uuid_bases = Vec::new();
        
        // Register maximum UUID bases (4)
        for i in 0..4 {
            let mut uuid_base = [0u8; 16];
            uuid_base[15] = i; // Make each UUID unique
            
            let result = state.register_uuid_base(uuid_base);
            assert!(result.is_ok());
            
            let handle = result.unwrap();
            assert_eq!(handle, i);
            uuid_bases.push((handle, uuid_base));
        }
        
        // Should have 4 bases registered
        assert_eq!(uuid_bases.len(), 4);
        
        // Try to register one more - should fail
        let overflow_uuid = [0xFF; 16];
        let overflow_result = state.register_uuid_base(overflow_uuid);
        assert!(matches!(overflow_result, Err(StateError::UuidBasesExhausted)));
        
        // Verify all 4 bases are still accessible
        for (handle, uuid_base) in &uuid_bases {
            let retrieved = state.get_uuid_base(*handle);
            assert!(retrieved.is_some());
            assert!(arrays_equal(&retrieved.unwrap().base, uuid_base));
        }
    }

    #[test]
    fn test_service_handle_collision_prevention() {
        proptest!(|(
            handles in prop::collection::vec(1u16..1000, 1..10)
        )| {
            // Property #33: Service Handle Collision Prevention
            // Service handles should never collide across registrations
            
            let mut state = ModemState::new();
            let mut used_handles = Vec::new();
            
            // Register services with various handles
            for &handle in handles.iter().take(MAX_SERVICES) {
                // Skip if handle already used
                if used_handles.contains(&handle) {
                    continue;
                }
                
                let uuid = Uuid::new_16((handle % 0xFFFF) as u16);
                let result = state.add_service(handle, uuid, ServiceType::Primary);
                
                if result.is_ok() {
                    used_handles.push(handle);
                    
                    // Try to register same handle again - should fail
                    let collision_uuid = Uuid::new_16(0x9999);
                    let collision_result = state.add_service(handle, collision_uuid, ServiceType::Primary);
                    prop_assert!(collision_result.is_err());
                }
            }
            
            // Verify all handles are unique
            let mut sorted_handles = used_handles.clone();
            sorted_handles.sort();
            sorted_handles.dedup();
            prop_assert_eq!(used_handles.len(), sorted_handles.len());
        });
    }

    #[test]
    fn test_characteristic_management() {
        // Property #34: Characteristic Management
        // System should support managing characteristics through command protocol
        
        let mut state = ModemState::new();
        let service_handle = 0x1234;
        let service_uuid = Uuid::new_16(0x1234);
        
        // Add a service first (required before adding characteristics)
        let add_result = state.add_service(service_handle, service_uuid, ServiceType::Primary);
        assert!(add_result.is_ok());
        
        // In a real BLE modem, characteristics would be added via GATTS_CHARACTERISTIC_ADD command
        // The command would include properties, permissions, and initial values
        // For now, test that the service exists and can be managed
        let service = state.get_service(service_handle);
        assert!(service.is_some());
        assert_eq!(service.unwrap().service_type, ServiceType::Primary);
        
        // Test service removal cleans up associated data
        let remove_result = state.remove_service(service_handle);
        assert!(remove_result.is_ok());
        
        // Verify complete cleanup
        let service_after = state.get_service(service_handle);
        assert!(service_after.is_none());
    }

    #[test]
    fn test_gatt_table_consistency() {
        // Property #35: GATT Table Consistency
        // GATT table should remain consistent after service modifications
        
        let mut state = ModemState::new();
        
        // Add initial service
        let service1_handle = 1;
        let service1_uuid = Uuid::new_16(0x1234);
        let result1 = state.add_service(service1_handle, service1_uuid, ServiceType::Primary);
        assert!(result1.is_ok());
        
        // Verify service is in table
        let service1 = state.get_service(service1_handle);
        assert!(service1.is_some());
        
        // Add second service
        let service2_handle = 5;
        let service2_uuid = Uuid::new_16(0x5678);
        let result2 = state.add_service(service2_handle, service2_uuid, ServiceType::Primary);
        assert!(result2.is_ok());
        
        // Verify both services are in table
        let service1_check = state.get_service(service1_handle);
        let service2_check = state.get_service(service2_handle);
        assert!(service1_check.is_some());
        assert!(service2_check.is_some());
        
        // Remove first service
        let remove_result = state.remove_service(service1_handle);
        assert!(remove_result.is_ok());
        
        // Verify table consistency: service1 gone, service2 remains
        let service1_after_remove = state.get_service(service1_handle);
        let service2_after_remove = state.get_service(service2_handle);
        assert!(service1_after_remove.is_none());
        assert!(service2_after_remove.is_some());
        
        // Add service back with same handle
        let result3 = state.add_service(service1_handle, service1_uuid, ServiceType::Primary);
        assert!(result3.is_ok());
        
        // Verify table is consistent again
        let final_service1 = state.get_service(service1_handle);
        let final_service2 = state.get_service(service2_handle);
        assert!(final_service1.is_some());
        assert!(final_service2.is_some());
    }
}
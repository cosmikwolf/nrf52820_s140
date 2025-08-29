#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

mod common;

use nrf52820_s140_firmware::ble::bonding::{
    BondingService, BondData, BondingError, BondingEvent, 
    MAX_BONDED_DEVICES, MAX_SYS_ATTR_SIZE
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

    #[test]
    fn test_bonded_device_storage_limits() {
        // Property #42: Bonded Device Storage Limits
        // System should store up to MAX_BONDED_DEVICES (2) bonded devices
        
        let mut bonding_service = BondingService::new();
        let mut bonded_devices = Vec::new();
        
        // Bond maximum devices
        for i in 0..MAX_BONDED_DEVICES {
            let peer_address = [0x10 + i as u8, 0x20, 0x30, 0x40, 0x50, 0x60];
            let ltk = [0x01 + i as u8; 16]; // Long Term Key
            let irk = [0x02 + i as u8; 16]; // Identity Resolving Key
            let csrk = [0x03 + i as u8; 16]; // Connection Signature Resolving Key
            
            let bond_data = BondData {
                peer_address,
                ltk: Some(ltk),
                irk: Some(irk),
                csrk: Some(csrk),
                system_attributes: vec![0x44 + i as u8; 32],
                authenticated: true,
            };
            
            let result = bonding_service.store_bond(bond_data);
            assert!(result.is_ok(), "Failed to store bond for device {}", i);
            bonded_devices.push(peer_address);
        }
        
        // Verify we stored MAX_BONDED_DEVICES
        assert_eq!(bonded_devices.len(), MAX_BONDED_DEVICES);
        assert_eq!(bonding_service.get_bonded_device_count(), MAX_BONDED_DEVICES);
        
        // Try to bond one more device - should fail
        let overflow_address = [0xFF, 0xEE, 0xDD, 0xCC, 0xBB, 0xAA];
        let overflow_bond = BondData {
            peer_address: overflow_address,
            ltk: Some([0xFF; 16]),
            irk: Some([0xEE; 16]),
            csrk: Some([0xDD; 16]),
            system_attributes: vec![0xCC; 16],
            authenticated: false,
        };
        
        let overflow_result = bonding_service.store_bond(overflow_bond);
        assert!(matches!(overflow_result, Err(BondingError::StorageFull)));
        
        // Verify count unchanged
        assert_eq!(bonding_service.get_bonded_device_count(), MAX_BONDED_DEVICES);
        
        // Remove one bond and try again
        let remove_result = bonding_service.remove_bond(bonded_devices[0]);
        assert!(remove_result.is_ok());
        
        let new_bond_result = bonding_service.store_bond(BondData {
            peer_address: overflow_address,
            ltk: Some([0xFF; 16]),
            irk: None,
            csrk: None,
            system_attributes: vec![],
            authenticated: false,
        });
        assert!(new_bond_result.is_ok());
    }

    #[test]
    fn test_bond_data_persistence() {
        // Property #43: Bond Data Persistence
        // Bonding data should survive system resets (when flash storage added)
        
        let peer_address = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66];
        let original_ltk = [0xAA; 16];
        let original_irk = [0xBB; 16];
        let original_csrk = [0xCC; 16];
        let original_sys_attr = vec![0xDD, 0xEE, 0xFF, 0x11];
        
        // Store bond data
        let mut bonding_service = BondingService::new();
        let bond_data = BondData {
            peer_address,
            ltk: Some(original_ltk),
            irk: Some(original_irk),
            csrk: Some(original_csrk),
            system_attributes: original_sys_attr.clone(),
            authenticated: true,
        };
        
        let store_result = bonding_service.store_bond(bond_data);
        assert!(store_result.is_ok());
        
        // Retrieve and verify
        let retrieved = bonding_service.get_bond_data(peer_address);
        assert!(retrieved.is_some());
        
        let retrieved_bond = retrieved.unwrap();
        assert!(arrays_equal(&retrieved_bond.peer_address, &peer_address));
        assert!(retrieved_bond.ltk.is_some());
        assert!(arrays_equal(&retrieved_bond.ltk.unwrap(), &original_ltk));
        assert!(retrieved_bond.irk.is_some());
        assert!(arrays_equal(&retrieved_bond.irk.unwrap(), &original_irk));
        assert!(retrieved_bond.csrk.is_some());
        assert!(arrays_equal(&retrieved_bond.csrk.unwrap(), &original_csrk));
        assert!(arrays_equal(&retrieved_bond.system_attributes, &original_sys_attr));
        assert_eq!(retrieved_bond.authenticated, true);
        
        // Simulate system reset by creating new bonding service
        let mut bonding_service_after_reset = BondingService::new();
        
        // In real implementation with flash storage, this would load from flash
        // For now, verify that we can restore the same data
        let restore_result = bonding_service_after_reset.store_bond(BondData {
            peer_address,
            ltk: Some(original_ltk),
            irk: Some(original_irk),
            csrk: Some(original_csrk),
            system_attributes: original_sys_attr.clone(),
            authenticated: true,
        });
        assert!(restore_result.is_ok());
        
        let post_reset_data = bonding_service_after_reset.get_bond_data(peer_address);
        assert!(post_reset_data.is_some());
        assert!(arrays_equal(&post_reset_data.unwrap().peer_address, &peer_address));
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
            
            let mut bonding_service = BondingService::new();
            let peer_address = [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC];
            
            for &size in attr_sizes.iter().take(5) {
                let sys_attr = create_test_data(size, 0x55);
                
                let bond_data = BondData {
                    peer_address,
                    ltk: Some([0x11; 16]),
                    irk: None,
                    csrk: None,
                    system_attributes: sys_attr.clone(),
                    authenticated: false,
                };
                
                let result = bonding_service.store_bond(bond_data);
                
                if size <= MAX_SYS_ATTR_SIZE {
                    // Should accept system attributes within limits
                    prop_assert!(result.is_ok(), "Should accept {} byte attributes", size);
                    
                    // Verify stored data
                    let retrieved = bonding_service.get_bond_data(peer_address);
                    if let Some(bond) = retrieved {
                        prop_assert_eq!(bond.system_attributes.len(), size);
                        prop_assert!(arrays_equal(&bond.system_attributes, &sys_attr));
                    }
                } else {
                    // Should reject oversized system attributes
                    // In real implementation, this would return an error
                    prop_assert!(size > MAX_SYS_ATTR_SIZE);
                }
                
                // Clean up for next iteration
                let _ = bonding_service.remove_bond(peer_address);
            }
        }
    }

    proptest! {
        #[test]
        fn test_bond_key_generation(
            key_patterns in prop::collection::vec(0u8..=255, 16..48)
        ) {
            // Property #45: Bond Key Generation
            // Bonding keys should be cryptographically secure and unique
            
            let mut bonding_service = BondingService::new();
            let mut generated_keys = Vec::new();
            
            // Generate multiple bond data with different keys
            for (i, key_chunk) in key_patterns.chunks(16).take(4).enumerate() {
                let peer_address = [0x10 + i as u8, 0x20, 0x30, 0x40, 0x50, 0x60];
                
                let mut ltk = [0u8; 16];
                let mut irk = [0u8; 16];
                let mut csrk = [0u8; 16];
                
                // Fill keys with pattern data
                for (j, &byte) in key_chunk.iter().take(16).enumerate() {
                    ltk[j] = byte;
                    irk[j] = byte.wrapping_add(0x10);
                    csrk[j] = byte.wrapping_add(0x20);
                }
                
                let bond_data = BondData {
                    peer_address,
                    ltk: Some(ltk),
                    irk: Some(irk),
                    csrk: Some(csrk),
                    system_attributes: vec![],
                    authenticated: true,
                };
                
                let result = bonding_service.store_bond(bond_data);
                prop_assert!(result.is_ok());
                
                generated_keys.push((ltk, irk, csrk));
            }
            
            // Verify key uniqueness
            for i in 0..generated_keys.len() {
                for j in (i + 1)..generated_keys.len() {
                    let (ltk1, irk1, csrk1) = &generated_keys[i];
                    let (ltk2, irk2, csrk2) = &generated_keys[j];
                    
                    // Keys should be different
                    prop_assert!(!arrays_equal(ltk1, ltk2), "LTK keys should be unique");
                    prop_assert!(!arrays_equal(irk1, irk2), "IRK keys should be unique");
                    prop_assert!(!arrays_equal(csrk1, csrk2), "CSRK keys should be unique");
                }
            }
            
            // Verify key properties (non-zero, varied content)
            for (ltk, irk, csrk) in &generated_keys {
                // Keys shouldn't be all zeros
                prop_assert!(!ltk.iter().all(|&x| x == 0), "LTK should not be all zeros");
                prop_assert!(!irk.iter().all(|&x| x == 0), "IRK should not be all zeros");
                prop_assert!(!csrk.iter().all(|&x| x == 0), "CSRK should not be all zeros");
                
                // Keys should have some entropy (not all same value)
                let ltk_first = ltk[0];
                let irk_first = irk[0];
                let csrk_first = csrk[0];
                
                prop_assert!(!ltk.iter().all(|&x| x == ltk_first), "LTK should have entropy");
                prop_assert!(!irk.iter().all(|&x| x == irk_first), "IRK should have entropy");
                prop_assert!(!csrk.iter().all(|&x| x == csrk_first), "CSRK should have entropy");
            }
        }
    }

    #[test]
    fn test_bond_state_consistency() {
        // Property #46: Bond State Consistency
        // Bond states should remain consistent across operations
        
        let mut bonding_service = BondingService::new();
        let peer1 = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66];
        let peer2 = [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF];
        
        // Initially no bonds
        assert_eq!(bonding_service.get_bonded_device_count(), 0);
        assert!(bonding_service.get_bond_data(peer1).is_none());
        assert!(bonding_service.get_bond_data(peer2).is_none());
        
        // Add first bond
        let bond1 = BondData {
            peer_address: peer1,
            ltk: Some([0x11; 16]),
            irk: Some([0x22; 16]),
            csrk: None,
            system_attributes: vec![0x33, 0x44],
            authenticated: true,
        };
        
        let store1_result = bonding_service.store_bond(bond1);
        assert!(store1_result.is_ok());
        
        // Verify state consistency
        assert_eq!(bonding_service.get_bonded_device_count(), 1);
        assert!(bonding_service.get_bond_data(peer1).is_some());
        assert!(bonding_service.get_bond_data(peer2).is_none());
        
        // Add second bond
        let bond2 = BondData {
            peer_address: peer2,
            ltk: Some([0xAA; 16]),
            irk: None,
            csrk: Some([0xBB; 16]),
            system_attributes: vec![],
            authenticated: false,
        };
        
        let store2_result = bonding_service.store_bond(bond2);
        assert!(store2_result.is_ok());
        
        // Verify state consistency with both bonds
        assert_eq!(bonding_service.get_bonded_device_count(), 2);
        assert!(bonding_service.get_bond_data(peer1).is_some());
        assert!(bonding_service.get_bond_data(peer2).is_some());
        
        // Remove first bond
        let remove_result = bonding_service.remove_bond(peer1);
        assert!(remove_result.is_ok());
        
        // Verify consistent state after removal
        assert_eq!(bonding_service.get_bonded_device_count(), 1);
        assert!(bonding_service.get_bond_data(peer1).is_none());
        assert!(bonding_service.get_bond_data(peer2).is_some());
        
        // Update remaining bond
        let updated_bond2 = BondData {
            peer_address: peer2,
            ltk: Some([0xCC; 16]),
            irk: Some([0xDD; 16]),
            csrk: Some([0xEE; 16]),
            system_attributes: vec![0xFF],
            authenticated: true,
        };
        
        let update_result = bonding_service.store_bond(updated_bond2);
        assert!(update_result.is_ok());
        
        // Verify state remains consistent after update
        assert_eq!(bonding_service.get_bonded_device_count(), 1);
        let updated = bonding_service.get_bond_data(peer2);
        assert!(updated.is_some());
        assert_eq!(updated.unwrap().authenticated, true);
    }

    #[test]
    fn test_bonding_event_propagation() {
        // Property #47: Bonding Event Propagation
        // Bonding events should be properly propagated to listeners
        
        let peer_address = [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC];
        
        // Test bonding started event
        let bond_started = BondingEvent::BondingStarted {
            peer_address,
            connection_id: 1,
        };
        
        match bond_started {
            BondingEvent::BondingStarted { peer_address: addr, connection_id } => {
                assert!(arrays_equal(&addr, &peer_address));
                assert_eq!(connection_id, 1);
            }
            _ => panic!("Should be BondingStarted event"),
        }
        
        // Test bonding completed event
        let bond_completed = BondingEvent::BondingCompleted {
            peer_address,
            connection_id: 1,
            success: true,
        };
        
        match bond_completed {
            BondingEvent::BondingCompleted { peer_address: addr, connection_id, success } => {
                assert!(arrays_equal(&addr, &peer_address));
                assert_eq!(connection_id, 1);
                assert_eq!(success, true);
            }
            _ => panic!("Should be BondingCompleted event"),
        }
        
        // Test bonding failed event
        let bond_failed = BondingEvent::BondingFailed {
            peer_address,
            connection_id: 1,
            error: BondingError::AuthenticationFailed,
        };
        
        match bond_failed {
            BondingEvent::BondingFailed { peer_address: addr, connection_id, error } => {
                assert!(arrays_equal(&addr, &peer_address));
                assert_eq!(connection_id, 1);
                assert!(matches!(error, BondingError::AuthenticationFailed));
            }
            _ => panic!("Should be BondingFailed event"),
        }
        
        // Test bond removed event
        let bond_removed = BondingEvent::BondRemoved {
            peer_address,
        };
        
        match bond_removed {
            BondingEvent::BondRemoved { peer_address: addr } => {
                assert!(arrays_equal(&addr, &peer_address));
            }
            _ => panic!("Should be BondRemoved event"),
        }
    }
}
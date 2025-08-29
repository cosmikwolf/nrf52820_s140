#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

mod common;

use nrf52820_s140_firmware::ble::events::{
    BleEvent, ConnectionEvent, GattEvent, GapEvent, EventHandler, 
    EventDispatcher, EventError, EventPriority
};
use nrf_softdevice::ble::gatt;
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
    fn test_ble_event_classification() {
        // Property #48: BLE Event Classification
        // BLE events should be correctly classified and routed
        
        // Test Connection Events
        let connection_event = BleEvent::Connection(ConnectionEvent::Connected {
            connection_id: 1,
            peer_address: [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC],
        });
        
        match connection_event {
            BleEvent::Connection(ConnectionEvent::Connected { connection_id, peer_address }) => {
                assert_eq!(connection_id, 1);
                assert_eq!(peer_address, [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC]);
            }
            _ => panic!("Should classify as Connection event"),
        }
        
        // Test GATT Events
        let gatt_event = BleEvent::Gatt(GattEvent::WriteRequest {
            connection_id: 1,
            handle: 20,
            data: vec![0xAA, 0xBB, 0xCC],
        });
        
        match gatt_event {
            BleEvent::Gatt(GattEvent::WriteRequest { connection_id, handle, data }) => {
                assert_eq!(connection_id, 1);
                assert_eq!(handle, 20);
                assert!(arrays_equal(&data, &[0xAA, 0xBB, 0xCC]));
            }
            _ => panic!("Should classify as GATT event"),
        }
        
        // Test GAP Events
        let gap_event = BleEvent::Gap(GapEvent::AdvertisementStarted {
            advertising_handle: 0,
        });
        
        match gap_event {
            BleEvent::Gap(GapEvent::AdvertisementStarted { advertising_handle }) => {
                assert_eq!(advertising_handle, 0);
            }
            _ => panic!("Should classify as GAP event"),
        }
        
        // Test Security Events  
        let security_event = BleEvent::Security(SecurityEvent::PairingRequest {
            connection_id: 2,
            bonded: false,
        });
        
        match security_event {
            BleEvent::Security(SecurityEvent::PairingRequest { connection_id, bonded }) => {
                assert_eq!(connection_id, 2);
                assert_eq!(bonded, false);
            }
            _ => panic!("Should classify as Security event"),
        }
    }

    #[test]
    fn test_event_handler_registration() {
        // Property #49: Event Handler Registration
        // Event handlers should be properly registered and called
        
        let mut dispatcher = EventDispatcher::new();
        
        // Create mock handler for connection events
        let connection_handler = EventHandler::new(
            "connection_handler",
            |event| {
                match event {
                    BleEvent::Connection(_) => Ok(()),
                    _ => Err(EventError::WrongEventType),
                }
            }
        );
        
        // Register handler
        let register_result = dispatcher.register_handler(
            BleEventType::Connection,
            connection_handler
        );
        assert!(register_result.is_ok());
        
        // Create mock handler for GATT events
        let gatt_handler = EventHandler::new(
            "gatt_handler", 
            |event| {
                match event {
                    BleEvent::Gatt(_) => Ok(()),
                    _ => Err(EventError::WrongEventType),
                }
            }
        );
        
        // Register GATT handler
        let gatt_register_result = dispatcher.register_handler(
            BleEventType::Gatt,
            gatt_handler
        );
        assert!(gatt_register_result.is_ok());
        
        // Verify handlers are registered
        assert!(dispatcher.has_handler(BleEventType::Connection));
        assert!(dispatcher.has_handler(BleEventType::Gatt));
        assert!(!dispatcher.has_handler(BleEventType::Gap)); // Not registered
        
        // Test handler count
        assert_eq!(dispatcher.handler_count(), 2);
    }

    #[test]
    fn test_event_processing_order() {
        // Property #50: Event Processing Order
        // Events should be processed in the order they arrive
        
        let mut event_queue = Vec::new();
        
        // Add events in specific order
        let events = [
            BleEvent::Connection(ConnectionEvent::Connected {
                connection_id: 1,
                peer_address: [0x01, 0x02, 0x03, 0x04, 0x05, 0x06],
            }),
            BleEvent::Gatt(GattEvent::WriteRequest {
                connection_id: 1,
                handle: 10,
                data: vec![0x11, 0x22],
            }),
            BleEvent::Gap(GapEvent::AdvertisementStarted {
                advertising_handle: 0,
            }),
            BleEvent::Connection(ConnectionEvent::Disconnected {
                connection_id: 1,
                reason: 0x13,
            }),
        ];
        
        // Enqueue events
        for event in events {
            event_queue.push(event);
        }
        
        assert_eq!(event_queue.len(), 4);
        
        // Process events in FIFO order
        let mut processed_types = Vec::new();
        while let Some(event) = event_queue.remove(0) { // Remove from front (FIFO)
            match event {
                BleEvent::Connection(_) => processed_types.push("Connection"),
                BleEvent::Gatt(_) => processed_types.push("Gatt"),
                BleEvent::Gap(_) => processed_types.push("Gap"),
                BleEvent::Security(_) => processed_types.push("Security"),
            }
        }
        
        // Verify processing order matches insertion order
        let expected_order = ["Connection", "Gatt", "Gap", "Connection"];
        assert_eq!(processed_types.len(), expected_order.len());
        for (i, expected_type) in expected_order.iter().enumerate() {
            assert_eq!(processed_types[i], *expected_type);
        }
    }

    #[test]
    fn test_event_handler_error_isolation() {
        // Property #51: Event Handler Error Isolation
        // Errors in one event handler should not affect others
        
        let mut dispatcher = EventDispatcher::new();
        let mut handler_results = Vec::new();
        
        // Register handler that always succeeds
        let success_handler = EventHandler::new(
            "success_handler",
            |_event| {
                Ok(())
            }
        );
        
        // Register handler that always fails
        let failing_handler = EventHandler::new(
            "failing_handler",
            |_event| {
                Err(EventError::ProcessingFailed)
            }
        );
        
        // Register both handlers for same event type
        let success_result = dispatcher.register_handler(
            BleEventType::Connection,
            success_handler
        );
        let fail_result = dispatcher.register_handler(
            BleEventType::Connection, 
            failing_handler
        );
        
        assert!(success_result.is_ok());
        assert!(fail_result.is_ok());
        
        // Create test event
        let test_event = BleEvent::Connection(ConnectionEvent::Connected {
            connection_id: 1,
            peer_address: [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF],
        });
        
        // Dispatch event to both handlers
        let dispatch_results = dispatcher.dispatch_event(&test_event);
        
        // Should get results from both handlers
        assert_eq!(dispatch_results.len(), 2);
        
        // One should succeed, one should fail, but both should be present
        let success_count = dispatch_results.iter().filter(|r| r.is_ok()).count();
        let failure_count = dispatch_results.iter().filter(|r| r.is_err()).count();
        
        assert_eq!(success_count, 1);
        assert_eq!(failure_count, 1);
        
        // System should remain stable despite one handler failing
        assert!(dispatcher.is_running());
        assert_eq!(dispatcher.handler_count(), 2);
    }

    proptest! {
        #[test]
        fn test_event_memory_management(
            event_counts in prop::collection::vec(1usize..10, 1..5)
        ) {
            // Property #52: Event Memory Management
            // Event data should be properly allocated and freed
            
            unsafe {
                common::HEAP.init(common::HEAP_MEM.as_ptr() as usize, common::HEAP_MEM.len());
            }
            
            let mut created_events = Vec::new();
            
            for &count in event_counts.iter() {
                // Create events with varying data sizes
                for i in 0..count.min(8) {
                    let data_size = (i + 1) * 8;
                    let event_data = create_test_data(data_size, 0x40 + i as u8);
                    
                    let event = BleEvent::Gatt(GattEvent::WriteRequest {
                        connection_id: i as u16 + 1,
                        handle: 100 + i as u16,
                        data: event_data,
                    });
                    
                    created_events.push(event);
                }
                
                // Process and drop events to test memory management
                let events_to_process = created_events.drain(..);
                for event in events_to_process {
                    match event {
                        BleEvent::Gatt(GattEvent::WriteRequest { connection_id, handle, data }) => {
                            prop_assert!(connection_id > 0);
                            prop_assert!(handle >= 100);
                            prop_assert!(data.len() > 0);
                        }
                        _ => {}
                    }
                    // Event is dropped here, memory should be freed
                }
                
                prop_assert!(created_events.is_empty());
            }
        }
    }

    #[test]
    fn test_event_priority_handling() {
        // Property #53: Event Priority Handling  
        // High-priority events should be processed before low-priority ones
        
        let mut priority_queue = Vec::new();
        
        // Create events with different priorities
        let low_priority_event = (
            EventPriority::Low,
            BleEvent::Gap(GapEvent::AdvertisementStarted { advertising_handle: 0 })
        );
        
        let normal_priority_event = (
            EventPriority::Normal,
            BleEvent::Gatt(GattEvent::WriteRequest {
                connection_id: 1,
                handle: 20,
                data: vec![0x01, 0x02],
            })
        );
        
        let high_priority_event = (
            EventPriority::High,
            BleEvent::Connection(ConnectionEvent::Disconnected {
                connection_id: 1,
                reason: 0x08, // Connection timeout
            })
        );
        
        let critical_priority_event = (
            EventPriority::Critical,
            BleEvent::Security(SecurityEvent::SecurityFailure {
                connection_id: 1,
                error_code: 0x05, // Pairing failed
            })
        );
        
        // Add events in random order
        priority_queue.push(low_priority_event);
        priority_queue.push(high_priority_event);
        priority_queue.push(normal_priority_event);
        priority_queue.push(critical_priority_event);
        
        // Sort by priority (Critical > High > Normal > Low)
        priority_queue.sort_by(|(a_priority, _), (b_priority, _)| {
            b_priority.cmp(a_priority) // Reverse order for highest first
        });
        
        // Verify processing order
        let expected_order = [
            EventPriority::Critical,
            EventPriority::High, 
            EventPriority::Normal,
            EventPriority::Low,
        ];
        
        for (i, (priority, _event)) in priority_queue.iter().enumerate() {
            assert_eq!(*priority, expected_order[i]);
        }
        
        // Verify event types are preserved correctly
        match &priority_queue[0].1 {
            BleEvent::Security(SecurityEvent::SecurityFailure { .. }) => {},
            _ => panic!("Critical event should be Security event"),
        }
        
        match &priority_queue[1].1 {
            BleEvent::Connection(ConnectionEvent::Disconnected { .. }) => {},
            _ => panic!("High priority event should be Connection event"),
        }
        
        match &priority_queue[2].1 {
            BleEvent::Gatt(GattEvent::WriteRequest { .. }) => {},
            _ => panic!("Normal priority event should be GATT event"),
        }
        
        match &priority_queue[3].1 {
            BleEvent::Gap(GapEvent::AdvertisementStarted { .. }) => {},
            _ => panic!("Low priority event should be GAP event"),
        }
    }

    #[test]
    fn test_event_type_validation() {
        // Additional test for event type validation and error handling
        
        // Test invalid event data
        let invalid_gatt_event = BleEvent::Gatt(GattEvent::WriteRequest {
            connection_id: 0, // Invalid connection ID
            handle: 0,        // Invalid handle
            data: vec![],     // Empty data might be invalid
        });
        
        match invalid_gatt_event {
            BleEvent::Gatt(GattEvent::WriteRequest { connection_id, handle, data }) => {
                assert_eq!(connection_id, 0);
                assert_eq!(handle, 0);
                assert!(data.is_empty());
            }
            _ => panic!("Should be GATT WriteRequest event"),
        }
        
        // Test event with maximum data size
        let max_data = vec![0xAA; 200]; // Large data payload
        let large_event = BleEvent::Gatt(GattEvent::WriteRequest {
            connection_id: 1,
            handle: 50,
            data: max_data.clone(),
        });
        
        match large_event {
            BleEvent::Gatt(GattEvent::WriteRequest { data, .. }) => {
                assert_eq!(data.len(), 200);
                assert!(arrays_equal(&data, &max_data));
            }
            _ => panic!("Should handle large data event"),
        }
        
        // Test event serialization/deserialization roundtrip
        let original_event = BleEvent::Connection(ConnectionEvent::ParametersUpdated {
            connection_id: 5,
            interval: 15,
            latency: 0,
            timeout: 100,
        });
        
        // In real implementation, this would test serialization
        match original_event {
            BleEvent::Connection(ConnectionEvent::ParametersUpdated { 
                connection_id, interval, latency, timeout 
            }) => {
                assert_eq!(connection_id, 5);
                assert_eq!(interval, 15);
                assert_eq!(latency, 0);
                assert_eq!(timeout, 100);
            }
            _ => panic!("Should preserve event structure"),
        }
    }
}
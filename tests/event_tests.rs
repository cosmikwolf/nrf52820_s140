#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

mod common;

use nrf52820_s140_firmware::ble::events::{
    BleModemEvent, create_disconnected_event, 
    create_gatts_write_event, create_cccd_write_event
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
    fn test_ble_modem_event_types() {
        // Property #48: BLE Modem Event Types
        // BLE modem events should be correctly created and serialized
        
        // Test Connected Event
        let connected_event = BleModemEvent::Connected {
            conn_handle: 1,
            peer_addr: [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC],
            addr_type: 0,
        };
        
        match connected_event {
            BleModemEvent::Connected { conn_handle, peer_addr, addr_type } => {
                assert_eq!(conn_handle, 1);
                assert_eq!(peer_addr, [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC]);
                assert_eq!(addr_type, 0);
            }
            _ => panic!("Should be Connected event"),
        }
        
        // Test GATTS Write Event
        let mut write_data = heapless::Vec::new();
        let _ = write_data.extend_from_slice(&[0xAA, 0xBB, 0xCC]);
        let write_event = BleModemEvent::GattsWrite {
            conn_handle: 1,
            char_handle: 20,
            data: write_data.clone(),
        };
        
        match write_event {
            BleModemEvent::GattsWrite { conn_handle, char_handle, data } => {
                assert_eq!(conn_handle, 1);
                assert_eq!(char_handle, 20);
                assert_eq!(data.len(), 3);
                assert_eq!(data[0], 0xAA);
                assert_eq!(data[1], 0xBB);
                assert_eq!(data[2], 0xCC);
            }
            _ => panic!("Should be GattsWrite event"),
        }
        
        // Test Disconnected Event
        let disconnected_event = BleModemEvent::Disconnected {
            conn_handle: 1,
            reason: 0x13, // Remote user terminated connection
        };
        
        match disconnected_event {
            BleModemEvent::Disconnected { conn_handle, reason } => {
                assert_eq!(conn_handle, 1);
                assert_eq!(reason, 0x13);
            }
            _ => panic!("Should be Disconnected event"),
        }
        
        // Test CCCD Write Event
        let cccd_event = BleModemEvent::CccdWrite {
            conn_handle: 1,
            char_handle: 25,
            notifications: true,
            indications: false,
        };
        
        match cccd_event {
            BleModemEvent::CccdWrite { conn_handle, char_handle, notifications, indications } => {
                assert_eq!(conn_handle, 1);
                assert_eq!(char_handle, 25);
                assert_eq!(notifications, true);
                assert_eq!(indications, false);
            }
            _ => panic!("Should be CccdWrite event"),
        }
    }

    #[test]
    fn test_event_serialization() {
        // Property #49: Event Serialization
        // BLE modem events should be properly serialized for transmission
        
        // Test Connected event serialization
        let connected_event = BleModemEvent::Connected {
            conn_handle: 0x1234,
            peer_addr: [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF],
            addr_type: 1,
        };
        
        let serialized = connected_event.serialize();
        assert!(serialized.is_ok());
        let data = serialized.unwrap();
        
        // Check event type (BLE_GAP_EVT_CONNECTED = 0x11)
        assert_eq!(data[0], 0x11);
        assert_eq!(data[1], 0x00);
        
        // Check connection handle (little endian)
        assert_eq!(data[2], 0x34); // Low byte
        assert_eq!(data[3], 0x12); // High byte
        
        // Check address type and address
        assert_eq!(data[4], 1); // addr_type
        assert_eq!(data[5], 0xAA); // First byte of address
        assert_eq!(data[10], 0xFF); // Last byte of address
        
        // Test Disconnected event serialization
        let disconnected_event = BleModemEvent::Disconnected {
            conn_handle: 0x5678,
            reason: 0x13,
        };
        
        let disconnected_data = disconnected_event.serialize().unwrap();
        
        // Check event type (BLE_GAP_EVT_DISCONNECTED = 0x12)
        assert_eq!(disconnected_data[0], 0x12);
        assert_eq!(disconnected_data[1], 0x00);
        
        // Check connection handle
        assert_eq!(disconnected_data[2], 0x78); // Low byte
        assert_eq!(disconnected_data[3], 0x56); // High byte
        
        // Check reason
        assert_eq!(disconnected_data[4], 0x13);
        
        // Test GATTS Write event serialization
        let mut write_data = heapless::Vec::new();
        let _ = write_data.extend_from_slice(&[0x01, 0x02, 0x03]);
        let write_event = BleModemEvent::GattsWrite {
            conn_handle: 0x9ABC,
            char_handle: 0xDEF0,
            data: write_data,
        };
        
        let write_serialized = write_event.serialize().unwrap();
        
        // Check event type (BLE_GATTS_EVT_WRITE = 0x50)
        assert_eq!(write_serialized[0], 0x50);
        assert_eq!(write_serialized[1], 0x00);
        
        // Check connection and characteristic handles
        assert_eq!(write_serialized[2], 0xBC); // conn_handle low
        assert_eq!(write_serialized[3], 0x9A); // conn_handle high
        assert_eq!(write_serialized[4], 0xF0); // char_handle low
        assert_eq!(write_serialized[5], 0xDE); // char_handle high
        
        // Check data length and payload
        assert_eq!(write_serialized[6], 3); // Data length
        assert_eq!(write_serialized[7], 0x01); // First data byte
        assert_eq!(write_serialized[8], 0x02); // Second data byte
        assert_eq!(write_serialized[9], 0x03); // Third data byte
    }

    #[test]
    fn test_event_creation_helpers() {
        // Property #50: Event Creation Helpers
        // Helper functions should create proper BLE modem events
        
        // Test create_disconnected_event
        let conn_handle = 42;
        let reason = 0x16; // Connection timeout
        let disc_event = create_disconnected_event(conn_handle, reason);
        
        match disc_event {
            BleModemEvent::Disconnected { conn_handle: h, reason: r } => {
                assert_eq!(h, 42);
                assert_eq!(r, 0x16);
            }
            _ => panic!("Should create Disconnected event"),
        }
        
        // Test create_gatts_write_event
        let conn_handle = 123;
        let char_handle = 456;
        let test_data = [0xDE, 0xAD, 0xBE, 0xEF];
        let write_result = create_gatts_write_event(conn_handle, char_handle, &test_data);
        
        assert!(write_result.is_ok());
        let write_event = write_result.unwrap();
        
        match &write_event {
            BleModemEvent::GattsWrite { conn_handle: ch, char_handle: chrh, data } => {
                assert_eq!(*ch, 123);
                assert_eq!(*chrh, 456);
                assert_eq!(data.len(), 4);
                assert_eq!(data[0], 0xDE);
                assert_eq!(data[1], 0xAD);
                assert_eq!(data[2], 0xBE);
                assert_eq!(data[3], 0xEF);
            }
            _ => panic!("Should create GattsWrite event"),
        }
        
        // Test create_cccd_write_event
        let cccd_event = create_cccd_write_event(789, 321, true, false);
        
        match cccd_event {
            BleModemEvent::CccdWrite { 
                conn_handle, char_handle, notifications, indications 
            } => {
                assert_eq!(conn_handle, 789);
                assert_eq!(char_handle, 321);
                assert_eq!(notifications, true);
                assert_eq!(indications, false);
            }
            _ => panic!("Should create CccdWrite event"),
        }
        
        // Test event queue ordering (simple Vec approach)
        let mut event_queue = Vec::new();
        
        // Add events in specific order
        event_queue.push(disc_event);
        event_queue.push(write_event);
        event_queue.push(cccd_event);
        
        assert_eq!(event_queue.len(), 3);
        
        // Process events in FIFO order and verify types
        let first_event = event_queue.remove(0);
        match first_event {
            BleModemEvent::Disconnected { .. } => {}, // Expected
            _ => panic!("First event should be Disconnected"),
        }
        
        let second_event = event_queue.remove(0);
        match second_event {
            BleModemEvent::GattsWrite { .. } => {}, // Expected
            _ => panic!("Second event should be GattsWrite"),
        }
        
        let third_event = event_queue.remove(0);
        match third_event {
            BleModemEvent::CccdWrite { .. } => {}, // Expected
            _ => panic!("Third event should be CccdWrite"),
        }
        
        assert!(event_queue.is_empty());
    }

    #[test]
    fn test_event_size_limits() {
        // Property #51: Event Size Limits
        // BLE modem events should respect size constraints for transmission
        
        // Test maximum GATTS write data size
        let conn_handle = 1;
        let char_handle = 2;
        
        // Create data at the limit (64 bytes for Vec<u8, 64>)
        let mut max_data = heapless::Vec::new();
        for i in 0..64 {
            let _ = max_data.push(i as u8);
        }
        
        let max_write_event = BleModemEvent::GattsWrite {
            conn_handle,
            char_handle,
            data: max_data.clone(),
        };
        
        // Should serialize successfully
        let serialized_result = max_write_event.serialize();
        assert!(serialized_result.is_ok());
        let serialized = serialized_result.unwrap();
        
        // Verify the data length field and payload
        assert_eq!(serialized[6], 64); // Data length byte
        assert_eq!(serialized.len(), 7 + 64); // Header (7) + data (64)
        
        // Verify first and last data bytes
        assert_eq!(serialized[7], 0); // First data byte
        assert_eq!(serialized[70], 63); // Last data byte (7 + 63)
        
        // Test event with large connection handle values
        let large_conn_handle = 0xFFFE; // Near maximum u16 value
        let large_char_handle = 0xFFFD;
        
        let large_handle_event = BleModemEvent::GattsRead {
            conn_handle: large_conn_handle,
            char_handle: large_char_handle,
        };
        
        let large_serialized = large_handle_event.serialize().unwrap();
        
        // Verify event type (BLE_GATTS_EVT_READ = 0x51)
        assert_eq!(large_serialized[0], 0x51);
        assert_eq!(large_serialized[1], 0x00);
        
        // Verify large handles are serialized correctly (little endian)
        assert_eq!(large_serialized[2], 0xFE); // conn_handle low
        assert_eq!(large_serialized[3], 0xFF); // conn_handle high
        assert_eq!(large_serialized[4], 0xFD); // char_handle low  
        assert_eq!(large_serialized[5], 0xFF); // char_handle high
        
        // Test MTU exchange with large MTU values
        let mtu_event = BleModemEvent::MtuExchange {
            conn_handle: 1,
            client_mtu: 512, // Large MTU
            server_mtu: 247, // Nordic maximum
        };
        
        let mtu_serialized = mtu_event.serialize().unwrap();
        
        // Check MTU values are serialized correctly
        assert_eq!(mtu_serialized[4], 0x00); // client_mtu low (512 & 0xFF = 0)
        assert_eq!(mtu_serialized[5], 0x02); // client_mtu high (512 >> 8 = 2)
        assert_eq!(mtu_serialized[6], 0xF7); // server_mtu low (247 & 0xFF = 247)
        assert_eq!(mtu_serialized[7], 0x00); // server_mtu high (247 >> 8 = 0)
        
        // Test CCCD value encoding
        let cccd_both = BleModemEvent::CccdWrite {
            conn_handle: 1,
            char_handle: 2,
            notifications: true,
            indications: true,
        };
        
        let cccd_serialized = cccd_both.serialize().unwrap();
        
        // CCCD value should be 3 (notifications=1 | indications=2)
        assert_eq!(cccd_serialized[6], 0x03);
    }

    proptest! {
        #[test]
        fn test_event_data_integrity(
            data_sizes in prop::collection::vec(1usize..64, 1..5),
            conn_handles in prop::collection::vec(1u16..1000, 1..5)
        ) {
            // Property #52: Event Data Integrity
            // Event data should maintain integrity through serialization
            
            unsafe {
                common::HEAP.init(common::HEAP_MEM.as_ptr() as usize, common::HEAP_MEM.len());
            }
            
            let mut created_events = Vec::new();
            
            for (&size, &handle) in data_sizes.iter().zip(conn_handles.iter()) {
                // Create GATTS write events with varying data sizes
                let mut event_data = heapless::Vec::new();
                for i in 0..size {
                    let _ = event_data.push((i % 256) as u8);
                }
                
                let event = BleModemEvent::GattsWrite {
                    conn_handle: handle,
                    char_handle: 100 + (size as u16),
                    data: event_data.clone(),
                };
                
                // Test serialization
                let serialized = event.serialize();
                prop_assert!(serialized.is_ok());
                
                let serialized_data = serialized.unwrap();
                
                // Verify event type
                prop_assert_eq!(serialized_data[0], 0x50); // BLE_GATTS_EVT_WRITE
                prop_assert_eq!(serialized_data[1], 0x00);
                
                // Verify handles
                prop_assert_eq!(serialized_data[2], (handle & 0xFF) as u8);
                prop_assert_eq!(serialized_data[3], (handle >> 8) as u8);
                
                let expected_char_handle = 100 + (size as u16);
                prop_assert_eq!(serialized_data[4], (expected_char_handle & 0xFF) as u8);
                prop_assert_eq!(serialized_data[5], (expected_char_handle >> 8) as u8);
                
                // Verify data length and payload
                prop_assert_eq!(serialized_data[6], size as u8);
                for i in 0..size {
                    prop_assert_eq!(serialized_data[7 + i], (i % 256) as u8);
                }
                
                created_events.push(event);
            }
            
            // Events should be properly managed in memory
            prop_assert_eq!(created_events.len(), data_sizes.len().min(conn_handles.len()));
            
            // Process and drop events
            for event in created_events.drain(..) {
                match event {
                    BleModemEvent::GattsWrite { conn_handle, char_handle, data } => {
                        prop_assert!(conn_handle > 0);
                        prop_assert!(char_handle >= 100);
                        prop_assert!(data.len() > 0);
                        prop_assert!(data.len() <= 64);
                    }
                    _ => {}
                }
            }
            
            prop_assert!(created_events.is_empty());
        }
    }

    #[test]
    fn test_event_transmission_reliability() {
        // Property #53: Event Transmission Reliability
        // Events should be properly formatted for reliable transmission over SPI
        
        // Test all event types for proper serialization format
        let test_events = [
            BleModemEvent::Connected {
                conn_handle: 0x1234,
                peer_addr: [0xDE, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE],
                addr_type: 1,
            },
            BleModemEvent::Disconnected {
                conn_handle: 0x1234,
                reason: 0x13,
            },
            BleModemEvent::GattsRead {
                conn_handle: 0x5678,
                char_handle: 0x9ABC,
            },
            BleModemEvent::MtuExchange {
                conn_handle: 0xDEF0,
                client_mtu: 247,
                server_mtu: 23,
            },
            BleModemEvent::CccdWrite {
                conn_handle: 0x1111,
                char_handle: 0x2222,
                notifications: false,
                indications: true,
            },
        ];
        
        // All events should serialize successfully
        for (i, event) in test_events.iter().enumerate() {
            let serialized = event.serialize();
            assert!(serialized.is_ok(), "Event {} should serialize successfully", i);
            
            let data = serialized.unwrap();
            
            // All events should have proper header structure
            assert!(data.len() >= 2, "Event {} should have at least 2-byte header", i);
            
            // Second byte should always be 0x00 (reserved/padding)
            assert_eq!(data[1], 0x00, "Event {} second byte should be 0x00", i);
            
            // Verify specific event type codes
            match event {
                BleModemEvent::Connected { .. } => assert_eq!(data[0], 0x11),
                BleModemEvent::Disconnected { .. } => assert_eq!(data[0], 0x12),
                BleModemEvent::GattsRead { .. } => assert_eq!(data[0], 0x51),
                BleModemEvent::MtuExchange { .. } => assert_eq!(data[0], 0x52),
                BleModemEvent::CccdWrite { .. } => assert_eq!(data[0], 0x53),
                BleModemEvent::GattsWrite { .. } => assert_eq!(data[0], 0x50),
            }
        }
        
        // Test edge cases for connection handles
        let edge_cases = [
            0x0000, // Minimum value
            0x0001, // Minimum valid connection
            0xFFFE, // Maximum valid connection
            0xFFFF, // Maximum value
        ];
        
        for &handle in &edge_cases {
            let event = BleModemEvent::Disconnected {
                conn_handle: handle,
                reason: 0x08,
            };
            
            let serialized = event.serialize().unwrap();
            
            // Verify handle is properly encoded in little endian
            let encoded_low = serialized[2];
            let encoded_high = serialized[3];
            let reconstructed = ((encoded_high as u16) << 8) | (encoded_low as u16);
            
            assert_eq!(reconstructed, handle, "Handle 0x{:04X} not properly serialized", handle);
        }
    }

    #[test]
    fn test_connection_event_integration() {
        // Property #53 (continued): Integration with Connection Events
        // BLE modem events should integrate properly with connection management
        
        // Test connection event creation from connection info
        let conn_handle = 42;
        let peer_addr = [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC];
        let addr_type = 1;
        
        // Create a connected event
        let connected = BleModemEvent::Connected {
            conn_handle,
            peer_addr,
            addr_type,
        };
        
        // Serialize and verify structure
        let serialized = connected.serialize().unwrap();
        
        // Verify complete structure for connected event
        assert_eq!(serialized.len(), 11); // Event(2) + Handle(2) + Type(1) + Addr(6)
        assert_eq!(serialized[0], 0x11); // BLE_GAP_EVT_CONNECTED
        assert_eq!(serialized[1], 0x00); // Reserved
        
        // Handle
        assert_eq!(serialized[2], 42); // Low byte
        assert_eq!(serialized[3], 0);  // High byte
        
        // Address type and address
        assert_eq!(serialized[4], 1); // addr_type
        for i in 0..6 {
            assert_eq!(serialized[5 + i], peer_addr[i]);
        }
        
        // Create corresponding disconnected event
        let disconnected = create_disconnected_event(conn_handle, 0x13);
        
        match disconnected {
            BleModemEvent::Disconnected { conn_handle: h, reason } => {
                assert_eq!(h, conn_handle);
                assert_eq!(reason, 0x13);
            }
            _ => panic!("Should create disconnected event"),
        }
        
        // Test that both events reference the same connection
        let disc_serialized = disconnected.serialize().unwrap();
        
        // Both should have same connection handle in serialization
        assert_eq!(serialized[2], disc_serialized[2]); // Same handle low byte
        assert_eq!(serialized[3], disc_serialized[3]); // Same handle high byte
        
        // But different event types
        assert_ne!(serialized[0], disc_serialized[0]);
        
        // Test GATTS events for this connection
        let write_data = [0xFF, 0xEE, 0xDD, 0xCC];
        let gatts_write = create_gatts_write_event(conn_handle, 100, &write_data);
        assert!(gatts_write.is_ok());
        
        let write_event = gatts_write.unwrap();
        let write_serialized = write_event.serialize().unwrap();
        
        // Should reference same connection handle
        assert_eq!(write_serialized[2], 42); // Same conn_handle low byte
        assert_eq!(write_serialized[3], 0);  // Same conn_handle high byte
        
        // But have different event type
        assert_eq!(write_serialized[0], 0x50); // BLE_GATTS_EVT_WRITE
    }
}
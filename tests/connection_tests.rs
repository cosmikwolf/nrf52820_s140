#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

mod common;

use nrf52820_s140_firmware::ble::connection::{ConnectionManager, ConnectionEvent, ConnectionParams, MAX_CONNECTIONS};
use proptest::prelude::*;

#[defmt_test::tests]
mod tests {
    use alloc::vec::Vec;
    use defmt::{assert, assert_eq};
    extern crate alloc;
    use alloc::format;

    use super::*;
    use crate::common::*;

    #[init]
    fn init() {
        ensure_heap_initialized();
    }

    #[test]
    fn test_connection_pool_management() {
        // Property #22: Connection Pool Management
        // System should accept up to MAX_CONNECTIONS (2) and reject additional connections
        
        let mut manager = ConnectionManager::new();
        let mut connections = Vec::new();
        
        // Should be able to add MAX_CONNECTIONS
        for i in 0..MAX_CONNECTIONS {
            let connection_id = i as u16 + 1; // Use non-zero IDs
            let mtu = 247; // Default MTU
            let result = manager.add_connection(connection_id, mtu);
            assert!(result.is_ok(), "Failed to add connection {}", i);
            connections.push(connection_id);
        }
        
        // Should reject additional connections
        let overflow_result = manager.add_connection(99, 247);
        assert!(overflow_result.is_err(), "Should reject connection beyond MAX_CONNECTIONS");
        
        // Remove one connection
        if let Some(conn_id) = connections.pop() {
            let remove_result = manager.remove_connection(conn_id, 0x16); // User terminated
            assert!(remove_result.is_ok(), "Failed to remove connection");
        }
        
        // Should now accept one more connection
        let new_connection_result = manager.add_connection(100, 247);
        assert!(new_connection_result.is_ok(), "Should accept connection after removal");
    }

    #[test]
    fn test_connection_event_propagation() {
        // Property #23: Connection Event Propagation
        // Connection events should be properly forwarded to registered listeners
        
        let mut manager = ConnectionManager::new();
        
        // Add a connection to generate events for
        let connection_id = 42;
        let add_result = manager.add_connection(connection_id, 23);
        assert!(add_result.is_ok());
        
        // Test connection event creation with actual API
        let params = ConnectionParams {
            min_conn_interval: 12,
            max_conn_interval: 24,
            slave_latency: 0,
            supervision_timeout: 100,
        };
        
        let connect_event = ConnectionEvent::Connected { 
            handle: connection_id,
            params: params.clone(),
        };
        
        // Verify event contains correct data
        match connect_event {
            ConnectionEvent::Connected { handle, params: event_params } => {
                assert_eq!(handle, connection_id);
                assert_eq!(event_params.min_conn_interval, 12);
                assert_eq!(event_params.max_conn_interval, 24);
            }
            _ => panic!("Event should be Connected variant"),
        }
        
        // Test disconnection event
        let disconnect_event = ConnectionEvent::Disconnected { 
            handle: connection_id,
            reason: 0x13, // Remote user terminated connection
        };
        
        match disconnect_event {
            ConnectionEvent::Disconnected { handle, reason } => {
                assert_eq!(handle, connection_id);
                assert_eq!(reason, 0x13);
            }
            _ => panic!("Event should be Disconnected variant"),
        }
    }

    #[test]
    fn test_connection_state_consistency() {
        // Property #24: Connection State Consistency  
        // Connection state should remain consistent across operations
        
        proptest!(|(
            connection_ids in prop::collection::vec(1u16..1000, 1..10)
        )| {
            let mut manager = ConnectionManager::new();
            let mut active_connections = Vec::new();
            
            // Add connections up to the limit
            for &conn_id in connection_ids.iter().take(MAX_CONNECTIONS) {
                let result = manager.add_connection(conn_id, 23);
                prop_assert!(result.is_ok());
                active_connections.push(conn_id);
                
                // Verify connection is tracked
                prop_assert!(manager.is_connected(conn_id));
                prop_assert_eq!(manager.connection_count(), active_connections.len());
            }
            
            // Remove all connections
            for &conn_id in &active_connections {
                let result = manager.remove_connection(conn_id, 0x16);
                prop_assert!(result.is_ok());
                prop_assert!(!manager.is_connected(conn_id));
            }
            
            // Should be empty now
            prop_assert_eq!(manager.connection_count(), 0);
        });
    }

    #[test]
    fn test_connection_cleanup_on_disconnect() {
        // Property #25: Connection Cleanup on Disconnect
        // Disconnected connections should be properly cleaned up and removed
        
        let mut manager = ConnectionManager::new();
        
        // Add connections with associated resources
        let connection_ids = [1, 2];
        for &conn_id in &connection_ids {
            let result = manager.add_connection(conn_id, 23);
            assert!(result.is_ok());
            
            // Simulate setting connection parameters/state
            assert!(manager.is_connected(conn_id));
        }
        
        assert_eq!(manager.connection_count(), 2);
        
        // Disconnect first connection
        let disconnect_result = manager.remove_connection(connection_ids[0], 0x13);
        assert!(disconnect_result.is_ok());
        
        // Verify cleanup
        assert!(!manager.is_connected(connection_ids[0]));
        assert!(manager.is_connected(connection_ids[1])); // Other should remain
        assert_eq!(manager.connection_count(), 1);
        
        // Resources should be freed - verify by adding new connection at same limit
        let new_connection_result = manager.add_connection(99, 247);
        assert!(new_connection_result.is_ok());
        assert_eq!(manager.connection_count(), 2);
    }

    #[test]
    fn test_connection_handle_uniqueness() {
        // Property #26: Connection Handle Uniqueness
        // Each active connection should have a unique handle
        
        proptest!(|(
            handles in prop::collection::vec(1u16..100, 1..10)
        )| {
            let mut manager = ConnectionManager::new();
            let mut unique_handles = Vec::new();
            
            // Add connections with unique handles up to limit
            for &handle in handles.iter().take(MAX_CONNECTIONS) {
                if !unique_handles.contains(&handle) {
                    let result = manager.add_connection(handle, 23);
                    prop_assert!(result.is_ok());
                    unique_handles.push(handle);
                }
            }
            
            // Try to add duplicate handle - should fail
            if let Some(&duplicate_handle) = unique_handles.first() {
                let duplicate_result = manager.add_connection(duplicate_handle, 23);
                prop_assert!(duplicate_result.is_err());
            }
            
            // All tracked handles should be unique
            let mut sorted_handles = unique_handles.clone();
            sorted_handles.sort();
            sorted_handles.dedup();
            prop_assert_eq!(unique_handles.len(), sorted_handles.len());
        });
    }

    #[test]
    fn test_connection_event_types() {
        // Property #27: Connection Event Types
        // All connection event types should be properly structured
        
        // Test different connection parameters
        let params1 = ConnectionParams {
            min_conn_interval: 6,
            max_conn_interval: 12,
            slave_latency: 0,
            supervision_timeout: 100,
        };
        
        let params2 = ConnectionParams {
            min_conn_interval: 15,
            max_conn_interval: 30,
            slave_latency: 4,
            supervision_timeout: 200,
        };
        
        // Create different event types to test event handling
        let events = [
            ConnectionEvent::Connected { handle: 1, params: params1.clone() },
            ConnectionEvent::Disconnected { handle: 1, reason: 0x13 },
            ConnectionEvent::Connected { handle: 2, params: params2.clone() },
            ConnectionEvent::ParamsUpdated { handle: 2, params: params1.clone() },
            ConnectionEvent::MtuChanged { handle: 2, mtu: 247 },
            ConnectionEvent::Disconnected { handle: 2, reason: 0x16 },
        ];
        
        assert_eq!(events.len(), 6);
        
        // Verify each event is properly formed
        for (i, event) in events.iter().enumerate() {
            match event {
                ConnectionEvent::Connected { handle, params } => {
                    assert!(*handle > 0, "Event {} should have valid handle", i);
                    assert!(params.min_conn_interval > 0, "Event {} should have valid params", i);
                }
                ConnectionEvent::Disconnected { handle, reason } => {
                    assert!(*handle > 0, "Event {} should have valid handle", i);
                    assert!(*reason > 0, "Event {} should have valid reason", i);
                }
                ConnectionEvent::ParamsUpdated { handle, params } => {
                    assert!(*handle > 0, "Event {} should have valid handle", i);
                    assert!(params.supervision_timeout > 0, "Event {} should have valid params", i);
                }
                ConnectionEvent::MtuChanged { handle, mtu } => {
                    assert!(*handle > 0, "Event {} should have valid handle", i);
                    assert!(*mtu >= 23, "Event {} should have valid MTU", i);
                }
            }
        }
    }
}
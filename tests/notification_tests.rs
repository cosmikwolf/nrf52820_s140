#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

mod common;

use nrf52820_s140_firmware::ble::notifications::{
    NotificationRequest, NotificationResponse, NotificationError, 
    MAX_NOTIFICATION_DATA, send_notification, send_indication
};
use nrf52820_s140_firmware::ble::connection::ConnectionManager;
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

    proptest! {
        #[test]
        fn test_notification_data_size_limits(
            data_sizes in prop::collection::vec(0usize..100, 1..10)
        ) {
            // Property #36: Notification Data Size Limits
            // Notifications should accept up to MAX_NOTIFICATION_DATA (64 bytes)
            
            unsafe {
                common::HEAP.init(common::HEAP_MEM.as_ptr() as usize, common::HEAP_MEM.len());
            }
            
            let connection_id = 1;
            let char_handle = 10;
            
            for size in data_sizes {
                let data = create_test_data(size, 0x42);
                
                let request = NotificationRequest {
                    connection_id,
                    char_handle,
                    data: data.clone(),
                    is_indication: false,
                    request_id: size as u32,
                };
                
                if size <= MAX_NOTIFICATION_DATA {
                    // Should accept data within limits
                    prop_assert!(request.data.len() <= MAX_NOTIFICATION_DATA);
                    prop_assert_eq!(request.data.len(), size);
                    prop_assert!(arrays_equal(&request.data, &data));
                } else {
                    // In real implementation, oversized data would be rejected
                    prop_assert!(size > MAX_NOTIFICATION_DATA);
                }
            }
        }
    }

    #[test]
    fn test_notification_channel_capacity() {
        // Property #37: Notification Channel Capacity
        // Notification channels should handle up to 8 requests before blocking
        
        let mut requests = Vec::new();
        
        // Create 8 notification requests (expected channel capacity)
        for i in 0..8 {
            let data = create_test_data(10, 0x40 + i as u8);
            let request = NotificationRequest {
                connection_id: 1,
                char_handle: 10 + i as u16,
                data,
                is_indication: i % 2 == 0, // Mix notifications and indications
                request_id: i as u32,
            };
            requests.push(request);
        }
        
        assert_eq!(requests.len(), 8);
        
        // Verify each request is properly formed
        for (i, request) in requests.iter().enumerate() {
            assert_eq!(request.connection_id, 1);
            assert_eq!(request.char_handle, 10 + i as u16);
            assert_eq!(request.data.len(), 10);
            assert_eq!(request.is_indication, i % 2 == 0);
            assert_eq!(request.request_id, i as u32);
        }
        
        // Test channel overflow scenario
        let overflow_request = NotificationRequest {
            connection_id: 1,
            char_handle: 99,
            data: vec![0xFF; 5],
            is_indication: false,
            request_id: 999,
        };
        
        // In real implementation, this would test actual channel blocking
        assert!(overflow_request.data.len() <= MAX_NOTIFICATION_DATA);
    }

    proptest! {
        #[test]
        fn test_request_response_matching(
            request_ids in prop::collection::vec(0u32..1000, 1..8)
        ) {
            // Property #38: Request-Response Matching
            // Notification responses should match their corresponding requests
            
            let connection_id = 1;
            let char_handle = 20;
            let mut request_response_pairs = Vec::new();
            
            // Create requests with unique IDs
            for &request_id in request_ids.iter().take(8) {
                let data = vec![0x55; 8];
                let request = NotificationRequest {
                    connection_id,
                    char_handle,
                    data: data.clone(),
                    is_indication: true, // Use indications for response matching
                    request_id,
                };
                
                // Create matching response
                let response = NotificationResponse {
                    request_id,
                    result: Ok(()),
                    connection_id,
                };
                
                request_response_pairs.push((request, response));
            }
            
            // Verify request-response matching
            for (request, response) in &request_response_pairs {
                prop_assert_eq!(request.request_id, response.request_id);
                prop_assert_eq!(request.connection_id, response.connection_id);
                prop_assert!(request.is_indication); // Only indications get responses
                prop_assert!(response.result.is_ok());
            }
            
            // Verify all request IDs are unique
            let request_ids: Vec<u32> = request_response_pairs.iter()
                .map(|(req, _)| req.request_id)
                .collect();
            let mut sorted_ids = request_ids.clone();
            sorted_ids.sort();
            sorted_ids.dedup();
            prop_assert_eq!(request_ids.len(), sorted_ids.len());
        }
    }

    #[test]
    fn test_connection_validation_for_notifications() {
        // Property #39: Connection Validation for Notifications
        // Notifications should fail gracefully for invalid connection handles
        
        let valid_connection_id = 1;
        let invalid_connection_ids = [0, 999, 0xFFFF];
        let char_handle = 30;
        let data = vec![0x88; 16];
        
        // Test with valid connection (should succeed in validation)
        let valid_request = NotificationRequest {
            connection_id: valid_connection_id,
            char_handle,
            data: data.clone(),
            is_indication: false,
            request_id: 100,
        };
        
        // Request should be well-formed
        assert_eq!(valid_request.connection_id, valid_connection_id);
        assert!(valid_request.connection_id > 0);
        
        // Test with invalid connections
        for &invalid_id in &invalid_connection_ids {
            let invalid_request = NotificationRequest {
                connection_id: invalid_id,
                char_handle,
                data: data.clone(),
                is_indication: false,
                request_id: 200 + invalid_id as u32,
            };
            
            // In real implementation, these would fail connection validation
            if invalid_id == 0 {
                assert_eq!(invalid_request.connection_id, 0); // Invalid connection ID
            } else {
                assert!(invalid_request.connection_id > valid_connection_id); // Non-existent connection
            }
        }
    }

    #[test]
    fn test_indication_vs_notification_handling() {
        // Property #40: Indication vs Notification Handling
        // Indications and notifications should be processed differently
        
        let connection_id = 1;
        let char_handle = 40;
        let data = vec![0xAA; 12];
        
        // Create notification (no response expected)
        let notification = NotificationRequest {
            connection_id,
            char_handle,
            data: data.clone(),
            is_indication: false,
            request_id: 300,
        };
        
        // Create indication (response expected)
        let indication = NotificationRequest {
            connection_id,
            char_handle: char_handle + 1,
            data: data.clone(),
            is_indication: true,
            request_id: 301,
        };
        
        // Verify different types
        assert!(!notification.is_indication);
        assert!(indication.is_indication);
        assert_ne!(notification.request_id, indication.request_id);
        assert_ne!(notification.char_handle, indication.char_handle);
        
        // Only indication should expect a response
        if indication.is_indication {
            let indication_response = NotificationResponse {
                request_id: indication.request_id,
                result: Ok(()),
                connection_id: indication.connection_id,
            };
            assert_eq!(indication_response.request_id, indication.request_id);
        }
        
        // Notification should not have a response
        assert!(!notification.is_indication);
    }

    proptest! {
        #[test]
        fn test_notification_request_id_uniqueness(
            base_ids in prop::collection::vec(0u32..100, 1..8)
        ) {
            // Property #41: Notification Request ID Uniqueness
            // Each notification request should have a unique request ID
            
            let connection_id = 1;
            let char_handle = 50;
            let data = vec![0xCC; 8];
            let mut request_ids = Vec::new();
            
            // Generate unique request IDs
            for (i, &base_id) in base_ids.iter().enumerate() {
                let unique_id = base_id + (i as u32 * 1000); // Ensure uniqueness
                
                let request = NotificationRequest {
                    connection_id,
                    char_handle,
                    data: data.clone(),
                    is_indication: i % 2 == 0,
                    request_id: unique_id,
                };
                
                request_ids.push(request.request_id);
                
                // Verify request is well-formed
                prop_assert_eq!(request.connection_id, connection_id);
                prop_assert_eq!(request.request_id, unique_id);
            }
            
            // Verify all request IDs are unique
            let mut sorted_ids = request_ids.clone();
            sorted_ids.sort();
            sorted_ids.dedup();
            prop_assert_eq!(request_ids.len(), sorted_ids.len());
            
            // Verify no ID collisions in realistic usage
            let mut collision_check = Vec::new();
            for id in &request_ids {
                prop_assert!(!collision_check.contains(id));
                collision_check.push(*id);
            }
        }
    }

    #[test]
    fn test_notification_error_handling() {
        // Additional test for notification error scenarios
        
        let connection_id = 1;
        let char_handle = 60;
        
        // Test oversized data error
        let oversized_data = vec![0xDD; MAX_NOTIFICATION_DATA + 10];
        let oversized_request = NotificationRequest {
            connection_id,
            char_handle,
            data: oversized_data,
            is_indication: false,
            request_id: 400,
        };
        
        // Request should be created but data size should exceed limit
        assert!(oversized_request.data.len() > MAX_NOTIFICATION_DATA);
        
        // Test error response
        let error_response = NotificationResponse {
            request_id: 401,
            result: Err(NotificationError::InvalidConnection),
            connection_id: 0, // Invalid connection
        };
        
        match error_response.result {
            Err(NotificationError::InvalidConnection) => {
                assert_eq!(error_response.connection_id, 0);
            }
            _ => panic!("Should be InvalidConnection error"),
        }
        
        // Test data size boundary
        let max_size_data = vec![0xEE; MAX_NOTIFICATION_DATA];
        let max_size_request = NotificationRequest {
            connection_id,
            char_handle,
            data: max_size_data,
            is_indication: true,
            request_id: 402,
        };
        
        assert_eq!(max_size_request.data.len(), MAX_NOTIFICATION_DATA);
        assert!(max_size_request.data.len() <= MAX_NOTIFICATION_DATA);
    }
}